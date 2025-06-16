//! Error types for QuantumLog
//!
//! This module defines all error types used throughout the QuantumLog framework.
//! It provides a unified error handling system with proper error chaining and
//! detailed error messages for debugging.

use thiserror::Error;

/// Main error type for QuantumLog operations
#[derive(Error, Debug)]
pub enum QuantumLogError {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Initialization errors
    #[error("Initialization error: {0}")]
    InitializationError(String),

    /// 停机相关错误
    #[error("Shutdown in progress")]
    ShutdownInProgress,

    #[error("Already shutdown")]
    AlreadyShutdown,

    #[error("Shutdown failed: {0}")]
    ShutdownFailed(String),

    #[error("Shutdown timeout")]
    ShutdownTimeout,

    #[error("Shutdown channel closed")]
    ShutdownChannelClosed,

    /// Configuration file not found
    #[error("Configuration file not found: {0}")]
    ConfigFileMissing(String),

    /// Invalid log level
    #[error("Invalid log level: {0}")]
    InvalidLogLevel(String),

    /// Invalid file path
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// I/O errors (file operations, network, etc.)
    #[error("I/O error: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    /// Serialization/deserialization errors
    #[error("Serialization error: {source}")]
    SerializationError {
        #[from]
        source: serde_json::Error,
    },

    /// TOML parsing errors
    #[error("TOML parsing error: {source}")]
    TomlError {
        #[from]
        source: toml::de::Error,
    },

    /// Database connection and operation errors
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// 数据库连接错误（占位符）
    #[error("Database connection error: {message}")]
    DatabaseConnection { message: String },

    /// 数据库连接池错误（占位符）
    #[error("Database pool error: {message}")]
    DatabasePool { message: String },

    /// CSV processing errors
    #[error("CSV error: {source}")]
    CsvError {
        #[from]
        source: csv::Error,
    },

    /// MPI FFI errors
    #[error("MPI error: {0}")]
    MpiError(String),

    /// Dynamic library loading errors
    #[cfg(feature = "dynamic_mpi")]
    #[error("Library loading error: {source}")]
    LibLoadingError {
        #[from]
        source: libloading::Error,
    },

    /// Tracing subscriber errors
    #[error("Tracing error: {0}")]
    TracingError(String),

    /// Shutdown-related errors
    #[error("Shutdown error: {0}")]
    ShutdownError(String),

    /// Channel communication errors
    #[error("Channel error: {0}")]
    ChannelError(String),

    /// Validation errors
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// File rotation errors
    #[error("File rotation error: {0}")]
    RotationError(String),

    /// Background task errors
    #[error("Background task error: {0}")]
    BackgroundTaskError(String),

    /// Network-related errors
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Sink-related errors
    #[error("Sink error: {0}")]
    SinkError(String),

    /// Generic internal errors
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Generic boxed error for compatibility
    #[error("Generic error: {source}")]
    GenericError {
        #[from]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Result type alias for QuantumLog operations
pub type Result<T> = std::result::Result<T, QuantumLogError>;

impl QuantumLogError {
    /// Create a new configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::ConfigError(msg.into())
    }

    /// Create a new database error
    pub fn database<S: Into<String>>(msg: S) -> Self {
        Self::DatabaseError(msg.into())
    }

    /// Create a new MPI error
    pub fn mpi<S: Into<String>>(msg: S) -> Self {
        Self::MpiError(msg.into())
    }

    /// Create a new tracing error
    pub fn tracing<S: Into<String>>(msg: S) -> Self {
        Self::TracingError(msg.into())
    }

    /// Create a new shutdown error
    pub fn shutdown<S: Into<String>>(msg: S) -> Self {
        Self::ShutdownError(msg.into())
    }

    /// Create a new channel error
    pub fn channel<S: Into<String>>(msg: S) -> Self {
        Self::ChannelError(msg.into())
    }

    /// Create a new validation error
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::ValidationError(msg.into())
    }

    /// Create a new network error
    pub fn network<S: Into<String>>(msg: S) -> Self {
        Self::NetworkError(msg.into())
    }

    /// Create a new sink error
    pub fn sink<S: Into<String>>(msg: S) -> Self {
        Self::SinkError(msg.into())
    }

    /// Create a new rotation error
    pub fn rotation<S: Into<String>>(msg: S) -> Self {
        Self::RotationError(msg.into())
    }

    /// Create a new background task error
    pub fn background_task<S: Into<String>>(msg: S) -> Self {
        Self::BackgroundTaskError(msg.into())
    }

    /// Create a new internal error
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::InternalError(msg.into())
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::IoError { .. } => true,
            Self::DatabaseError(_) => true,
            Self::DatabaseConnection { .. } => true,
            Self::DatabasePool { .. } => true,
            Self::ChannelError(_) => true,
            Self::BackgroundTaskError(_) => true,
            Self::InitializationError(_) => false,
            Self::ShutdownInProgress => false,
            Self::AlreadyShutdown => false,
            Self::ShutdownFailed(_) => false,
            Self::ShutdownTimeout => false,
            Self::ShutdownChannelClosed => false,
            _ => false,
        }
    }

    /// Get the error category for logging purposes
    pub fn category(&self) -> &'static str {
        match self {
            Self::ConfigError(_)
            | Self::ConfigFileMissing(_)
            | Self::InvalidLogLevel(_)
            | Self::InvalidPath(_) => "config",
            Self::InitializationError(_) => "initialization",
            Self::ShutdownInProgress
            | Self::AlreadyShutdown
            | Self::ShutdownFailed(_)
            | Self::ShutdownTimeout
            | Self::ShutdownChannelClosed => "shutdown",
            Self::IoError { .. } => "io",
            Self::SerializationError { .. } => "serialization",
            Self::TomlError { .. } => "toml",
            Self::DatabaseError(_)
            | Self::DatabaseConnection { .. }
            | Self::DatabasePool { .. } => "database",
            Self::CsvError { .. } => "csv",
            Self::MpiError(_) => "mpi",
            #[cfg(feature = "dynamic_mpi")]
            Self::LibLoadingError { .. } => "mpi",
            Self::TracingError(_) => "tracing",
            Self::ShutdownError(_) => "shutdown",
            Self::ChannelError(_) => "channel",
            Self::ValidationError(_) => "validation",
            Self::RotationError(_) => "rotation",
            Self::BackgroundTaskError(_) => "background_task",
            Self::NetworkError(_) => "network",
            Self::SinkError(_) => "sink",
            Self::InternalError(_) => "internal",
            Self::GenericError { .. } => "generic",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_error_creation() {
        let config_err = QuantumLogError::config("Invalid configuration");
        assert!(matches!(config_err, QuantumLogError::ConfigError(_)));
        assert_eq!(
            config_err.to_string(),
            "Configuration error: Invalid configuration"
        );

        let db_err = QuantumLogError::database("Connection failed");
        assert!(matches!(db_err, QuantumLogError::DatabaseError(_)));
        assert_eq!(db_err.to_string(), "Database error: Connection failed");
    }

    #[test]
    fn test_error_from_conversions() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let quantum_error: QuantumLogError = io_error.into();
        assert!(matches!(quantum_error, QuantumLogError::IoError { .. }));

        let json_error = serde_json::from_str::<serde_json::Value>("{invalid json").unwrap_err();
        let quantum_error: QuantumLogError = json_error.into();
        assert!(matches!(
            quantum_error,
            QuantumLogError::SerializationError { .. }
        ));
    }

    #[test]
    fn test_error_recoverability() {
        assert!(QuantumLogError::database("temp failure").is_recoverable());
        assert!(QuantumLogError::channel("channel closed").is_recoverable());
        assert!(!QuantumLogError::config("invalid config").is_recoverable());
        assert!(!QuantumLogError::validation("invalid input").is_recoverable());
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(QuantumLogError::config("test").category(), "config");
        assert_eq!(QuantumLogError::database("test").category(), "database");
        assert_eq!(QuantumLogError::mpi("test").category(), "mpi");
        assert_eq!(QuantumLogError::tracing("test").category(), "tracing");
        assert_eq!(QuantumLogError::shutdown("test").category(), "shutdown");
        assert_eq!(QuantumLogError::channel("test").category(), "channel");
        assert_eq!(QuantumLogError::validation("test").category(), "validation");
        assert_eq!(QuantumLogError::rotation("test").category(), "rotation");
        assert_eq!(
            QuantumLogError::background_task("test").category(),
            "background_task"
        );
        assert_eq!(QuantumLogError::internal("test").category(), "internal");
    }

    #[test]
    fn test_empty_string_error() {
        let err = QuantumLogError::config("");
        assert_eq!(err.to_string(), "Configuration error: ");
    }

    #[test]
    fn test_very_long_error_message() {
        let long_msg = "a".repeat(10000);
        let err = QuantumLogError::config(&long_msg);
        assert!(err.to_string().contains(&long_msg));
    }

    #[test]
    fn test_unicode_error_message() {
        let unicode_msg = "配置错误: 无效的参数 🚫";
        let err = QuantumLogError::config(unicode_msg);
        assert!(err.to_string().contains(unicode_msg));
    }

    #[test]
    fn test_error_chain() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let quantum_error: QuantumLogError = io_error.into();

        // Test that the source error is preserved
        let error_string = quantum_error.to_string();
        assert!(error_string.contains("Access denied"));
    }

    #[test]
    fn test_error_debug_format() {
        let err = QuantumLogError::config("test error");
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("ConfigError"));
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_result_type_alias() {
        fn test_function() -> Result<i32> {
            Ok(42)
        }

        fn test_error_function() -> Result<i32> {
            Err(QuantumLogError::config("test"))
        }

        assert_eq!(test_function().unwrap(), 42);
        assert!(test_error_function().is_err());
    }

    #[test]
    fn test_error_boundary_values() {
        // Test with null character
        let err = QuantumLogError::config("test\0error");
        assert!(err.to_string().contains("test\0error"));

        // Test with newlines
        let err = QuantumLogError::config("line1\nline2\r\nline3");
        assert!(err.to_string().contains("line1\nline2\r\nline3"));

        // Test with tabs
        let err = QuantumLogError::config("tab\there");
        assert!(err.to_string().contains("tab\there"));
    }

    #[test]
    fn test_all_error_variants_display() {
        let errors = vec![
            QuantumLogError::config("config"),
            QuantumLogError::database("database"),
            QuantumLogError::mpi("mpi"),
            QuantumLogError::tracing("tracing"),
            QuantumLogError::shutdown("shutdown"),
            QuantumLogError::channel("channel"),
            QuantumLogError::validation("validation"),
            QuantumLogError::rotation("rotation"),
            QuantumLogError::background_task("background_task"),
            QuantumLogError::internal("internal"),
        ];

        for error in errors {
            let display_str = error.to_string();
            assert!(!display_str.is_empty());
            assert!(display_str.len() > 5); // Should have meaningful content
        }
    }
}
