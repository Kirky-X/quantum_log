//! 按级别分离的文件 Sink
//!
//! 此模块实现按日志级别分离到不同文件的 sink。

use crate::config::LevelFileConfig;
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};
use crate::sinks::file_common::{FileCleaner, FilePathGenerator, FileWriter};
use crate::sinks::traits::{ExclusiveSink, QuantumSink, SinkError};
use crate::utils::FileTools;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// 按级别分离的文件 Sink
#[derive(Debug)]
pub struct LevelFileSink {
    /// 配置
    config: LevelFileConfig,
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

/// 按级别分离的文件 Sink 处理器
struct LevelFileSinkProcessor {
    /// 配置
    config: LevelFileConfig,
    /// 事件接收器
    receiver: mpsc::Receiver<SinkMessage>,
    /// 文件写入器映射（按级别）
    writers: HashMap<String, FileWriter>,
    /// 路径生成器
    path_generator: FilePathGenerator,
    /// 文件清理器
    cleaner: Option<FileCleaner>,
}

impl LevelFileSink {
    /// 创建新的按级别分离文件 sink
    pub fn new(config: LevelFileConfig) -> Self {
        Self {
            config,
            sender: None,
            processor_handle: None,
        }
    }

    /// 启动 sink
    pub async fn start(&mut self) -> Result<()> {
        if self.sender.is_some() {
            return Err(QuantumLogError::ConfigError(
                "Sink already started".to_string(),
            ));
        }

        let buffer_size = 1000; // Fixed buffer size since LevelFileConfig doesn't have backpressure field

        let (sender, receiver) = mpsc::channel(buffer_size);

        let processor = LevelFileSinkProcessor::new(self.config.clone(), receiver).await?;
        let handle = tokio::spawn(async move {
            if let Err(e) = processor.run().await {
                tracing::error!("LevelFileSink processor error: {}", e);
            }
        });

        self.sender = Some(sender);
        self.processor_handle = Some(handle);

        Ok(())
    }

    /// 发送事件
    pub async fn send_event_internal(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(sender) = &self.sender {
            let message = SinkMessage::Event(Box::new(event));

            // Using fixed blocking strategy since LevelFileConfig doesn't have backpressure field
            sender.send(message).await.map_err(|_| {
                QuantumLogError::SinkError("Failed to send event to LevelFileSink".to_string())
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
                return Err(QuantumLogError::SinkError(
                    "Failed to send shutdown signal".to_string(),
                ));
            }

            // 等待关闭完成
            match rx.await {
                Ok(result) => result?,
                Err(_) => {
                    return Err(QuantumLogError::SinkError(
                        "Shutdown signal lost".to_string(),
                    ))
                }
            }
        }

        // 等待处理器完成
        if let Some(handle) = self.processor_handle.take() {
            if let Err(e) = handle.await {
                tracing::error!("Error waiting for LevelFileSink processor: {}", e);
            }
        }

        Ok(())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.sender.is_some()
    }
}

impl LevelFileSinkProcessor {
    /// 创建新的处理器
    async fn new(config: LevelFileConfig, receiver: mpsc::Receiver<SinkMessage>) -> Result<Self> {
        // 使用 FileTools 确保目录存在
        FileTools::ensure_directory_exists(&config.directory)?;

        // 检查目录权限
        if !FileTools::is_directory_writable(&config.directory) {
            return Err(QuantumLogError::IoError {
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("目录不可写: {}", config.directory.display()),
                ),
            });
        }

        let path_generator = FilePathGenerator::new(&config.directory, "{level}.log".to_string());

        // LevelFileConfig doesn't have cleanup_enabled, max_files, base_path fields
        // Disabling file cleanup for now
        let cleaner = None;

        Ok(Self {
            config,
            receiver,
            writers: HashMap::new(),
            path_generator,
            cleaner,
        })
    }

    /// 运行处理器
    async fn run(mut self) -> Result<()> {
        while let Some(message) = self.receiver.recv().await {
            match message {
                SinkMessage::Event(event) => {
                    if let Err(e) = self.handle_event(*event).await {
                        tracing::error!("Error handling event in LevelFileSink: {}", e);
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
        // 检查是否应该记录此级别
        if !self.should_log_level(&event.level) {
            return Ok(());
        }

        // 获取或创建写入器
        self.get_or_create_writer(&event.level).await?;

        // 格式化事件
        let formatted = self.format_event(&event)?;

        // 获取写入器并写入文件
        let writer = self.writers.get_mut(&event.level).unwrap();
        writer.write(formatted.as_bytes()).await?;

        Ok(())
    }

    /// 获取或创建写入器
    async fn get_or_create_writer(&mut self, level: &str) -> Result<&mut FileWriter> {
        if !self.writers.contains_key(level) {
            let file_path = self.path_generator.generate_level_based_path(level);

            let file_config = crate::config::FileConfig {
                enabled: true,
                level: None,
                output_type: crate::config::FileOutputType::Text,
                directory: file_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf(),
                filename_base: file_path
                    .file_stem()
                    .unwrap_or_else(|| std::ffi::OsStr::new("log"))
                    .to_string_lossy()
                    .to_string(),
                extension: file_path
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_string()),
                separation_strategy: crate::config::FileSeparationStrategy::None,
                write_buffer_size: self.config.buffer_size,
                rotation: self.config.rotation.clone(),
                writer_cache_ttl_seconds: 300,
                writer_cache_capacity: 100,
            };

            let writer = FileWriter::new(file_config).await?;
            self.writers.insert(level.to_string(), writer);
        }

        Ok(self.writers.get_mut(level).unwrap())
    }

    /// 检查是否应该记录此级别
    fn should_log_level(&self, level: &str) -> bool {
        if let Some(ref levels) = self.config.levels {
            levels.contains(
                &level
                    .parse::<crate::config::LogLevel>()
                    .unwrap_or(crate::config::LogLevel::Info),
            )
        } else {
            true
        }
    }

    /// 格式化事件
    fn format_event(&self, event: &QuantumLogEvent) -> Result<String> {
        match self.config.format {
            crate::config::OutputFormat::Text => Ok(event.to_formatted_string("full")),
            crate::config::OutputFormat::Json => event
                .to_json()
                .map_err(|e| QuantumLogError::SerializationError { source: e }),
            crate::config::OutputFormat::Csv => {
                let csv_row = event.to_csv_row();
                Ok(csv_row.join(","))
            }
        }
    }

    /// 关闭处理器
    async fn shutdown(&mut self) -> Result<()> {
        // 刷新并关闭所有写入器
        for (level, writer) in &self.writers {
            if let Err(e) = writer.flush().await {
                tracing::error!("Error flushing writer for level {}: {}", level, e);
            }
        }
        
        // 显式关闭所有写入器并释放文件句柄
        self.writers.clear();
        
        // 关闭消息接收器
        self.receiver.close();
        
        // 处理并丢弃所有剩余的未处理消息
        while let Ok(msg) = self.receiver.try_recv() {
            match msg {
                SinkMessage::Event(_) => {
                    // 丢弃剩余事件
                }
                SinkMessage::Shutdown(sender) => {
                    // 响应关闭请求
                    let _ = sender.send(Ok(()));
                }
            }
        }

        // 执行清理（如果启用）
        if let Some(ref cleaner) = self.cleaner {
            match cleaner.cleanup().await {
                Ok(removed) => {
                    if removed > 0 {
                        tracing::info!("Cleaned up {} old log files", removed);
                    }
                }
                Err(e) => {
                    tracing::error!("Error during log file cleanup: {}", e);
                }
            }
        }
        
        tracing::info!("LevelFileSink shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LogLevel, OutputFormat};
    use crate::core::event::ContextInfo;
    use tempfile::TempDir;
    use tracing::Level;

    fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
        // Create a simple test event without metadata dependency
        let context = ContextInfo {
            pid: 1234,
            tid: 5678,
            username: Some("test_user".to_string()),
            hostname: Some("test_host".to_string()),
            mpi_rank: Some(0),
            custom_fields: HashMap::new(),
        };

        QuantumLogEvent {
            timestamp: chrono::Utc::now(),
            level: level.to_string(),
            target: "test_target".to_string(),
            message: message.to_string(),
            fields: HashMap::new(),
            file: Some("test.rs".to_string()),
            line: Some(42),
            module_path: Some("test::module".to_string()),
            thread_name: Some("test-thread".to_string()),
            thread_id: "test-thread-id".to_string(),
            context: context,
        }
    }

    #[tokio::test]
    async fn test_level_file_sink_creation() {
        let temp_dir = TempDir::new().unwrap();

        let config = LevelFileConfig {
            enabled: true,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "{level}".to_string(),
            extension: Some("log".to_string()),
            levels: Some(vec![LogLevel::Info, LogLevel::Error]),
            format: OutputFormat::Text,
            buffer_size: 8192,
            rotation: None,
        };

        let sink = LevelFileSink::new(config);
        assert!(!sink.is_running());
    }

    #[tokio::test]
    async fn test_level_file_sink_start_stop() {
        let temp_dir = TempDir::new().unwrap();

        let config = LevelFileConfig {
            enabled: true,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "{level}".to_string(),
            extension: Some("log".to_string()),
            levels: None,
            format: OutputFormat::Text,
            buffer_size: 8192,
            rotation: None,
        };

        let mut sink = LevelFileSink::new(config);

        // 启动
        let result = sink.start().await;
        assert!(result.is_ok());
        assert!(sink.is_running());

        // 关闭
        let result = sink.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_level_file_sink_send_events() {
        let temp_dir = TempDir::new().unwrap();

        let config = LevelFileConfig {
            enabled: true,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "{level}".to_string(),
            extension: Some("log".to_string()),
            levels: None,
            format: OutputFormat::Text,
            buffer_size: 8192,
            rotation: None,
        };

        let mut sink = LevelFileSink::new(config);
        sink.start().await.unwrap();

        // 发送不同级别的事件
        let info_event = create_test_event(Level::INFO, "Info message");
        let error_event = create_test_event(Level::ERROR, "Error message");

        sink.send_event(info_event).await.unwrap();
        sink.send_event(error_event).await.unwrap();

        // 等待一下让事件被处理
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        sink.shutdown().await.unwrap();

        // 检查文件是否被创建
        let info_file = temp_dir.path().join("INFO.log");
        let error_file = temp_dir.path().join("ERROR.log");

        assert!(info_file.exists());
        assert!(error_file.exists());

        // 检查文件内容
        let info_content = tokio::fs::read_to_string(info_file).await.unwrap();
        let error_content = tokio::fs::read_to_string(error_file).await.unwrap();

        assert!(info_content.contains("Info message"));
        assert!(error_content.contains("Error message"));
    }

    #[tokio::test]
    async fn test_level_filtering() {
        let temp_dir = TempDir::new().unwrap();

        let config = LevelFileConfig {
            enabled: true,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "{level}".to_string(),
            extension: Some("log".to_string()),
            levels: Some(vec![LogLevel::Error]), // 只记录 ERROR 级别
            format: OutputFormat::Text,
            buffer_size: 8192,
            rotation: None,
        };

        let mut sink = LevelFileSink::new(config);
        sink.start().await.unwrap();

        // 发送不同级别的事件
        let info_event = create_test_event(Level::INFO, "Info message");
        let error_event = create_test_event(Level::ERROR, "Error message");

        sink.send_event(info_event).await.unwrap();
        sink.send_event(error_event).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        sink.shutdown().await.unwrap();

        // 只有 ERROR 文件应该存在
        let info_file = temp_dir.path().join("INFO.log");
        let error_file = temp_dir.path().join("ERROR.log");

        assert!(!info_file.exists());
        assert!(error_file.exists());
    }

    #[tokio::test]
    async fn test_json_format() {
        let temp_dir = TempDir::new().unwrap();

        let config = LevelFileConfig {
            enabled: true,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "{level}".to_string(),
            extension: Some("log".to_string()),
            levels: None,
            format: OutputFormat::Json,
            buffer_size: 8192,
            rotation: None,
        };

        let mut sink = LevelFileSink::new(config);
        sink.start().await.unwrap();

        let event = create_test_event(Level::INFO, "Test message");
        sink.send_event(event).await.unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        sink.shutdown().await.unwrap();

        let file_content = tokio::fs::read_to_string(temp_dir.path().join("INFO.log"))
            .await
            .unwrap();

        // 应该是有效的 JSON
        let _: serde_json::Value = serde_json::from_str(&file_content).unwrap();
    }
}

// 实现新的统一 Sink trait
#[async_trait]
impl QuantumSink for LevelFileSink {
    type Config = LevelFileConfig;
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
        // 注意：这里需要可变引用，但trait要求不可变引用
        // 在实际使用中，可能需要使用内部可变性或重新设计
        Err(SinkError::Generic(
            "LevelFileSink shutdown requires mutable reference".to_string(),
        ))
    }

    async fn is_healthy(&self) -> bool {
        self.is_running()
    }

    fn name(&self) -> &'static str {
        "level_file"
    }

    fn stats(&self) -> String {
        format!(
            "LevelFileSink: running={}, directory={}, base={}",
            self.is_running(),
            self.config.directory.display(),
            self.config.filename_base
        )
    }

    fn metadata(&self) -> crate::sinks::traits::SinkMetadata {
        crate::sinks::traits::SinkMetadata {
            name: "level_file".to_string(),
            sink_type: crate::sinks::traits::SinkType::Exclusive,
            enabled: self.is_running(),
            description: Some(format!(
                "Level-based file sink writing to directory {} with base filename {}",
                self.config.directory.display(),
                self.config.filename_base
            )),
        }
    }
}

// 标记为独占型 sink
impl ExclusiveSink for LevelFileSink {}
