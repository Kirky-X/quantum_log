//! 滚动文件 Sink 实现
//!
//! 提供基于时间、大小等策略的文件滚动功能

use crate::config::RollingFileConfig;
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};
use crate::sinks::file_common::{FileWriter, FileCleaner, FilePathGenerator};
use crate::utils::FileTools;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// 滚动文件 Sink 消息类型
#[derive(Debug)]
enum SinkMessage {
    Event(Box<QuantumLogEvent>),
    Shutdown,
}

/// 滚动文件 Sink
///
/// 支持多种滚动策略的文件日志记录器
pub struct RollingFileSink {
    sender: Option<mpsc::UnboundedSender<SinkMessage>>,
    handle: Option<tokio::task::JoinHandle<()>>,
    config: RollingFileConfig,
}

impl RollingFileSink {
    /// 创建新的滚动文件 Sink
    pub fn new(config: RollingFileConfig) -> Self {
        Self {
            sender: None,
            handle: None,
            config,
        }
    }

    /// 启动 Sink
    pub async fn start(&mut self) -> Result<()> {
        if self.sender.is_some() {
            return Err(QuantumLogError::InternalError(
                "RollingFileSink already started".to_string(),
            ));
        }

        let (sender, receiver) = mpsc::unbounded_channel();
        let processor = RollingFileSinkProcessor::new(self.config.clone()).await?;
        let handle = tokio::spawn(async move {
            processor.run(receiver).await;
        });

        self.sender = Some(sender);
        self.handle = Some(handle);

        info!("RollingFileSink started with config: {:?}", self.config);
        Ok(())
    }

    /// 发送事件
    pub async fn send_event(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(sender) = &self.sender {
            sender
                .send(SinkMessage::Event(Box::new(event)))
                .map_err(|_| QuantumLogError::InternalError("Failed to send event".to_string()))?;
            Ok(())
        } else {
            Err(QuantumLogError::InternalError(
                "RollingFileSink not started".to_string(),
            ))
        }
    }

    /// 尝试发送事件（非阻塞）
    pub fn try_send_event(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(sender) = &self.sender {
            sender
                .send(SinkMessage::Event(Box::new(event)))
                .map_err(|_| QuantumLogError::InternalError("Failed to send event".to_string()))?;
            Ok(())
        } else {
            Err(QuantumLogError::InternalError(
                "RollingFileSink not started".to_string(),
            ))
        }
    }

    /// 优雅停机
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(SinkMessage::Shutdown);
        }

        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }

        info!("RollingFileSink shutdown completed");
        Ok(())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.sender.is_some()
    }

    /// 获取配置
    pub fn config(&self) -> &RollingFileConfig {
        &self.config
    }
}

/// 滚动文件 Sink 处理器
struct RollingFileSinkProcessor {
    config: RollingFileConfig,
    writers: Arc<RwLock<HashMap<String, Arc<Mutex<FileWriter>>>>>,
    path_generator: FilePathGenerator,
    cleaner: Option<FileCleaner>,
    last_cleanup: Arc<Mutex<Instant>>,
}

impl RollingFileSinkProcessor {
    /// 创建新的处理器
    async fn new(config: RollingFileConfig) -> Result<Self> {
        // 确保目录存在
        FileTools::ensure_directory_exists(&config.directory)?;
        
        // 检查目录权限
        if !FileTools::is_directory_writable(&config.directory) {
            return Err(QuantumLogError::IoError {
                source: std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("目录不可写: {}", config.directory.display())
                )
            });
        }

        let pattern = match config.separation_strategy {
            crate::config::FileSeparationStrategy::ByPid => {
                format!("{}_{{pid}}.{}", config.filename_base, config.extension.as_deref().unwrap_or("log"))
            }
            crate::config::FileSeparationStrategy::ByTid => {
                format!("{}_{{tid}}.{}", config.filename_base, config.extension.as_deref().unwrap_or("log"))
            }
            crate::config::FileSeparationStrategy::Level => {
                format!("{}_{{level}}.{}", config.filename_base, config.extension.as_deref().unwrap_or("log"))
            }
            crate::config::FileSeparationStrategy::Module => {
                format!("{}_{{module}}.{}", config.filename_base, config.extension.as_deref().unwrap_or("log"))
            }
            crate::config::FileSeparationStrategy::Time => {
                format!("{}_{{time}}.{}", config.filename_base, config.extension.as_deref().unwrap_or("log"))
            }
            _ => {
                format!("{}.{}", config.filename_base, config.extension.as_deref().unwrap_or("log"))
            }
        };
        
        let path_generator = FilePathGenerator::new(
            &config.directory,
            pattern,
        );

        let cleaner = if let Some(ref rotation) = config.rotation {
            let mut cleaner = FileCleaner::new(&config.directory);
            if let Some(max_files) = rotation.max_files {
                cleaner = cleaner.max_files(max_files);
            }
            // 使用默认的7天清理策略
            cleaner = cleaner.max_age(chrono::Duration::days(7));
            Some(cleaner)
        } else {
            None
        };

        Ok(Self {
            config,
            writers: Arc::new(RwLock::new(HashMap::new())),
            path_generator,
            cleaner,
            last_cleanup: Arc::new(Mutex::new(Instant::now())),
        })
    }

    /// 运行处理器
    async fn run(self, mut receiver: mpsc::UnboundedReceiver<SinkMessage>) {
        while let Some(message) = receiver.recv().await {
            match message {
                SinkMessage::Event(event) => {
                    if let Err(e) = self.handle_event(*event).await {
                        error!("Failed to handle event: {}", e);
                    }
                }
                SinkMessage::Shutdown => {
                    debug!("Received shutdown signal");
                    break;
                }
            }
        }

        // 执行清理
        self.shutdown().await;
    }

    /// 处理事件
    async fn handle_event(&self, event: QuantumLogEvent) -> Result<()> {
        // 级别过滤
        if let Some(min_level) = &self.config.level {
            if event.level < *min_level {
                return Ok(());
            }
        }

        // 生成文件路径
        let file_key = self.generate_file_key(&event);
        let file_path = self.generate_file_path(&event, &file_key)?;

        // 获取或创建写入器
        let writer = self.get_or_create_writer(&file_key, &file_path).await?;

        // 格式化并写入事件
        let formatted = self.format_event(&event)?;
        {
            let writer_guard = writer.lock().await;
            writer_guard.write(formatted.as_bytes()).await?;
        }

        // 定期清理
        self.periodic_cleanup().await;

        Ok(())
    }

    /// 生成文件键
    fn generate_file_key(&self, event: &QuantumLogEvent) -> String {
        match &self.config.separation_strategy {
            crate::config::FileSeparationStrategy::None => "default".to_string(),
            crate::config::FileSeparationStrategy::ByPid => format!("pid_{}", std::process::id()),
            crate::config::FileSeparationStrategy::ByTid => {
                format!("tid_{:?}", std::thread::current().id())
            }
            crate::config::FileSeparationStrategy::ByMpiRank => {
                "mpi_rank".to_string()
            }
            crate::config::FileSeparationStrategy::Level => event.level.to_string(),
            crate::config::FileSeparationStrategy::Module => {
                event.module_path.as_deref().unwrap_or("unknown").to_string()
            }
            crate::config::FileSeparationStrategy::Time => {
                event.timestamp.format("%Y%m%d_%H").to_string()
            }
        }
    }

    /// 生成文件路径
    fn generate_file_path(&self, event: &QuantumLogEvent, _file_key: &str) -> Result<PathBuf> {
        match &self.config.separation_strategy {
            crate::config::FileSeparationStrategy::None => {
                let filename = format!("{}.{}", self.config.filename_base, self.config.extension.as_deref().unwrap_or("log"));
                Ok(self.config.directory.join(filename))
            }
            crate::config::FileSeparationStrategy::ByPid => {
                let filename = format!("{}_{}.{}", self.config.filename_base, std::process::id(), self.config.extension.as_deref().unwrap_or("log"));
                Ok(self.config.directory.join(filename))
            }
            crate::config::FileSeparationStrategy::ByTid => {
                let filename = format!("{}_{:?}.{}", self.config.filename_base, std::thread::current().id(), self.config.extension.as_deref().unwrap_or("log"));
                Ok(self.config.directory.join(filename))
            }
            crate::config::FileSeparationStrategy::ByMpiRank => {
                let filename = format!("{}_mpi.{}", self.config.filename_base, self.config.extension.as_deref().unwrap_or("log"));
                Ok(self.config.directory.join(filename))
            }
            crate::config::FileSeparationStrategy::Level => {
                Ok(self.path_generator.generate_level_based_path(&event.level.to_string()))
            }
            crate::config::FileSeparationStrategy::Module => {
                let module = event.module_path.as_deref().unwrap_or("unknown");
                Ok(self.path_generator.generate_module_based_path(module))
            }
            crate::config::FileSeparationStrategy::Time => {
                Ok(self.path_generator.generate_time_based_path(event.timestamp))
            }
        }
    }

    /// 获取或创建写入器
    async fn get_or_create_writer(
        &self,
        file_key: &str,
        file_path: &Path,
    ) -> Result<Arc<Mutex<FileWriter>>> {
        // 首先尝试读锁
        {
            let readers = self.writers.read().await;
            if let Some(writer) = readers.get(file_key) {
                return Ok(writer.clone());
            }
        }

        // 需要创建新的写入器，获取写锁
        let mut writers = self.writers.write().await;
        
        // 双重检查
        if let Some(writer) = writers.get(file_key) {
            return Ok(writer.clone());
        }

        // 创建文件配置
        let file_config = crate::config::FileConfig {
            enabled: true,
            level: self.config.level.clone(),
            output_type: self.config.output_type.clone(),
            directory: file_path.parent().unwrap_or(&self.config.directory).to_path_buf(),
            filename_base: file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("quantum")
                .to_string(),
            extension: file_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string()),
            separation_strategy: self.config.separation_strategy.clone(),
            write_buffer_size: self.config.write_buffer_size,
            rotation: self.config.rotation.clone(),
            writer_cache_ttl_seconds: self.config.writer_cache_ttl_seconds,
            writer_cache_capacity: self.config.writer_cache_capacity,
        };

        let writer = FileWriter::new(file_config).await?;
        let writer_arc = Arc::new(Mutex::new(writer));
        writers.insert(file_key.to_string(), writer_arc.clone());

        Ok(writer_arc)
    }

    /// 格式化事件
    fn format_event(&self, event: &QuantumLogEvent) -> Result<String> {
        match &self.config.output_type {
            crate::config::FileOutputType::Json => {
                serde_json::to_string(event)
                    .map(|s| format!("{}\n", s))
                    .map_err(|e| QuantumLogError::InternalError(format!("JSON serialization failed: {}", e)))
            }
            crate::config::FileOutputType::Text => {
                Ok(format!(
                    "[{}] {} [{}:{}] {}: {}\n",
                    event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                    event.level,
                    event.module_path.as_deref().unwrap_or("unknown"),
                    event.line.unwrap_or(0),
                    event.target,
                    event.message
                ))
            }
            crate::config::FileOutputType::Csv => {
                Ok(format!(
                    "{},{},{},{},{},\"{}\"\n",
                    event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                    event.level,
                    event.target,
                    event.module_path.as_deref().unwrap_or("unknown"),
                    event.line.unwrap_or(0),
                    event.message.replace('"', "\"\"")
                ))
            }
        }
    }

    /// 定期清理
    async fn periodic_cleanup(&self) {
        if let Some(cleaner) = &self.cleaner {
            let mut last_cleanup = self.last_cleanup.lock().await;
            let now = Instant::now();
            
            // 每小时执行一次清理
            if now.duration_since(*last_cleanup) > Duration::from_secs(3600) {
                match cleaner.cleanup().await {
                    Ok(removed_count) => {
                        if removed_count > 0 {
                            info!("Cleaned up {} old log files", removed_count);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to cleanup old log files: {}", e);
                    }
                }
                *last_cleanup = now;
            }
        }
    }

    /// 停机清理
    async fn shutdown(&self) {
        // 刷新所有写入器
        let writers = self.writers.read().await;
        for (key, writer) in writers.iter() {
            let writer_guard = writer.lock().await;
            if let Err(e) = writer_guard.flush().await {
                warn!("Failed to flush writer {}: {}", key, e);
            }
        }
        drop(writers);

        // 执行最终清理
        if let Some(cleaner) = &self.cleaner {
            if let Err(e) = cleaner.cleanup().await {
                warn!("Failed to perform final cleanup: {}", e);
            }
        }

        debug!("RollingFileSinkProcessor shutdown completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FileSeparationStrategy, FileOutputType};
    use tempfile::TempDir;
    use tracing::Level;

    fn create_test_event() -> QuantumLogEvent {
        use crate::core::event::ContextInfo;
        QuantumLogEvent {
            timestamp: chrono::Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "Test message".to_string(),
            module_path: Some("test::module".to_string()),
            file: Some("test.rs".to_string()),
            line: Some(42),
            fields: std::collections::HashMap::new(),
            context: ContextInfo {
                pid: std::process::id(),
                tid: 0,
                username: None,
                hostname: None,
                mpi_rank: None,
                custom_fields: std::collections::HashMap::new(),
            },
        }
    }

    #[tokio::test]
    async fn test_rolling_file_sink_creation() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = RollingFileConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            output_type: FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "app".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };
        
        let sink = RollingFileSink::new(config);
        assert!(!sink.is_running());
    }

    #[tokio::test]
    async fn test_rolling_file_sink_start_stop() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = RollingFileConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            output_type: FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "app".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };
        
        let mut sink = RollingFileSink::new(config);
        
        // 启动
        assert!(sink.start().await.is_ok());
        assert!(sink.is_running());
        
        // 停止
        assert!(sink.shutdown().await.is_ok());
        assert!(!sink.is_running());
    }

    #[tokio::test]
    async fn test_rolling_file_sink_send_event() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = RollingFileConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            output_type: FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "app".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };
        
        let mut sink = RollingFileSink::new(config);
        assert!(sink.start().await.is_ok());
        
        let event = create_test_event();
        assert!(sink.send_event(event).await.is_ok());
        
        // 给一些时间让事件被处理
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        assert!(sink.shutdown().await.is_ok());
        
        // 验证文件是否被创建
        let log_file = temp_dir.path().join("app.log");
        assert!(log_file.exists());
    }

    #[tokio::test]
    async fn test_level_separation() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = RollingFileConfig {
            enabled: true,
            level: Some("DEBUG".to_string()),
            output_type: FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "app".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: FileSeparationStrategy::Level,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };
        
        let mut sink = RollingFileSink::new(config);
        assert!(sink.start().await.is_ok());
        
        // 发送不同级别的事件
        let mut info_event = create_test_event();
        info_event.level = "INFO".to_string();
        assert!(sink.send_event(info_event).await.is_ok());
        
        let mut error_event = create_test_event();
        error_event.level = "ERROR".to_string();
        assert!(sink.send_event(error_event).await.is_ok());
        
        // 给一些时间让事件被处理
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        assert!(sink.shutdown().await.is_ok());
        
        // 验证不同级别的文件是否被创建
        let info_file = temp_dir.path().join("app_INFO.log");
        let error_file = temp_dir.path().join("app_ERROR.log");
        
        assert!(info_file.exists());
        assert!(error_file.exists());
    }
}