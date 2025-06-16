//! 定义 QuantumLog (灵迹) 日志框架的所有配置结构体。

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

// --- 辅助函数，用于提供配置项的默认值 ---
fn default_global_level() -> String {
    "INFO".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_true_option() -> Option<bool> {
    Some(true)
}
fn default_timestamp_format() -> String {
    "%Y-%m-%d %H:%M:%S%.3f".to_string()
}
fn default_log_template() -> String {
    "{timestamp} [{level}] {target} - {message}".to_string()
}
fn default_json_fields_key() -> String {
    "fields".to_string()
}
fn default_file_sink_filename_base() -> String {
    "quantum".to_string()
}
fn default_file_buffer_size() -> usize {
    8192
}
fn default_writer_cache_ttl_seconds() -> u64 {
    300
} // 5 minutes
fn default_writer_cache_capacity() -> u64 {
    1024
}
fn default_db_table_name() -> String {
    "quantum_logs".to_string()
}
fn default_db_batch_size() -> usize {
    100
}
fn default_db_pool_size() -> u32 {
    5
}
fn default_db_connection_timeout_ms() -> u64 {
    5000
}
fn default_output_format() -> OutputFormat {
    OutputFormat::Text
}

/// 定义当日志通道已满时的背压处理策略。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub enum BackpressureStrategy {
    /// 阻塞直到通道有空间。
    Block,
    /// 如果通道已满，立即丢弃最新的日志。
    #[default]
    Drop,
}

/// QuantumLog (灵迹) 日志框架的顶层配置结构体。
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct QuantumLoggerConfig {
    #[serde(default = "default_global_level")]
    pub global_level: String,
    pub pre_init_buffer_size: Option<usize>,
    #[serde(default = "default_false")]
    pub pre_init_stdout_enabled: bool,
    #[serde(default)]
    pub backpressure_strategy: BackpressureStrategy,
    pub stdout: Option<StdoutConfig>,
    pub file: Option<FileSinkConfig>,
    pub database: Option<DatabaseSinkConfig>,
    #[serde(default)]
    pub context_fields: ContextFieldsConfig,
    #[serde(default)]
    pub format: LogFormatConfig,
}

impl Default for QuantumLoggerConfig {
    fn default() -> Self {
        Self {
            global_level: default_global_level(),
            pre_init_buffer_size: None,
            pre_init_stdout_enabled: default_false(),
            backpressure_strategy: BackpressureStrategy::default(),
            stdout: None,
            file: None,
            database: None,
            context_fields: ContextFieldsConfig::default(),
            format: LogFormatConfig::default(),
        }
    }
}

/// 控制哪些背景信息字段需要被采集和包含在日志输出中。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextFieldsConfig {
    #[serde(default = "default_true")]
    pub timestamp: bool,
    #[serde(default = "default_true")]
    pub level: bool,
    #[serde(default = "default_true")]
    pub target: bool,
    #[serde(default = "default_false")]
    pub file_line: bool,
    #[serde(default = "default_false")]
    pub pid: bool,
    #[serde(default = "default_false")]
    pub tid: bool,
    #[serde(default = "default_false")]
    pub mpi_rank: bool,
    #[serde(default = "default_false")]
    pub username: bool,
    #[serde(default = "default_false")]
    pub hostname: bool,
    #[serde(default = "default_false")]
    pub span_info: bool,
}

impl Default for ContextFieldsConfig {
    fn default() -> Self {
        Self {
            timestamp: default_true(),
            level: default_true(),
            target: default_true(),
            file_line: default_false(),
            pid: default_false(),
            tid: default_false(),
            mpi_rank: default_false(),
            username: default_false(),
            hostname: default_false(),
            span_info: default_false(),
        }
    }
}

/// 日志级别枚举
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "TRACE" => Ok(LogLevel::Trace),
            "DEBUG" => Ok(LogLevel::Debug),
            "INFO" => Ok(LogLevel::Info),
            "WARN" => Ok(LogLevel::Warn),
            "ERROR" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

/// 日志格式类型
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LogFormatType {
    Text,
    Json,
    Csv,
}

/// 输出格式
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    Csv,
}

/// 网络协议
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum NetworkProtocol {
    Tcp,
    Udp,
    Http,
}

/// 轮转策略（别名）
pub type RotationPolicy = RotationStrategy;

/// 日志格式化相关的配置。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LogFormatConfig {
    #[serde(default = "default_timestamp_format")]
    pub timestamp_format: String,
    #[serde(default = "default_log_template")]
    pub template: String,
    pub csv_columns: Option<Vec<String>>,
    #[serde(default = "default_false")]
    pub json_flatten_fields: bool,
    #[serde(default = "default_json_fields_key")]
    pub json_fields_key: String,
    #[serde(default = "default_format_type")]
    pub format_type: LogFormatType,
}

fn default_format_type() -> LogFormatType {
    LogFormatType::Text
}

impl Default for LogFormatConfig {
    fn default() -> Self {
        Self {
            timestamp_format: default_timestamp_format(),
            template: default_log_template(),
            csv_columns: None,
            json_flatten_fields: default_false(),
            json_fields_key: default_json_fields_key(),
            format_type: default_format_type(),
        }
    }
}

/// 标准输出 (stdout) 的配置。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StdoutConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub level: Option<String>,
    #[serde(default = "default_true_option")]
    pub color_enabled: Option<bool>,
    pub level_colors: Option<HashMap<String, String>>,
    #[serde(default = "default_output_format")]
    pub format: OutputFormat,
    #[serde(default = "default_true")]
    pub colored: bool,
}

impl Default for StdoutConfig {
    fn default() -> Self {
        Self {
            enabled: default_false(),
            level: None,
            color_enabled: default_true_option(),
            level_colors: None,
            format: default_output_format(),
            colored: default_true(),
        }
    }
}

/// 文件输出类型。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum FileOutputType {
    Text,
    Csv,
    Json,
}

/// 文件分离策略。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub enum FileSeparationStrategy {
    #[default]
    None,
    ByPid,
    ByTid,
    ByMpiRank,
    Level,
    Module,
    Time,
}

/// 日志文件轮转策略。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum RotationStrategy {
    None,
    Hourly,
    Daily,
    Size,
}

/// 日志文件轮转配置。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RotationConfig {
    pub strategy: RotationStrategy,
    pub max_size_mb: Option<u64>,
    pub max_files: Option<usize>,
    #[serde(default = "default_false")]
    pub compress_rotated_files: bool,
}

/// 文件输出管道的配置。
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct FileSinkConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub level: Option<String>,
    pub output_type: FileOutputType,
    pub directory: PathBuf,
    #[serde(default = "default_file_sink_filename_base")]
    pub filename_base: String,
    pub extension: Option<String>,
    #[serde(default)]
    pub separation_strategy: FileSeparationStrategy,
    #[serde(default = "default_file_buffer_size")]
    pub write_buffer_size: usize,
    pub rotation: Option<RotationConfig>,
    #[serde(default = "default_writer_cache_ttl_seconds")]
    pub writer_cache_ttl_seconds: u64,
    #[serde(default = "default_writer_cache_capacity")]
    pub writer_cache_capacity: u64,
}

/// 支持的数据库类型。
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum DatabaseType {
    Sqlite,
    Mysql,
    Postgresql,
}

/// 文件配置（别名）
pub type FileConfig = FileSinkConfig;

/// 滚动文件配置（别名）
pub type RollingFileConfig = FileSinkConfig;

/// 网络配置
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub level: Option<String>,
    pub protocol: NetworkProtocol,
    pub host: String,
    pub port: u16,
    pub format: OutputFormat,
    #[serde(default = "default_file_buffer_size")]
    pub buffer_size: usize,
    pub timeout_ms: Option<u64>,
}

/// 级别文件配置
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct LevelFileConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub directory: PathBuf,
    #[serde(default = "default_file_sink_filename_base")]
    pub filename_base: String,
    pub extension: Option<String>,
    pub levels: Option<Vec<LogLevel>>,
    pub format: OutputFormat,
    #[serde(default = "default_file_buffer_size")]
    pub buffer_size: usize,
    pub rotation: Option<RotationConfig>,
}

/// QuantumLog配置（别名）
pub type QuantumLogConfig = QuantumLoggerConfig;

/// 数据库输出管道的配置。
#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSinkConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub level: Option<String>,
    pub db_type: DatabaseType,
    pub connection_string: String,
    pub schema_name: Option<String>,
    #[serde(default = "default_db_table_name")]
    pub table_name: String,
    #[serde(default = "default_db_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_db_pool_size")]
    pub connection_pool_size: u32,
    #[serde(default = "default_db_connection_timeout_ms")]
    pub connection_timeout_ms: u64,
    #[serde(default = "default_true")]
    pub auto_create_table: bool,
}

/// 用于从 TOML 文件加载 `QuantumLoggerConfig` 的辅助函数。
pub fn load_config_from_file(path: &std::path::Path) -> crate::error::Result<QuantumLoggerConfig> {
    use crate::error::QuantumLogError;
    use std::fs;

    if !path.exists() {
        return Err(QuantumLogError::ConfigFileMissing(
            path.to_string_lossy().into_owned(),
        ));
    }

    let config_str = fs::read_to_string(path)?;
    let config: QuantumLoggerConfig = toml::from_str(&config_str)
        .map_err(|e| QuantumLogError::ConfigError(format!("TOML解析失败: {}", e)))?;

    Ok(config)
}

/// 用于从 TOML 字符串加载 `QuantumLoggerConfig` 的辅助函数。
pub fn load_config_from_str(config_str: &str) -> crate::error::Result<QuantumLoggerConfig> {
    use crate::error::QuantumLogError;

    let config: QuantumLoggerConfig = toml::from_str(config_str)
        .map_err(|e| QuantumLogError::ConfigError(format!("TOML解析失败: {}", e)))?;

    Ok(config)
}

/// 验证配置的有效性。
pub fn validate_config(config: &QuantumLoggerConfig) -> crate::error::Result<()> {
    use crate::error::QuantumLogError;

    // 验证全局日志级别
    match config.global_level.to_uppercase().as_str() {
        "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR" => {}
        _ => {
            return Err(QuantumLogError::InvalidLogLevel(
                config.global_level.clone(),
            ))
        }
    }

    // 验证各个sink的日志级别
    if let Some(ref stdout_config) = config.stdout {
        if let Some(ref level) = stdout_config.level {
            match level.to_uppercase().as_str() {
                "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR" => {}
                _ => return Err(QuantumLogError::InvalidLogLevel(level.clone())),
            }
        }
    }

    if let Some(ref file_config) = config.file {
        if let Some(ref level) = file_config.level {
            match level.to_uppercase().as_str() {
                "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR" => {}
                _ => return Err(QuantumLogError::InvalidLogLevel(level.clone())),
            }
        }

        // 验证文件路径
        if !file_config.directory.is_absolute() {
            return Err(QuantumLogError::InvalidPath(format!(
                "文件输出目录必须是绝对路径: {:?}",
                file_config.directory
            )));
        }
    }

    if let Some(ref db_config) = config.database {
        if let Some(ref level) = db_config.level {
            match level.to_uppercase().as_str() {
                "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR" => {}
                _ => return Err(QuantumLogError::InvalidLogLevel(level.clone())),
            }
        }

        // 验证数据库连接字符串不为空
        if db_config.connection_string.trim().is_empty() {
            return Err(QuantumLogError::ConfigError(
                "数据库连接字符串不能为空".to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = QuantumLoggerConfig::default();
        assert_eq!(config.global_level, "INFO");
        assert_eq!(config.pre_init_stdout_enabled, false);
        assert_eq!(config.backpressure_strategy, BackpressureStrategy::Drop);
        assert!(config.stdout.is_none());
        assert!(config.file.is_none());
        assert!(config.database.is_none());
    }

    #[test]
    fn test_context_fields_config_defaults() {
        let config = ContextFieldsConfig::default();
        assert_eq!(config.timestamp, true);
        assert_eq!(config.level, true);
        assert_eq!(config.target, true);
        assert_eq!(config.file_line, false);
        assert_eq!(config.pid, false);
        assert_eq!(config.tid, false);
        assert_eq!(config.mpi_rank, false);
        assert_eq!(config.username, false);
        assert_eq!(config.hostname, false);
        assert_eq!(config.span_info, false);
    }

    #[test]
    fn test_log_format_config_defaults() {
        let config = LogFormatConfig::default();
        assert_eq!(config.timestamp_format, "%Y-%m-%d %H:%M:%S%.3f");
        assert_eq!(
            config.template,
            "{timestamp} [{level}] {target} - {message}"
        );
        assert!(config.csv_columns.is_none());
        assert_eq!(config.json_flatten_fields, false);
        assert_eq!(config.json_fields_key, "fields");
    }

    #[test]
    fn test_stdout_config_defaults() {
        let config = StdoutConfig::default();
        assert_eq!(config.enabled, false);
        assert!(config.level.is_none());
        assert_eq!(config.color_enabled, Some(true));
        assert!(config.level_colors.is_none());
    }

    #[test]
    fn test_backpressure_strategy_default() {
        let strategy = BackpressureStrategy::default();
        assert_eq!(strategy, BackpressureStrategy::Drop);
    }

    #[test]
    fn test_file_separation_strategy_default() {
        let strategy = FileSeparationStrategy::default();
        assert_eq!(strategy, FileSeparationStrategy::None);
    }

    #[test]
    fn test_load_config_from_str_basic() {
        let toml_str = r#"
            global_level = "DEBUG"
            pre_init_stdout_enabled = true
            
            [stdout]
            enabled = true
            level = "INFO"
        "#;

        let config = load_config_from_str(toml_str).unwrap();
        assert_eq!(config.global_level, "DEBUG");
        assert_eq!(config.pre_init_stdout_enabled, true);
        assert!(config.stdout.is_some());

        let stdout_config = config.stdout.unwrap();
        assert_eq!(stdout_config.enabled, true);
        assert_eq!(stdout_config.level, Some("INFO".to_string()));
    }

    #[test]
    fn test_load_config_from_str_with_context_fields() {
        let toml_str = r#"
            global_level = "TRACE"
            
            [context_fields]
            timestamp = true
            level = true
            target = false
            pid = true
            tid = true
            mpi_rank = true
        "#;

        let config = load_config_from_str(toml_str).unwrap();
        assert_eq!(config.global_level, "TRACE");

        let context = &config.context_fields;
        assert_eq!(context.timestamp, true);
        assert_eq!(context.level, true);
        assert_eq!(context.target, false);
        assert_eq!(context.pid, true);
        assert_eq!(context.tid, true);
        assert_eq!(context.mpi_rank, true);
    }

    #[test]
    fn test_load_config_from_str_with_file_config() {
        let toml_str = r#"
            global_level = "INFO"
            
            [file]
            enabled = true
            output_type = "Json"
            directory = "/tmp/logs"
            filename_base = "test"
            extension = "log"
            separation_strategy = "ByPid"
            write_buffer_size = 4096
            
            [file.rotation]
            strategy = "Daily"
            max_files = 7
            compress_rotated_files = true
        "#;

        let config = load_config_from_str(toml_str).unwrap();
        assert!(config.file.is_some());

        let file_config = config.file.unwrap();
        assert_eq!(file_config.enabled, true);
        assert_eq!(file_config.output_type, FileOutputType::Json);
        assert_eq!(file_config.directory, PathBuf::from("/tmp/logs"));
        assert_eq!(file_config.filename_base, "test");
        assert_eq!(file_config.extension, Some("log".to_string()));
        assert_eq!(
            file_config.separation_strategy,
            FileSeparationStrategy::ByPid
        );
        assert_eq!(file_config.write_buffer_size, 4096);

        assert!(file_config.rotation.is_some());
        let rotation = file_config.rotation.unwrap();
        assert_eq!(rotation.strategy, RotationStrategy::Daily);
        assert_eq!(rotation.max_files, Some(7));
        assert_eq!(rotation.compress_rotated_files, true);
    }

    #[test]
    fn test_load_config_from_str_with_database_config() {
        let toml_str = r#"
            global_level = "WARN"
            
            [database]
            enabled = true
            level = "ERROR"
            db_type = "Sqlite"
            connection_string = "sqlite:///tmp/quantum.db"
            table_name = "logs"
            batch_size = 50
            connection_pool_size = 3
            auto_create_table = false
        "#;

        let config = load_config_from_str(toml_str).unwrap();
        assert!(config.database.is_some());

        let db_config = config.database.unwrap();
        assert_eq!(db_config.enabled, true);
        assert_eq!(db_config.level, Some("ERROR".to_string()));
        assert_eq!(db_config.db_type, DatabaseType::Sqlite);
        assert_eq!(db_config.connection_string, "sqlite:///tmp/quantum.db");
        assert_eq!(db_config.table_name, "logs");
        assert_eq!(db_config.batch_size, 50);
        assert_eq!(db_config.connection_pool_size, 3);
        assert_eq!(db_config.auto_create_table, false);
    }

    #[test]
    fn test_validate_config_valid() {
        let mut config = QuantumLoggerConfig::default();
        config.global_level = "INFO".to_string();

        let result = validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_invalid_global_level() {
        let mut config = QuantumLoggerConfig::default();
        config.global_level = "INVALID".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());

        if let Err(crate::error::QuantumLogError::InvalidLogLevel(level)) = result {
            assert_eq!(level, "INVALID");
        } else {
            panic!("Expected InvalidLogLevel error");
        }
    }

    #[test]
    fn test_validate_config_invalid_stdout_level() {
        let mut config = QuantumLoggerConfig::default();
        config.stdout = Some(StdoutConfig {
            enabled: true,
            level: Some("INVALID".to_string()),
            color_enabled: Some(true),
            level_colors: None,
            format: crate::config::OutputFormat::Text,
            colored: true,
        });

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_invalid_file_path() {
        let mut config = QuantumLoggerConfig::default();
        config.file = Some(FileSinkConfig {
            enabled: true,
            level: None,
            output_type: FileOutputType::Text,
            directory: PathBuf::from("relative/path"), // 相对路径，应该失败
            filename_base: "test".to_string(),
            extension: None,
            separation_strategy: FileSeparationStrategy::None,
            write_buffer_size: 8192,
            rotation: None,
            writer_cache_ttl_seconds: 300,
            writer_cache_capacity: 1024,
        });

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_empty_database_connection() {
        let mut config = QuantumLoggerConfig::default();
        config.database = Some(DatabaseSinkConfig {
            enabled: true,
            level: None,
            db_type: DatabaseType::Sqlite,
            connection_string: "".to_string(), // 空连接字符串，应该失败
            schema_name: None,
            table_name: "logs".to_string(),
            batch_size: 100,
            connection_pool_size: 5,
            connection_timeout_ms: 5000,
            auto_create_table: true,
        });

        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_from_str_invalid_toml() {
        let invalid_toml = r#"
            global_level = "INFO
            # 缺少引号结束
        "#;

        let result = load_config_from_str(invalid_toml);
        assert!(result.is_err());

        if let Err(crate::error::QuantumLogError::ConfigError(msg)) = result {
            assert!(msg.contains("TOML解析失败"));
        } else {
            panic!("Expected ConfigError");
        }
    }
}
