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

// 网络相关默认值
fn default_network_buffer_size() -> usize {
    1000
}

fn default_network_timeout_ms() -> u64 {
    30000
}

fn default_max_reconnect_attempts() -> usize {
    5
}

fn default_reconnect_delay_ms() -> u64 {
    1000
}

#[cfg(feature = "tls")]
fn default_tls_verify_certificates() -> bool {
    true
}

#[cfg(feature = "tls")]
fn default_tls_verify_hostname() -> bool {
    true
}

#[cfg(feature = "tls")]
fn default_tls_min_version() -> TlsVersion {
    TlsVersion::Tls13
}

#[cfg(feature = "tls")]
fn default_tls_cipher_suite() -> TlsCipherSuite {
    TlsCipherSuite::High
}

#[cfg(feature = "tls")]
fn default_tls_require_sni() -> bool {
    true
}

fn default_security_policy() -> SecurityPolicy {
    SecurityPolicy::Strict
}

fn default_connection_rate_limit() -> u32 {
    100 // 每秒最多100个连接
}

fn default_enable_security_audit() -> bool {
    true
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
    pub network: Option<NetworkConfig>,
    pub level_file: Option<LevelFileConfig>,
    /// InfluxDB 配置
    pub influxdb: Option<InfluxDBConfig>,
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
            network: None,
            level_file: None,
            influxdb: None,
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

/// 安全策略级别
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SecurityPolicy {
    /// 严格安全策略 - 生产环境推荐
    Strict,
    /// 平衡安全策略 - 开发环境推荐
    Balanced,
    /// 宽松安全策略 - 仅用于测试环境
    Permissive,
}

/// TLS 最低版本要求
#[cfg(feature = "tls")]
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TlsVersion {
    /// TLS 1.2
    Tls12,
    /// TLS 1.3 (推荐)
    Tls13,
}

/// TLS 密码套件安全级别
#[cfg(feature = "tls")]
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TlsCipherSuite {
    /// 高安全级别密码套件
    High,
    /// 中等安全级别密码套件
    Medium,
    /// 兼容性密码套件（不推荐生产环境）
    Compatible,
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

/// InfluxDB 配置
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct InfluxDBConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub level: Option<String>,
    /// InfluxDB 服务器 URL
    pub url: String,
    /// 数据库名称
    pub database: String,
    /// 认证 Token (可选)
    pub token: Option<String>,
    /// 用户名 (可选，用于 InfluxDB 1.x)
    pub username: Option<String>,
    /// 密码 (可选，用于 InfluxDB 1.x)
    pub password: Option<String>,
    /// 批处理大小
    #[serde(default = "default_influxdb_batch_size")]
    pub batch_size: usize,
    /// 批处理刷新间隔（秒）
    #[serde(default = "default_influxdb_flush_interval")]
    pub flush_interval_seconds: u64,
    /// 是否使用 HTTPS
    #[serde(default = "default_true")]
    pub use_https: bool,
    /// 是否验证 SSL 证书
    #[serde(default = "default_true")]
    pub verify_ssl: bool,
}

fn default_influxdb_batch_size() -> usize {
    100
}

fn default_influxdb_flush_interval() -> u64 {
    5
}

// 安全的Debug实现，避免泄露敏感的认证信息
impl std::fmt::Debug for InfluxDBConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InfluxDBConfig")
            .field("enabled", &self.enabled)
            .field("level", &self.level)
            .field("url", &self.url)
            .field("database", &self.database)
            .field("token", &self.token.as_ref().map(|_| "[REDACTED]"))
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("batch_size", &self.batch_size)
            .field("flush_interval_seconds", &self.flush_interval_seconds)
            .field("use_https", &self.use_https)
            .field("verify_ssl", &self.verify_ssl)
            .finish()
    }
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
    #[serde(default = "default_network_buffer_size")]
    pub buffer_size: usize,
    #[serde(default = "default_network_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: usize,
    #[serde(default = "default_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,
    
    // 安全策略配置
    #[serde(default = "default_security_policy")]
    pub security_policy: SecurityPolicy,
    #[serde(default = "default_connection_rate_limit")]
    pub connection_rate_limit: u32,
    #[serde(default = "default_enable_security_audit")]
    pub enable_security_audit: bool,
    
    // TLS 配置
    #[cfg(feature = "tls")]
    pub use_tls: Option<bool>,
    #[cfg(feature = "tls")]
    #[serde(default = "default_tls_verify_certificates")]
    pub tls_verify_certificates: bool,
    #[cfg(feature = "tls")]
    #[serde(default = "default_tls_verify_hostname")]
    pub tls_verify_hostname: bool,
    #[cfg(feature = "tls")]
    #[serde(default = "default_tls_min_version")]
    pub tls_min_version: TlsVersion,
    #[cfg(feature = "tls")]
    #[serde(default = "default_tls_cipher_suite")]
    pub tls_cipher_suite: TlsCipherSuite,
    #[cfg(feature = "tls")]
    #[serde(default = "default_tls_require_sni")]
    pub tls_require_sni: bool,
    #[cfg(feature = "tls")]
    pub tls_ca_file: Option<String>,
    #[cfg(feature = "tls")]
    pub tls_cert_file: Option<String>,
    #[cfg(feature = "tls")]
    pub tls_key_file: Option<String>,
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
#[derive(Deserialize, Clone)]
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

// 安全的Debug实现，避免泄露敏感的连接字符串
impl std::fmt::Debug for DatabaseSinkConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseSinkConfig")
            .field("enabled", &self.enabled)
            .field("level", &self.level)
            .field("db_type", &self.db_type)
            .field("connection_string", &"[REDACTED]")
            .field("schema_name", &self.schema_name)
            .field("table_name", &self.table_name)
            .field("batch_size", &self.batch_size)
            .field("connection_pool_size", &self.connection_pool_size)
            .field("connection_timeout_ms", &self.connection_timeout_ms)
            .field("auto_create_table", &self.auto_create_table)
            .finish()
    }
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

    // 验证预初始化缓冲区大小
    if let Some(buffer_size) = config.pre_init_buffer_size {
        if buffer_size == 0 {
            return Err(QuantumLogError::ConfigError(
                "预初始化缓冲区大小必须大于0".to_string(),
            ));
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

        // 验证缓冲区大小
        if file_config.write_buffer_size == 0 {
            return Err(QuantumLogError::ConfigError(
                "文件写入缓冲区大小必须大于0".to_string(),
            ));
        }

        // 验证文件名基础部分不为空
        if file_config.filename_base.trim().is_empty() {
            return Err(QuantumLogError::ConfigError(
                "文件名基础部分不能为空".to_string(),
            ));
        }

        // 验证写入器缓存配置
        if file_config.writer_cache_capacity == 0 {
            return Err(QuantumLogError::ConfigError(
                "写入器缓存容量必须大于0".to_string(),
            ));
        }

        // 验证轮转配置（如果提供）
        if let Some(ref rotation) = file_config.rotation {
            if let RotationStrategy::Size = rotation.strategy {
                if rotation.max_size_mb.is_none() {
                    return Err(QuantumLogError::ConfigError(
                        "使用大小轮转策略时必须指定max_size_mb".to_string(),
                    ));
                }
                if let Some(max_size) = rotation.max_size_mb {
                    if max_size == 0 {
                        return Err(QuantumLogError::ConfigError(
                            "轮转文件最大大小必须大于0MB".to_string(),
                        ));
                    }
                }
            }
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

        // 验证数据库连接池大小
        if db_config.connection_pool_size == 0 {
            return Err(QuantumLogError::ConfigError(
                "数据库连接池大小必须大于0".to_string(),
            ));
        }

        // 验证批量大小
        if db_config.batch_size == 0 {
            return Err(QuantumLogError::ConfigError(
                "数据库批量大小必须大于0".to_string(),
            ));
        }
    }

    // 验证网络配置
    if let Some(ref network_config) = config.network {
        if let Some(ref level) = network_config.level {
            match level.to_uppercase().as_str() {
                "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR" => {}
                _ => return Err(QuantumLogError::InvalidLogLevel(level.clone())),
            }
        }

        // 验证主机地址不为空
        if network_config.host.trim().is_empty() {
            return Err(QuantumLogError::ConfigError(
                "网络主机地址不能为空".to_string(),
            ));
        }

        // 验证端口范围
        if network_config.port == 0 {
            return Err(QuantumLogError::ConfigError(
                "网络端口必须大于0".to_string(),
            ));
        }

        // 验证缓冲区大小
        if network_config.buffer_size == 0 {
            return Err(QuantumLogError::ConfigError(
                "网络缓冲区大小必须大于0".to_string(),
            ));
        }
    }

    // 验证级别文件配置
    if let Some(ref level_file_config) = config.level_file {
        // 验证文件路径
        if !level_file_config.directory.is_absolute() {
            return Err(QuantumLogError::InvalidPath(format!(
                "级别文件输出目录必须是绝对路径: {:?}",
                level_file_config.directory
            )));
        }

        // 验证缓冲区大小
        if level_file_config.buffer_size == 0 {
            return Err(QuantumLogError::ConfigError(
                "级别文件缓冲区大小必须大于0".to_string(),
            ));
        }

        // 验证文件名基础部分不为空
        if level_file_config.filename_base.trim().is_empty() {
            return Err(QuantumLogError::ConfigError(
                "级别文件名基础部分不能为空".to_string(),
            ));
        }

        // 验证级别列表（如果提供）
        if let Some(ref levels) = level_file_config.levels {
            if levels.is_empty() {
                return Err(QuantumLogError::ConfigError(
                    "级别文件的级别列表不能为空".to_string(),
                ));
            }
        }

        // 验证轮转配置（如果提供）
        if let Some(ref rotation) = level_file_config.rotation {
            if let RotationStrategy::Size = rotation.strategy {
                if rotation.max_size_mb.is_none() {
                    return Err(QuantumLogError::ConfigError(
                        "使用大小轮转策略时必须指定max_size_mb".to_string(),
                    ));
                }
                if let Some(max_size) = rotation.max_size_mb {
                    if max_size == 0 {
                        return Err(QuantumLogError::ConfigError(
                            "轮转文件最大大小必须大于0MB".to_string(),
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

/// 验证网络安全配置
pub fn validate_network_security(config: &NetworkConfig) -> crate::error::Result<()> {
    // 严格安全策略下的验证
    if config.security_policy == SecurityPolicy::Strict {
        #[cfg(feature = "tls")]
        {
            if config.use_tls != Some(true) {
                return Err(crate::error::QuantumLogError::ConfigError(
                    "Strict security policy requires TLS to be enabled".to_string(),
                ));
            }
            
            if !config.tls_verify_certificates {
                return Err(crate::error::QuantumLogError::ConfigError(
                    "Strict security policy requires certificate verification".to_string(),
                ));
            }
            
            if !config.tls_verify_hostname {
                return Err(crate::error::QuantumLogError::ConfigError(
                    "Strict security policy requires hostname verification".to_string(),
                ));
            }
            
            if config.tls_min_version != TlsVersion::Tls13 {
                return Err(crate::error::QuantumLogError::ConfigError(
                    "Strict security policy requires TLS 1.3 minimum".to_string(),
                ));
            }
            
            if config.tls_cipher_suite != TlsCipherSuite::High {
                return Err(crate::error::QuantumLogError::ConfigError(
                    "Strict security policy requires high-security cipher suites".to_string(),
                ));
            }
        }
        
        #[cfg(not(feature = "tls"))]
        {
            return Err(crate::error::QuantumLogError::ConfigError(
                "Strict security policy requires TLS feature to be enabled".to_string(),
            ));
        }
    }
    
    // 验证连接速率限制
    if config.connection_rate_limit == 0 {
        return Err(crate::error::QuantumLogError::ConfigError(
            "Connection rate limit must be greater than 0".to_string(),
        ));
    }
    
    if config.connection_rate_limit > 10000 {
        return Err(crate::error::QuantumLogError::ConfigError(
            "Connection rate limit too high (max: 10000)".to_string(),
        ));
    }
    
    Ok(())
}

/// 获取安全的默认网络配置
pub fn get_secure_network_config() -> NetworkConfig {
    NetworkConfig {
        enabled: false,
        level: None,
        protocol: NetworkProtocol::Tcp,
        host: "localhost".to_string(),
        port: 8443,
        format: OutputFormat::Json,
        buffer_size: default_network_buffer_size(),
        timeout_ms: default_network_timeout_ms(),
        max_reconnect_attempts: default_max_reconnect_attempts(),
        reconnect_delay_ms: default_reconnect_delay_ms(),
        security_policy: SecurityPolicy::Strict,
        connection_rate_limit: default_connection_rate_limit(),
        enable_security_audit: true,
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: true,
        #[cfg(feature = "tls")]
        tls_verify_hostname: true,
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls13,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::High,
        #[cfg(feature = "tls")]
        tls_require_sni: true,
        #[cfg(feature = "tls")]
        tls_ca_file: None,
        #[cfg(feature = "tls")]
        tls_cert_file: None,
        #[cfg(feature = "tls")]
        tls_key_file: None,
    }
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
