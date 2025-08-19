//! 单一文件 Sink
//!
//! 此模块实现写入单一文件的 sink，支持文件轮转和清理。

use crate::config::FileConfig;
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};
use crate::sinks::file_common::{FileCleaner, FileWriter};
use crate::sinks::traits::{ExclusiveSink, QuantumSink, SinkError};
use crate::utils::FileTools;
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::Level;
use std::sync::{Arc, Mutex};

/// 单一文件 Sink
#[derive(Debug)]
pub struct FileSink {
    /// 配置
    config: FileConfig,
    /// 事件发送器（内部可变性，便于从 &self 关闭）
    sender: Arc<Mutex<Option<mpsc::Sender<SinkMessage>>>>,
    /// 处理器句柄（内部可变性，便于从 &self 等待任务结束）
    processor_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

/// Sink 消息
enum SinkMessage {
    /// 日志事件
    Event(Box<QuantumLogEvent>),
    /// 关闭信号
    Shutdown(oneshot::Sender<Result<()>>),
}

/// 单一文件 Sink 处理器
struct FileSinkProcessor {
    /// 配置
    config: FileConfig,
    /// 事件接收器
    receiver: mpsc::Receiver<SinkMessage>,
    /// 文件写入器
    writer: FileWriter,
    /// 文件清理器
    cleaner: Option<FileCleaner>,
    /// 级别过滤器
    level_filter: Option<Level>,
}

impl FileSink {
    /// 创建新的文件 sink
    pub fn new(config: FileConfig) -> Self {
        Self {
            config,
            sender: Arc::new(Mutex::new(None)),
            processor_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// 设置级别过滤器
    pub fn with_level_filter(self, _level: Level) -> Self {
        // 注意：这里我们需要在配置中添加级别过滤器字段
        // 暂时存储在内部，实际实现时需要修改配置结构
        self
    }

    /// 启动 sink
    pub async fn start(&mut self) -> Result<()> {
        if self.sender.lock().unwrap().is_some() {
            return Err(QuantumLogError::ConfigError(
                "Sink already started".into(),
            ));
        }

        let buffer_size = 1000; // 默认缓冲区大小

        let (sender, receiver) = mpsc::channel(buffer_size);

        let processor = FileSinkProcessor::new(self.config.clone(), receiver).await?;
        let handle = tokio::spawn(async move {
            if let Err(e) = processor.run().await {
                tracing::error!("FileSink processor error: {}", e);
            }
        });

        {
            let mut guard = self.sender.lock().unwrap();
            *guard = Some(sender);
        }
        {
            let mut guard = self.processor_handle.lock().unwrap();
            *guard = Some(handle);
        }

        Ok(())
    }

    /// 发送事件
    pub async fn send_event_internal(&self, event: QuantumLogEvent) -> Result<()> {
        let sender_opt = {
            let guard = self.sender.lock().unwrap();
            guard.as_ref().cloned()
        };
        if let Some(sender) = sender_opt {
            let message = SinkMessage::Event(Box::new(event));
            sender
                .send(message)
                .await
                .map_err(|_| QuantumLogError::SinkError("Failed to send event to FileSink".into()))?;
        }
        Ok(())
    }

    /// 尝试发送事件（非阻塞）
    pub fn try_send_event(&self, event: QuantumLogEvent) -> Result<()> {
        let sender_opt = {
            let guard = self.sender.lock().unwrap();
            guard.as_ref().cloned()
        };
        if let Some(sender) = sender_opt {
            let message = SinkMessage::Event(Box::new(event));
            sender.try_send(message).map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => {
                    QuantumLogError::SinkError("FileSink buffer full".into())
                }
                mpsc::error::TrySendError::Closed(_) => {
                    QuantumLogError::SinkError("FileSink closed".into())
                }
            })?;
        }
        Ok(())
    }

    /// 内部通用关闭逻辑，支持从 &self 调用
    async fn shutdown_ref(&self) -> Result<()> {
        // 发送关闭信号
        let sender_taken = {
            let mut guard = self.sender.lock().unwrap();
            guard.take()
        };
        if let Some(sender) = sender_taken {
            let (tx, rx) = oneshot::channel();

            if sender.send(SinkMessage::Shutdown(tx)).await.is_err() {
                return Err(QuantumLogError::SinkError(
                    "Failed to send shutdown signal".into(),
                ));
            }

            match rx.await {
                Ok(result) => result?,
                Err(_) => {
                    return Err(QuantumLogError::SinkError(
                        "Shutdown signal lost".into(),
                    ))
                }
            }
        }

        // 等待处理器完成
        let handle_taken = {
            let mut guard = self.processor_handle.lock().unwrap();
            guard.take()
        };
        if let Some(handle) = handle_taken {
            if let Err(e) = handle.await {
                tracing::error!("Error waiting for FileSink processor: {}", e);
            }
        }

        Ok(())
    }

    /// 关闭 sink（保留以兼容现有调用点）
    pub async fn shutdown(self) -> Result<()> {
        self.shutdown_ref().await
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.sender.lock().unwrap().is_some()
    }

    /// 获取配置
    pub fn config(&self) -> &FileConfig {
        &self.config
    }
}

impl FileSinkProcessor {
    /// 创建新的处理器
    async fn new(config: FileConfig, receiver: mpsc::Receiver<SinkMessage>) -> Result<Self> {
        // 确保目录存在
        FileTools::ensure_directory_exists(&config.directory)?;

        // 检查目录是否可写
        if !FileTools::is_directory_writable(&config.directory) {
            return Err(QuantumLogError::IoError {
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("目录不可写: {}", config.directory.display()),
                ),
            });
        }

        let writer = FileWriter::new(config.clone()).await?;

        let cleaner = if let Some(ref rotation) = config.rotation {
            // 如果启用了轮转，创建清理器
            let base_path = config.directory.to_string_lossy().into_owned();
            let mut cleaner = FileCleaner::new(&base_path);

            // 根据轮转策略设置清理参数
            if let Some(max_files) = rotation.max_files {
                cleaner = cleaner.max_files(max_files);
            }

            Some(cleaner)
        } else {
            None
        };

        // 解析级别过滤器
        let level_filter = if let Some(ref level_str) = config.level {
            Some(level_str.parse::<Level>().map_err(|_| {
                QuantumLogError::ConfigError(format!("Invalid log level: {}", level_str))
            })?)
        } else {
            None
        };

        Ok(Self {
            config,
            receiver,
            writer,
            cleaner,
            level_filter,
        })
    }

    /// 运行处理器
    async fn run(mut self) -> Result<()> {
        while let Some(message) = self.receiver.recv().await {
            match message {
                SinkMessage::Event(event) => {
                    if let Err(e) = self.handle_event(*event).await {
                        tracing::error!("Error handling event in FileSink: {}", e);
                    }
                }
                SinkMessage::Shutdown(response) => {
                    let result = self.shutdown().await;
                    let _ = response.send(result);
                    break;
                }
            }
        }
        Ok(())
    }

    /// 处理事件
    async fn handle_event(&mut self, event: QuantumLogEvent) -> Result<()> {
        // 检查级别过滤
        if let Some(ref filter_level) = self.level_filter {
            let event_level = event
                .level
                .parse::<Level>()
                .map_err(|_| QuantumLogError::ConfigError(format!("Invalid log level: {}", event.level)))?;
            if event_level < *filter_level {
                return Ok(());
            }
        }

        // 目前不进行级别过滤，交由上层控制
        // 格式化事件
        let formatted = self.format_event(&event)?;
        // 写入文件
        self.writer.write(formatted.as_bytes()).await?;
        Ok(())
    }

    /// 格式化事件
    fn format_event(&self, event: &QuantumLogEvent) -> Result<String> {
        // 根据配置的输出类型选择格式化方式
        let format_type = match self.config.output_type {
            crate::config::FileOutputType::Json => "json",
            crate::config::FileOutputType::Text => "full",
            crate::config::FileOutputType::Csv => "csv",
        };
        let formatted = event.to_formatted_string(format_type);
        let mut result = String::with_capacity(formatted.len() + 1);
        result.push_str(&formatted);
        result.push('\n');
        Ok(result)
    }

    /// 关闭处理器
    async fn shutdown(&mut self) -> Result<()> {
        tracing::debug!("FileSinkProcessor 开始关闭");
        
        // 刷新写入器
        if let Err(e) = self.writer.flush().await {
            tracing::error!("Error flushing file writer: {}", e);
        } else {
            tracing::debug!("文件写入器刷新完成");
        }
        
        // 显式关闭文件句柄
        if let Err(e) = self.writer.close().await {
            tracing::error!("Error closing file writer: {}", e);
        } else {
            tracing::debug!("文件写入器关闭完成");
        }

        // 执行清理（如果启用）
        if let Some(ref cleaner) = self.cleaner {
            tracing::debug!("开始执行文件清理");
            match cleaner.cleanup().await {
                Ok(removed) => {
                    if removed > 0 {
                        tracing::info!("Cleaned up {} old log files", removed);
                    } else {
                        tracing::debug!("没有需要清理的旧日志文件");
                    }
                }
                Err(e) => {
                    tracing::error!("Error during log file cleanup: {}", e);
                }
            }
        }
        
        // 关闭消息接收器
        self.receiver.close();
        
        // 清空接收器中剩余的消息
        let mut remaining_messages = 0;
        while let Ok(message) = self.receiver.try_recv() {
            remaining_messages += 1;
            match message {
                SinkMessage::Event(_) => {
                    // 丢弃剩余的事件
                }
                SinkMessage::Shutdown(sender) => {
                    // 响应剩余的关闭请求
                    let _ = sender.send(Ok(()));
                }
            }
        }
        
        if remaining_messages > 0 {
            tracing::warn!("丢弃了 {} 个未处理的消息", remaining_messages);
        }
        
        tracing::debug!("FileSinkProcessor 关闭完成");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::ContextInfo;
    use tempfile::TempDir;
    use tracing::Level;

    fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
        // Create a simple metadata without circular dependency
        static CALLSITE: tracing::callsite::DefaultCallsite =
            tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
                "test",
                "test_target",
                Level::INFO,
                Some("test.rs"),
                Some(42),
                Some("test_module"),
                tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
                tracing::metadata::Kind::EVENT,
            ));
        let fields = tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE));
        let metadata = tracing::Metadata::new(
            "test",
            "test_target",
            level,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            fields,
            tracing::metadata::Kind::EVENT,
        );

        QuantumLogEvent::new(
            level,
            message.to_string(),
            &metadata,
            std::collections::HashMap::new(),
            ContextInfo::default(),
        )
    }

    #[tokio::test]
    async fn test_file_sink_creation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let config = FileConfig {
            enabled: true,
            level: None,
            output_type: crate::config::FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "test".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: crate::config::FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };

        let sink = FileSink::new(config);
        assert!(!sink.is_running());
    }

    #[tokio::test]
    async fn test_file_sink_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let config = FileConfig {
            enabled: true,
            level: None,
            output_type: crate::config::FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "test".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: crate::config::FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };

        let mut sink = FileSink::new(config);

        // 启动
        let result = sink.start().await;
        assert!(result.is_ok());
        assert!(sink.is_running());

        // 关闭
        let result = sink.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_file_sink_send_events() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let config = FileConfig {
            enabled: true,
            level: None,
            output_type: crate::config::FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "test".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: crate::config::FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };

        let mut sink = FileSink::new(config);
        sink.start().await.unwrap();

        // 发送事件
        let event = create_test_event(Level::INFO, "Test message");
        sink.send_event(event).await.unwrap();

        // 等待一下让事件被处理
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        sink.shutdown().await.unwrap();

        // 检查文件是否被创建并包含内容
        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("Test message"));
    }

    #[tokio::test]
    async fn test_file_sink_try_send() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let config = FileConfig {
            enabled: true,
            level: None,
            output_type: crate::config::FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "test".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: crate::config::FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };

        let mut sink = FileSink::new(config);
        sink.start().await.unwrap();

        // 使用 try_send
        let event = create_test_event(Level::INFO, "Try send message");
        let result = sink.try_send_event(event);
        assert!(result.is_ok());

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        sink.shutdown().await.unwrap();

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("Try send message"));
    }

    #[tokio::test]
    async fn test_file_sink_multiple_events() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let config = FileConfig {
            enabled: true,
            level: None,
            output_type: crate::config::FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "test".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: crate::config::FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };

        let mut sink = FileSink::new(config);
        sink.start().await.unwrap();

        // 发送多个事件
        for i in 0..10 {
            let event = create_test_event(Level::INFO, &format!("Message {}", i));
            sink.send_event(event).await.unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        sink.shutdown().await.unwrap();

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();

        // 检查所有消息都被写入
        for i in 0..10 {
            assert!(content.contains(&format!("Message {}", i)));
        }
    }

    #[tokio::test]
    async fn test_file_sink_config_access() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.log");

        let config = FileConfig {
            enabled: true,
            level: None,
            output_type: crate::config::FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "test".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: crate::config::FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };

        let sink = FileSink::new(config.clone());

        // 检查配置访问
        assert_eq!(sink.config().directory, config.directory);
        assert_eq!(sink.config().filename_base, config.filename_base);
        assert_eq!(sink.config().extension, config.extension);
    }
}

// 实现新的统一 Sink trait
#[async_trait]
impl QuantumSink for FileSink {
    type Config = FileConfig;
    type Error = SinkError;

    async fn send_event(&self, event: QuantumLogEvent) -> std::result::Result<(), Self::Error> {
        self.send_event_internal(event).await.map_err(|e| match e {
            QuantumLogError::ChannelError(msg) => SinkError::Generic(msg),
            QuantumLogError::ConfigError(msg) => SinkError::Config(msg),
            QuantumLogError::IoError { source } => SinkError::Io(source),
            _ => SinkError::Generic(e.to_string()),
        })
    }

    async fn shutdown(&self) -> std::result::Result<(), Self::Error> {
        self.shutdown_ref().await.map_err(|e| match e {
            QuantumLogError::ChannelError(msg) => SinkError::Generic(msg),
            QuantumLogError::ConfigError(msg) => SinkError::Config(msg),
            QuantumLogError::IoError { source } => SinkError::Io(source),
            _ => SinkError::Generic(e.to_string()),
        })
    }

    async fn is_healthy(&self) -> bool {
        self.is_running()
    }

    fn name(&self) -> &'static str {
        "file"
    }

    fn stats(&self) -> String {
        format!(
            "FileSink: running={}, file={}/{}.{}",
            self.is_running(),
            self.config.directory.display(),
            self.config.filename_base,
            self.config.extension.as_deref().unwrap_or("log")
        )
    }

    fn metadata(&self) -> crate::sinks::traits::SinkMetadata {
        crate::sinks::traits::SinkMetadata {
            name: "file".to_string(),
            sink_type: crate::sinks::traits::SinkType::Exclusive,
            enabled: self.is_running(),
            description: Some(format!(
                "File sink writing to {}/{}.{}",
                self.config.directory.display(),
                self.config.filename_base,
                self.config.extension.as_deref().unwrap_or("log")
            )),
        }
    }
}

// 标记为独占型 sink
impl ExclusiveSink for FileSink {}
