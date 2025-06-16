//! Sinks 模块
//!
//! 提供各种日志输出目标的实现

pub mod console;
#[cfg(feature = "database")]
pub mod database;
pub mod file;
pub mod file_common;
pub mod level_file;
pub mod network;
pub mod rolling_file;
pub mod stdout;

// 重新导出主要类型
pub use console::ConsoleSink;
#[cfg(feature = "database")]
pub use database::DatabaseSink;
pub use file::FileSink;
pub use level_file::LevelFileSink;
pub use network::NetworkSink;
pub use rolling_file::RollingFileSink;
pub use stdout::StdoutSink;
