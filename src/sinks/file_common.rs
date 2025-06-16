//! 文件 Sink 通用功能
//!
//! 此模块提供文件 sink 的通用功能，包括文件轮转、路径管理等。

use crate::config::FileConfig;
use crate::error::{QuantumLogError, Result};
use crate::utils::FileTools;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 文件写入器
///
/// 封装文件写入操作，支持缓冲和轮转
pub struct FileWriter {
    /// 当前文件路径
    current_path: PathBuf,
    /// 文件写入器
    writer: Arc<Mutex<BufWriter<File>>>,
    /// 文件配置
    config: FileConfig,
    /// 自动刷新
    auto_flush: bool,
    /// 是否归档
    archive: bool,
    /// 当前文件大小
    current_size: Arc<Mutex<u64>>,
    /// 文件创建时间
    created_at: chrono::DateTime<chrono::Utc>,
}

impl FileWriter {
    /// 创建新的文件写入器
    pub async fn new(config: FileConfig) -> Result<Self> {
        let path = Self::generate_file_path(&config)?;

        // 使用 FileTools 确保目录存在
        if let Some(parent) = path.parent() {
            FileTools::ensure_directory_exists(parent)?;

            // 检查目录是否可写
            if !FileTools::is_directory_writable(parent) {
                return Err(QuantumLogError::IoError {
                    source: std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        format!("目录不可写: {}", parent.display()),
                    ),
                });
            }
        }

        // 打开文件
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| {
                QuantumLogError::InternalError(format!(
                    "Failed to open file {}: {}",
                    path.display(),
                    e
                ))
            })?;

        let writer = BufWriter::new(file);
        let current_size = tokio::fs::metadata(&path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(Self {
            current_path: path,
            writer: Arc::new(Mutex::new(writer)),
            auto_flush: config.write_buffer_size == 0,
            archive: true,
            config,
            current_size: Arc::new(Mutex::new(current_size)),
            created_at: chrono::Utc::now(),
        })
    }

    /// 写入数据
    pub async fn write(&self, data: &[u8]) -> Result<()> {
        // 检查是否需要轮转
        let _rotated = if self.should_rotate(data.len()).await? {
            self.rotate().await?;
            true
        } else {
            false
        };

        // 写入数据
        {
            let mut writer = self.writer.lock().await;
            writer.write_all(data).map_err(|e| {
                QuantumLogError::InternalError(format!("Failed to write to file: {}", e))
            })?;

            if self.auto_flush {
                writer.flush().map_err(|e| {
                    QuantumLogError::InternalError(format!("Failed to flush file: {}", e))
                })?;
            }
        }

        // 更新文件大小
        {
            let mut size = self.current_size.lock().await;
            *size += data.len() as u64;
        }

        Ok(())
    }

    /// 刷新缓冲区
    pub async fn flush(&self) -> Result<()> {
        let mut writer = self.writer.lock().await;
        writer
            .flush()
            .map_err(|e| QuantumLogError::InternalError(format!("Failed to flush file: {}", e)))
    }

    /// 检查是否需要轮转
    async fn should_rotate(&self, additional_size: usize) -> Result<bool> {
        match &self.config.rotation {
            Some(rotation_config) => {
                if let Some(max_size_mb) = rotation_config.max_size_mb {
                    let current_size = *self.current_size.lock().await;
                    let max_size_bytes = max_size_mb * 1024 * 1024; // 转换为字节
                    if current_size + additional_size as u64 > max_size_bytes {
                        return Ok(true);
                    }
                }
                // 注意：RotationConfig 没有 max_age 字段，这里暂时移除时间轮转检查
                Ok(false)
            }
            None => Ok(false),
        }
    }

    /// 执行文件轮转
    async fn rotate(&self) -> Result<()> {
        // 刷新当前文件
        self.flush().await?;

        // 生成新的文件路径
        let new_path = Self::generate_file_path(&self.config)?;

        // 如果启用了归档，重命名当前文件
        if self.archive {
            let archive_path = self.generate_archive_path()?;

            // 使用 FileTools 安全地移动文件
            if FileTools::file_exists(&self.current_path) {
                tokio::fs::rename(&self.current_path, &archive_path)
                    .await
                    .map_err(|e| {
                        QuantumLogError::InternalError(format!("Failed to archive file: {}", e))
                    })?;
            }
        }

        // 打开新文件
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&new_path)
            .map_err(|e| {
                QuantumLogError::InternalError(format!(
                    "Failed to create new file {}: {}",
                    new_path.display(),
                    e
                ))
            })?;

        // 更新写入器
        {
            let mut writer = self.writer.lock().await;
            *writer = BufWriter::new(file);
        }

        // 重置状态
        {
            let mut size = self.current_size.lock().await;
            *size = 0;
        }

        Ok(())
    }

    /// 生成文件路径
    fn generate_file_path(config: &FileConfig) -> Result<PathBuf> {
        let mut path = PathBuf::from(&config.directory);

        // 构建文件名
        let filename = if let Some(ref ext) = config.extension {
            format!("{}.{}", config.filename_base, ext)
        } else {
            config.filename_base.clone()
        };

        path.push(filename);

        Ok(path)
    }

    /// 生成归档文件路径
    fn generate_archive_path(&self) -> Result<PathBuf> {
        let current_path = &self.current_path;
        let timestamp = self.created_at.format("%Y%m%d_%H%M%S");

        let file_stem = current_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("quantum");

        let extension = current_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("log");

        let archive_name = format!("{}_{}.{}", file_stem, timestamp, extension);

        let mut archive_path = current_path.clone();
        archive_path.set_file_name(archive_name);

        Ok(archive_path)
    }

    /// 获取当前文件路径
    pub fn current_path(&self) -> &Path {
        &self.current_path
    }

    /// 获取当前文件大小
    pub async fn current_size(&self) -> u64 {
        *self.current_size.lock().await
    }
}

/// 文件路径生成器
///
/// 根据不同的策略生成文件路径
pub struct FilePathGenerator {
    base_path: PathBuf,
    pattern: String,
}

impl FilePathGenerator {
    /// 创建新的路径生成器
    pub fn new<P: AsRef<Path>>(base_path: P, pattern: String) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            pattern,
        }
    }

    /// 生成基于时间的文件路径
    pub fn generate_time_based_path(&self, timestamp: chrono::DateTime<chrono::Utc>) -> PathBuf {
        let formatted = timestamp.format(&self.pattern).to_string();
        self.base_path.join(formatted)
    }

    /// 生成基于级别的文件路径
    pub fn generate_level_based_path(&self, level: &str) -> PathBuf {
        let filename = self.pattern.replace("{level}", level);
        self.base_path.join(filename)
    }

    /// 生成基于模块的文件路径
    pub fn generate_module_based_path(&self, module: &str) -> PathBuf {
        let safe_module = module.replace("::", "_");
        let filename = self.pattern.replace("{module}", &safe_module);
        self.base_path.join(filename)
    }
}

/// 文件清理器
///
/// 负责清理旧的日志文件
pub struct FileCleaner {
    base_path: PathBuf,
    max_files: Option<usize>,
    max_age: Option<chrono::Duration>,
}

impl FileCleaner {
    /// 创建新的文件清理器
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            max_files: None,
            max_age: None,
        }
    }

    /// 设置最大文件数量
    pub fn max_files(mut self, max_files: usize) -> Self {
        self.max_files = Some(max_files);
        self
    }

    /// 设置最大文件年龄
    pub fn max_age(mut self, max_age: chrono::Duration) -> Self {
        self.max_age = Some(max_age);
        self
    }

    /// 执行清理
    pub async fn cleanup(&self) -> Result<usize> {
        let mut removed_count = 0;

        // 读取目录中的所有文件
        let mut entries = tokio::fs::read_dir(&self.base_path).await.map_err(|e| {
            QuantumLogError::InternalError(format!("Failed to read directory: {}", e))
        })?;

        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            QuantumLogError::InternalError(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if path.is_file() {
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        files.push((path, modified));
                    }
                }
            }
        }

        // 按修改时间排序（最新的在前）
        files.sort_by(|a, b| b.1.cmp(&a.1));

        let now = std::time::SystemTime::now();

        for (i, (path, modified)) in files.iter().enumerate() {
            let mut should_remove = false;

            // 检查文件数量限制
            if let Some(max_files) = self.max_files {
                if i >= max_files {
                    should_remove = true;
                }
            }

            // 检查文件年龄限制
            if let Some(max_age) = self.max_age {
                if let Ok(elapsed) = now.duration_since(*modified) {
                    let elapsed_chrono = chrono::Duration::from_std(elapsed).unwrap_or_default();
                    if elapsed_chrono > max_age {
                        should_remove = true;
                    }
                }
            }

            if should_remove {
                // 使用 FileTools 安全地删除文件
                if FileTools::file_exists(path) {
                    if let Err(e) = FileTools::remove_file_safe(path) {
                        tracing::warn!("Failed to remove old log file {}: {}", path.display(), e);
                    } else {
                        removed_count += 1;
                        tracing::debug!("Removed old log file: {}", path.display());
                    }
                }
            }
        }

        Ok(removed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_writer_creation() {
        let temp_dir = TempDir::new().unwrap();

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

        let writer = FileWriter::new(config).await;
        assert!(writer.is_ok());
    }

    #[tokio::test]
    async fn test_file_writer_write() {
        let temp_dir = TempDir::new().unwrap();

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

        let writer = FileWriter::new(config).await.unwrap();

        let data = b"Test log message\n";
        let result = writer.write(data).await;
        assert!(result.is_ok());

        assert_eq!(writer.current_size().await, data.len() as u64);
    }

    #[tokio::test]
    async fn test_file_rotation_by_size() {
        let temp_dir = TempDir::new().unwrap();

        let config = FileConfig {
            enabled: true,
            level: None,
            output_type: crate::config::FileOutputType::Text,
            directory: temp_dir.path().to_path_buf(),
            filename_base: "test".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: crate::config::FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: Some(crate::config::RotationConfig {
                strategy: crate::config::RotationStrategy::Size,
                max_size_mb: Some(1),
                max_files: None,
                compress_rotated_files: false,
            }),
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 100,
        };

        let writer = FileWriter::new(config).await.unwrap();

        // 写入足够大的数据触发轮转（超过1MB）
        let large_data = vec![b'A'; 1024 * 1024 + 100]; // 1MB + 100字节

        let result = writer.write(&large_data).await;
        assert!(result.is_ok());

        let final_size = writer.current_size().await;

        // 轮转后，数据被写入新文件，所以新文件的大小应该等于数据大小
        // 这证明轮转成功发生了
        assert_eq!(
            final_size,
            large_data.len() as u64,
            "Expected final_size ({}) == data_len ({})",
            final_size,
            large_data.len()
        );

        // 验证轮转确实发生了 - 检查是否有归档文件存在
        let entries = tokio::fs::read_dir(temp_dir.path()).await.unwrap();
        let mut file_count = 0;
        let mut entries = entries;
        while let Some(entry) = entries.next_entry().await.unwrap() {
            if entry.file_type().await.unwrap().is_file() {
                file_count += 1;
            }
        }
        // 应该有2个文件：当前文件和归档文件
        assert!(
            file_count >= 2,
            "Expected at least 2 files (current + archived), found {}",
            file_count
        );
    }

    #[test]
    fn test_path_generator() {
        let generator = FilePathGenerator::new("/logs", "app_{level}.log".to_string());

        let path = generator.generate_level_based_path("INFO");
        assert_eq!(path, PathBuf::from("/logs/app_INFO.log"));

        let generator = FilePathGenerator::new("/logs", "app_{module}.log".to_string());
        let path = generator.generate_module_based_path("my::module");
        assert_eq!(path, PathBuf::from("/logs/app_my_module.log"));
    }

    #[tokio::test]
    async fn test_file_cleaner() {
        let temp_dir = TempDir::new().unwrap();

        // 创建一些测试文件
        for i in 0..5 {
            let file_path = temp_dir.path().join(format!("test_{}.log", i));
            tokio::fs::write(&file_path, format!("Test content {}", i))
                .await
                .unwrap();
        }

        let cleaner = FileCleaner::new(temp_dir.path()).max_files(3);
        let removed = cleaner.cleanup().await.unwrap();

        assert_eq!(removed, 2); // 应该删除2个文件，保留3个
    }
}
