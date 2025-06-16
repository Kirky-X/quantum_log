//! QuantumLog - 高性能异步日志库
//!
//! QuantumLog 是一个专为高性能计算环境设计的异步日志库，
//! 支持多种输出格式和目标，包括文件、数据库和标准输出。
//!
//! # 快速开始
//!
//! ```rust
//! use quantum_log::{init, shutdown};
//! use tracing::{info, warn, error};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 初始化 QuantumLog
//!     init().await?;
//!     
//!     // 使用标准的 tracing 宏
//!     info!("Application started");
//!     warn!("This is a warning");
//!     error!("This is an error");
//!     
//!     // 优雅关闭
//!     shutdown().await?;
//!     Ok(())
//! }
//! ```
//!
//! # 自定义配置
//!
//! ```rust
//! use quantum_log::{QuantumLogConfig, init_with_config};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = QuantumLogConfig {
//!         global_level: "DEBUG".to_string(),
//!         ..Default::default()
//!     };
//!     
//!     quantum_log::init_with_config(config).await?;
//!     
//!     // 你的应用代码...
//!     
//!     quantum_log::shutdown().await?;
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod core;
pub mod diagnostics;
pub mod error;
pub mod mpi;
pub mod shutdown;
pub mod sinks;
pub mod utils;

// 条件编译数据库功能
#[cfg(feature = "database")]
pub mod database {
    pub use crate::sinks::database::*;
}

// 重新导出主要类型
pub use config::{
    load_config_from_file, QuantumLogConfig, QuantumLoggerConfig,
    BackpressureStrategy, StdoutConfig, OutputFormat
};
pub use diagnostics::{get_diagnostics, DiagnosticsSnapshot};
pub use error::{QuantumLogError, Result};
pub use shutdown::{
    ShutdownCoordinator, ShutdownHandle as QuantumShutdownHandle, ShutdownListener, ShutdownSignal,
    ShutdownState,
};

// 重新导出核心功能
pub use core::event::QuantumLogEvent;
pub use core::subscriber::{BufferStats, QuantumLogSubscriber, QuantumLogSubscriberBuilder};

use once_cell::sync::Lazy;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

/// 库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 全局订阅器实例
static GLOBAL_SUBSCRIBER: Lazy<Arc<Mutex<Option<QuantumLogSubscriber>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// QuantumLog 初始化标记
/// 用于确保日志系统只被初始化一次
pub static IS_QUANTUM_LOG_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 使用默认配置初始化 QuantumLog
///
/// 这是一个便捷函数，使用默认配置初始化 QuantumLog。
/// 默认配置包括：
/// - 启用控制台输出
/// - 日志级别为 INFO
/// - 使用默认格式化器
///
/// # 示例
///
/// ```rust
/// use quantum_log::init;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     init().await?;
///     tracing::info!("Hello, QuantumLog!");
///     Ok(())
/// }
/// ```
pub async fn init() -> Result<()> {
    let config = QuantumLogConfig {
        pre_init_stdout_enabled: true,
        stdout: Some(crate::config::StdoutConfig {
            enabled: true,
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut subscriber = QuantumLogSubscriber::with_config(config)?;

    // 初始化订阅器
    subscriber.initialize().await?;

    // 存储订阅器实例以便后续关闭
    if let Ok(mut global) = GLOBAL_SUBSCRIBER.lock() {
        *global = Some(subscriber.clone());
    }

    // 安装为全局订阅器
    subscriber.install_global()?;

    Ok(())
}

/// 使用指定配置初始化 QuantumLog
///
/// # 参数
///
/// * `config` - QuantumLog 配置
///
/// # 示例
///
/// ```rust
/// use quantum_log::{QuantumLogConfig, init_with_config};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = QuantumLogConfig {
///         global_level: "DEBUG".to_string(),
///         ..Default::default()
///     };
///     
///     quantum_log::init_with_config(config).await?;
///     quantum_log::shutdown().await?;
///     Ok(())
/// }
/// ```
pub async fn init_with_config(config: QuantumLogConfig) -> Result<()> {
    let mut subscriber = QuantumLogSubscriber::with_config(config)?;

    // 初始化订阅器
    subscriber.initialize().await?;

    // 存储订阅器实例以便后续关闭
    if let Ok(mut global) = GLOBAL_SUBSCRIBER.lock() {
        *global = Some(subscriber.clone());
    }

    // 安装为全局订阅器（这会消费 subscriber）
    subscriber.install_global()?;

    Ok(())
}

/// 使用构建器初始化 QuantumLog
///
/// # 参数
///
/// * `builder_fn` - 构建器配置函数
///
/// # 示例
///
/// ```rust
/// use quantum_log::{init_with_builder, shutdown};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
///     init_with_builder(|builder| {
///         builder
///             .max_buffer_size(5000)
///             .custom_field("service", "my-app")
///             .custom_field("version", "1.0.0")
///     }).await?;
///     
///     shutdown().await?;
///     Ok(())
/// }
/// ```
pub async fn init_with_builder<F>(
    builder_fn: F,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(QuantumLogSubscriberBuilder) -> QuantumLogSubscriberBuilder,
{
    let builder = QuantumLogSubscriber::builder();
    let mut subscriber = builder_fn(builder).build()?;

    // 初始化订阅器
    subscriber.initialize().await?;

    // 存储订阅器实例以便后续关闭
    if let Ok(mut global) = GLOBAL_SUBSCRIBER.lock() {
        *global = Some(subscriber.clone());
    }

    // 安装为全局订阅器（这会消费 subscriber）
    subscriber.install_global()?;

    Ok(())
}

/// 优雅关闭 QuantumLog
///
/// 这会关闭所有 sink 并等待它们完成处理所有待处理的事件。
/// 建议在应用程序退出前调用此函数。
///
/// # 示例
///
/// ```rust
/// use quantum_log::{init, shutdown};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     init().await?;
///     
///     // 你的应用代码...
///     
///     shutdown().await?;
///     Ok(())
/// }
/// ```
pub async fn shutdown() -> Result<()> {
    let subscriber = if let Ok(mut global) = GLOBAL_SUBSCRIBER.lock() {
        global.take()
    } else {
        None
    };

    if let Some(subscriber) = subscriber {
        subscriber.shutdown().await?;
    }
    Ok(())
}

/// 获取缓冲区统计信息
///
/// 返回当前预初始化缓冲区的统计信息，包括当前大小、丢弃的事件数量等。
///
/// # 示例
///
/// ```rust
/// use quantum_log::{init, get_buffer_stats, shutdown};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     init().await?;
///     
///     let stats = get_buffer_stats();
///     if let Some(stats) = stats {
///         println!("Buffer size: {}, Dropped: {}", stats.current_size, stats.dropped_count);
///     }
///     
///     shutdown().await?;
///     Ok(())
/// }
/// ```
pub fn get_buffer_stats() -> Option<BufferStats> {
    if let Ok(global) = GLOBAL_SUBSCRIBER.lock() {
        global
            .as_ref()
            .map(|subscriber| subscriber.get_buffer_stats())
    } else {
        None
    }
}

/// 检查 QuantumLog 是否已初始化
///
/// # 示例
///
/// ```rust
/// use quantum_log::{init, is_initialized, shutdown};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     assert!(!is_initialized());
///     
///     init().await?;
///     assert!(is_initialized());
///     
///     shutdown().await?;
///     Ok(())
/// }
/// ```
pub fn is_initialized() -> bool {
    if let Ok(global) = GLOBAL_SUBSCRIBER.lock() {
        global
            .as_ref()
            .is_some_and(|subscriber| subscriber.is_initialized())
    } else {
        false
    }
}

/// 获取当前配置的引用
///
/// 返回当前 QuantumLog 实例使用的配置。如果 QuantumLog 未初始化，返回 None。
pub fn get_config() -> Option<QuantumLogConfig> {
    if let Ok(global) = GLOBAL_SUBSCRIBER.lock() {
        global
            .as_ref()
            .map(|subscriber| subscriber.config().clone())
    } else {
        None
    }
}

// 外部依赖
use tokio::sync::oneshot;

/// 关闭句柄，用于优雅关闭日志系统
#[derive(Debug)]
pub struct ShutdownHandle {
    sender: Option<oneshot::Sender<()>>,
}

impl ShutdownHandle {
    /// 创建新的关闭句柄
    pub fn new(sender: oneshot::Sender<()>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    /// 触发优雅关闭
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(sender) = self.sender.take() {
            sender.send(()).map_err(|_| {
                QuantumLogError::ShutdownError("Failed to send shutdown signal".to_string())
            })?;
        }
        Ok(())
    }
}

/// 初始化 QuantumLog 日志系统
///
/// 这是设计文档中指定的主要初始化函数，它会：
/// 1. 检查是否已经初始化，确保只初始化一次
/// 2. 使用默认配置创建并初始化 QuantumLogSubscriber
/// 3. 返回 ShutdownHandle 用于优雅关闭
///
/// # 返回值
///
/// 返回 `ShutdownHandle`，可用于优雅关闭日志系统
///
/// # 错误
///
/// 如果日志系统已经初始化，将返回错误
///
/// # 示例
///
/// ```rust
/// use quantum_log::init_quantum_logger;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let shutdown_handle = init_quantum_logger().await?;
///     
///     // 你的应用代码...
///     
///     shutdown_handle.shutdown().await?;
///     Ok(())
/// }
/// ```
pub async fn init_quantum_logger() -> Result<ShutdownHandle> {
    // 检查是否已经初始化
    if IS_QUANTUM_LOG_INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(QuantumLogError::InitializationError(
            "QuantumLog has already been initialized".to_string(),
        ));
    }

    // 创建默认配置的订阅器
    let mut subscriber = QuantumLogSubscriber::new().map_err(|e| {
        QuantumLogError::InitializationError(format!("Failed to create subscriber: {}", e))
    })?;

    // 初始化订阅器
    subscriber.initialize().await.map_err(|e| {
        QuantumLogError::InitializationError(format!("Failed to initialize subscriber: {}", e))
    })?;

    // 创建关闭通道
    let (shutdown_sender, shutdown_receiver) = oneshot::channel();

    // 克隆订阅器用于关闭任务
    let subscriber_for_shutdown = subscriber.clone();

    // 保存订阅器到全局状态
    if let Ok(mut global) = GLOBAL_SUBSCRIBER.lock() {
        *global = Some(subscriber.clone());
    } else {
        // 如果获取锁失败，重置初始化标记
        IS_QUANTUM_LOG_INITIALIZED.store(false, Ordering::SeqCst);
        return Err(QuantumLogError::InitializationError(
            "Failed to acquire global subscriber lock".to_string(),
        ));
    }

    // 安装为全局订阅器
    if let Err(e) = subscriber.install_global() {
        // 如果安装失败，清理状态
        IS_QUANTUM_LOG_INITIALIZED.store(false, Ordering::SeqCst);
        if let Ok(mut global) = GLOBAL_SUBSCRIBER.lock() {
            *global = None;
        }
        return Err(QuantumLogError::InitializationError(format!(
            "Failed to install global subscriber: {}",
            e
        )));
    }

    // 启动关闭监听任务
    tokio::spawn(async move {
        if shutdown_receiver.await.is_ok() {
            // 执行优雅关闭
            if let Err(e) = subscriber_for_shutdown.shutdown().await {
                eprintln!("Error during shutdown: {}", e);
            }
            // 重置初始化标记
            IS_QUANTUM_LOG_INITIALIZED.store(false, Ordering::SeqCst);
            // 清理全局订阅器
            if let Ok(mut global) = GLOBAL_SUBSCRIBER.lock() {
                *global = None;
            }
        }
    });

    Ok(ShutdownHandle::new(shutdown_sender))
}
