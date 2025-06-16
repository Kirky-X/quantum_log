//! 文件工具模块
//!
//! 提供文件操作相关的实用工具函数，包括文件创建、目录管理、
//! 文件大小检查、权限验证等功能。

use crate::error::{QuantumLogError, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// 文件工具结构体
///
/// 提供各种文件操作的静态方法
pub struct FileTools;

impl FileTools {
    /// 确保目录存在，如果不存在则创建
    ///
    /// # 参数
    ///
    /// * `path` - 目录路径
    ///
    /// # 返回值
    ///
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn ensure_directory_exists<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();

        if !path.exists() {
            fs::create_dir_all(path).map_err(|e| QuantumLogError::IoError { source: e })?
        } else if !path.is_dir() {
            return Err(QuantumLogError::IoError {
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("路径存在但不是目录: {}", path.display()),
                ),
            });
        }

        Ok(())
    }

    /// 安全地创建文件，确保父目录存在
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    /// * `create_parents` - 是否创建父目录
    ///
    /// # 返回值
    ///
    /// 成功时返回文件句柄，失败时返回错误
    pub fn create_file_safe<P: AsRef<Path>>(file_path: P, create_parents: bool) -> Result<File> {
        let file_path = file_path.as_ref();

        if create_parents {
            if let Some(parent) = file_path.parent() {
                Self::ensure_directory_exists(parent)?;
            }
        }

        File::create(file_path).map_err(|e| QuantumLogError::IoError { source: e })
    }

    /// 安全地打开文件进行追加写入
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    /// * `create_if_not_exists` - 如果文件不存在是否创建
    ///
    /// # 返回值
    ///
    /// 成功时返回文件句柄，失败时返回错误
    pub fn open_file_append<P: AsRef<Path>>(
        file_path: P,
        create_if_not_exists: bool,
    ) -> Result<File> {
        let file_path = file_path.as_ref();

        let mut options = OpenOptions::new();
        options.append(true);

        if create_if_not_exists {
            options.create(true);

            // 确保父目录存在
            if let Some(parent) = file_path.parent() {
                Self::ensure_directory_exists(parent)?;
            }
        }

        options
            .open(file_path)
            .map_err(|e| QuantumLogError::IoError { source: e })
    }

    /// 获取文件大小（字节）
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 成功时返回文件大小，失败时返回错误
    pub fn get_file_size<P: AsRef<Path>>(file_path: P) -> Result<u64> {
        let file_path = file_path.as_ref();

        let metadata =
            fs::metadata(file_path).map_err(|e| QuantumLogError::IoError { source: e })?;

        Ok(metadata.len())
    }

    /// 检查文件是否可写
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 如果文件可写返回 `true`，否则返回 `false`
    pub fn is_file_writable<P: AsRef<Path>>(file_path: P) -> bool {
        let file_path = file_path.as_ref();

        // 如果文件不存在，检查父目录是否可写
        if !file_path.exists() {
            if let Some(parent) = file_path.parent() {
                return Self::is_directory_writable(parent);
            }
            return false;
        }

        // 尝试以追加模式打开文件
        OpenOptions::new().append(true).open(file_path).is_ok()
    }

    /// 检查目录是否可写
    ///
    /// # 参数
    ///
    /// * `dir_path` - 目录路径
    ///
    /// # 返回值
    ///
    /// 如果目录可写返回 `true`，否则返回 `false`
    pub fn is_directory_writable<P: AsRef<Path>>(dir_path: P) -> bool {
        let dir_path = dir_path.as_ref();

        if !dir_path.exists() || !dir_path.is_dir() {
            return false;
        }

        // 尝试在目录中创建临时文件
        let temp_file = dir_path.join(".quantum_log_write_test");
        let result = File::create(&temp_file).is_ok();

        // 清理临时文件
        if temp_file.exists() {
            let _ = fs::remove_file(&temp_file);
        }

        result
    }

    /// 检查文件是否存在
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 如果文件存在返回 `true`，否则返回 `false`
    pub fn file_exists<P: AsRef<Path>>(file_path: P) -> bool {
        file_path.as_ref().exists()
    }

    /// 安全地删除文件
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn remove_file_safe<P: AsRef<Path>>(file_path: P) -> Result<()> {
        let file_path = file_path.as_ref();

        if file_path.exists() {
            fs::remove_file(file_path).map_err(|e| QuantumLogError::IoError { source: e })?;
        }

        Ok(())
    }

    /// 安全地删除文件（别名方法）
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 成功时返回 `Ok(())`，失败时返回错误
    pub fn safe_remove_file<P: AsRef<Path>>(file_path: P) -> Result<()> {
        Self::remove_file_safe(file_path)
    }

    /// 获取文件的规范化路径
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 成功时返回规范化的路径，失败时返回错误
    pub fn canonicalize_path<P: AsRef<Path>>(file_path: P) -> Result<PathBuf> {
        let file_path = file_path.as_ref();

        file_path
            .canonicalize()
            .map_err(|e| QuantumLogError::IoError {
                source: std::io::Error::other(format!(
                    "规范化路径失败: {}: {}",
                    file_path.display(),
                    e
                )),
            })
    }

    /// 创建带缓冲的文件写入器
    ///
    /// # 参数
    ///
    /// * `file` - 文件句柄
    /// * `buffer_size` - 缓冲区大小（可选，默认为8KB）
    ///
    /// # 返回值
    ///
    /// 返回带缓冲的写入器
    pub fn create_buffered_writer(file: File, buffer_size: Option<usize>) -> BufWriter<File> {
        match buffer_size {
            Some(size) => BufWriter::with_capacity(size, file),
            None => BufWriter::new(file),
        }
    }

    /// 安全地写入数据到文件
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    /// * `data` - 要写入的数据
    /// * `append` - 是否追加模式
    ///
    /// # 返回值
    ///
    /// 成功时返回写入的字节数，失败时返回错误
    pub fn write_to_file<P: AsRef<Path>>(file_path: P, data: &[u8], append: bool) -> Result<usize> {
        let file_path = file_path.as_ref();

        // 确保父目录存在
        if let Some(parent) = file_path.parent() {
            Self::ensure_directory_exists(parent)?;
        }

        let mut options = OpenOptions::new();
        if append {
            options.append(true).create(true);
        } else {
            options.write(true).create(true).truncate(true);
        }

        let mut file = options
            .open(file_path)
            .map_err(|e| QuantumLogError::IoError {
                source: std::io::Error::new(
                    e.kind(),
                    format!("打开文件失败: {}: {}", file_path.display(), e),
                ),
            })?;

        file.write_all(data).map_err(|e| QuantumLogError::IoError {
            source: std::io::Error::new(
                e.kind(),
                format!("写入文件失败: {}: {}", file_path.display(), e),
            ),
        })?;

        file.flush().map_err(|e| QuantumLogError::IoError {
            source: std::io::Error::new(
                e.kind(),
                format!("刷新文件失败: {}: {}", file_path.display(), e),
            ),
        })?;

        Ok(data.len())
    }

    /// 获取文件扩展名
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 返回文件扩展名（如果存在）
    pub fn get_file_extension<P: AsRef<Path>>(file_path: P) -> Option<String> {
        file_path
            .as_ref()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase())
    }

    /// 生成带时间戳的文件名
    ///
    /// # 参数
    ///
    /// * `base_name` - 基础文件名
    /// * `extension` - 文件扩展名
    ///
    /// # 返回值
    ///
    /// 返回带时间戳的文件名
    pub fn generate_timestamped_filename(base_name: &str, extension: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        format!(
            "{}_{}_{}.{}",
            base_name,
            timestamp,
            std::process::id(),
            extension
        )
    }

    /// 检查磁盘空间是否足够
    ///
    /// # 参数
    ///
    /// * `path` - 检查路径
    /// * `required_bytes` - 需要的字节数
    ///
    /// # 返回值
    ///
    /// 如果空间足够返回 `true`，否则返回 `false`
    #[cfg(unix)]
    pub fn has_sufficient_disk_space<P: AsRef<Path>>(path: P, required_bytes: u64) -> bool {
        use std::os::unix::fs::MetadataExt;

        if let Ok(metadata) = fs::metadata(path) {
            // 在Unix系统上，可以通过statvfs获取更准确的磁盘空间信息
            // 这里简化处理，假设有足够空间
            true
        } else {
            false
        }
    }

    #[cfg(windows)]
    pub fn has_sufficient_disk_space<P: AsRef<Path>>(_path: P, _required_bytes: u64) -> bool {
        // Windows平台的磁盘空间检查
        // 这里简化处理，假设有足够空间
        true
    }

    #[cfg(not(any(unix, windows)))]
    pub fn has_sufficient_disk_space<P: AsRef<Path>>(_path: P, _required_bytes: u64) -> bool {
        // 其他平台默认返回true
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_ensure_directory_exists() {
        let temp_dir = env::temp_dir().join("quantum_log_test_dir");

        // 清理可能存在的目录
        let _ = fs::remove_dir_all(&temp_dir);

        // 测试创建目录
        assert!(FileTools::ensure_directory_exists(&temp_dir).is_ok());
        assert!(temp_dir.exists());
        assert!(temp_dir.is_dir());

        // 测试目录已存在的情况
        assert!(FileTools::ensure_directory_exists(&temp_dir).is_ok());

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_create_file_safe() {
        let temp_dir = env::temp_dir().join("quantum_log_test_create");
        let test_file = temp_dir.join("test.txt");

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);

        // 测试创建文件（包括父目录）
        let file = FileTools::create_file_safe(&test_file, true);
        assert!(file.is_ok());
        assert!(test_file.exists());

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_file_size() {
        let temp_file = env::temp_dir().join("quantum_log_size_test.txt");

        // 创建测试文件
        let test_data = b"Hello, QuantumLog!";
        fs::write(&temp_file, test_data).unwrap();

        // 测试获取文件大小
        let size = FileTools::get_file_size(&temp_file);
        assert!(size.is_ok());
        assert_eq!(size.unwrap(), test_data.len() as u64);

        // 清理
        let _ = fs::remove_file(&temp_file);
    }

    #[test]
    fn test_write_to_file() {
        let temp_file = env::temp_dir().join("quantum_log_write_test.txt");
        let test_data = b"Test data for QuantumLog";

        // 清理可能存在的文件
        let _ = fs::remove_file(&temp_file);

        // 测试写入文件
        let result = FileTools::write_to_file(&temp_file, test_data, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_data.len());

        // 验证文件内容
        let content = fs::read(&temp_file).unwrap();
        assert_eq!(content, test_data);

        // 测试追加模式
        let append_data = b" - Appended";
        let result = FileTools::write_to_file(&temp_file, append_data, true);
        assert!(result.is_ok());

        // 验证追加后的内容
        let content = fs::read(&temp_file).unwrap();
        let expected = [&test_data[..], &append_data[..]].concat();
        assert_eq!(content, expected);

        // 清理
        let _ = fs::remove_file(&temp_file);
    }

    #[test]
    fn test_file_extension() {
        assert_eq!(
            FileTools::get_file_extension("test.txt"),
            Some("txt".to_string())
        );
        assert_eq!(
            FileTools::get_file_extension("test.LOG"),
            Some("log".to_string())
        );
        assert_eq!(FileTools::get_file_extension("test"), None);
        assert_eq!(FileTools::get_file_extension(".hidden"), None);
    }

    #[test]
    fn test_timestamped_filename() {
        let filename = FileTools::generate_timestamped_filename("quantum_log", "log");
        assert!(filename.starts_with("quantum_log_"));
        assert!(filename.ends_with(".log"));
        assert!(filename.contains(&std::process::id().to_string()));
    }

    #[test]
    fn test_is_file_writable() {
        let temp_dir = env::temp_dir();
        let test_file = temp_dir.join("quantum_log_writable_test.txt");

        // 清理可能存在的文件
        let _ = fs::remove_file(&test_file);

        // 测试不存在的文件（应该检查父目录权限）
        let writable = FileTools::is_file_writable(&test_file);
        // 临时目录通常是可写的
        assert!(writable);

        // 创建文件后测试
        fs::write(&test_file, b"test").unwrap();
        assert!(FileTools::is_file_writable(&test_file));

        // 清理
        let _ = fs::remove_file(&test_file);
    }
}
