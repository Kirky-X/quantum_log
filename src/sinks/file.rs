//! 单一文件 Sink
//!
//! 此模块实现写入单一文件的 sink，支持文件轮转和清理。

use crate::config::FileConfig;
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};
use crate::sinks::file_common::{FileWriter, FileCleaner};
use crate::utils::FileTools;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::Level;

/// 单一文件 Sink
pub struct FileSink {
    /// 配置
    config: FileConfig,
    /// 事件发送器
    sender: Option<mpsc::Sender<SinkMessage>>,
    /// 处理器句柄
    processor_handle: Option<JoinHandle<()>>,
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
            sender: None,
            processor_handle: None,
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
        if self.sender.is_some() {
            return Err(QuantumLogError::ConfigError("Sink already started".to_string()));
        }

        let buffer_size = 1000; // 默认缓冲区大小

        let (sender, receiver) = mpsc::channel(buffer_size);
        
        let processor = FileSinkProcessor::new(self.config.clone(), receiver).await?;
        let handle = tokio::spawn(async move {
            if let Err(e) = processor.run().await {
                tracing::error!("FileSink processor error: {}", e);
            }
        });

        self.sender = Some(sender);
        self.processor_handle = Some(handle);

        Ok(())
    }

    /// 发送事件
    pub async fn send_event(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(sender) = &self.sender {
            let message = SinkMessage::Event(Box::new(event));
            
            sender.send(message).await
                .map_err(|_| QuantumLogError::SinkError("Failed to send event to FileSink".to_string()))?;
        }
        Ok(())
    }

    /// 尝试发送事件（非阻塞）
    pub fn try_send_event(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(sender) = &self.sender {
            let message = SinkMessage::Event(Box::new(event));
            
            sender.try_send(message)
                .map_err(|e| match e {
                    mpsc::error::TrySendError::Full(_) => {
                        QuantumLogError::SinkError("FileSink buffer full".to_string())
                    },
                    mpsc::error::TrySendError::Closed(_) => {
                        QuantumLogError::SinkError("FileSink closed".to_string())
                    },
                })?;
        }
        Ok(())
    }

    /// 关闭 sink
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(sender) = self.sender.take() {
            let (tx, rx) = oneshot::channel();
            
            // 发送关闭信号
            if sender.send(SinkMessage::Shutdown(tx)).await.is_err() {
                return Err(QuantumLogError::SinkError("Failed to send shutdown signal".to_string()));
            }
            
            // 等待关闭完成
            match rx.await {
                Ok(result) => result?,
                Err(_) => return Err(QuantumLogError::SinkError("Shutdown signal lost".to_string())),
            }
        }
        
        // 等待处理器完成
        if let Some(handle) = self.processor_handle.take() {
            if let Err(e) = handle.await {
                tracing::error!("Error waiting for FileSink processor: {}", e);
            }
        }
        
        Ok(())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.sender.is_some()
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
                    format!("目录不可写: {}", config.directory.display())
                )
            });
        }
        
        let writer = FileWriter::new(config.clone()).await?;
        
        let cleaner = if let Some(ref rotation) = config.rotation {
            // 如果启用了轮转，创建清理器
            let base_path = config.directory
                .to_string_lossy()
                .to_string();
            
            let mut cleaner = FileCleaner::new(&base_path);
            
            // 根据轮转策略设置清理参数
            if let Some(max_files) = rotation.max_files {
                cleaner = cleaner.max_files(max_files);
            }
            
            Some(cleaner)
        } else {
            None
        };
        
        Ok(Self {
            config,
            receiver,
            writer,
            cleaner,
            level_filter: None,
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
                },
                SinkMessage::Shutdown(response) => {
                    let result = self.shutdown().await;
                    let _ = response.send(result);
                    break;
                },
            }
        }
        Ok(())
    }

    /// 处理事件
    async fn handle_event(&mut self, event: QuantumLogEvent) -> Result<()> {
        // 检查级别过滤
        if let Some(ref filter_level) = self.level_filter {
            let event_level = event.level.parse::<Level>()
                .map_err(|_| QuantumLogError::ConfigError(format!("Invalid log level: {}", event.level)))?;
            
            if event_level < *filter_level {
                return Ok(());
            }
        }
        
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
        Ok(format!("{}
", event.to_formatted_string(format_type)))
    }

    /// 关闭处理器
    async fn shutdown(&mut self) -> Result<()> {
        // 刷新写入器
        if let Err(e) = self.writer.flush().await {
            tracing::error!("Error flushing file writer: {}", e);
        }
        
        // 执行清理（如果启用）
        if let Some(ref cleaner) = self.cleaner {
            match cleaner.cleanup().await {
                Ok(removed) => {
                    if removed > 0 {
                        tracing::info!("Cleaned up {} old log files", removed);
                    }
                },
                Err(e) => {
                    tracing::error!("Error during log file cleanup: {}", e);
                },
            }
        }
        
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
        static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
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