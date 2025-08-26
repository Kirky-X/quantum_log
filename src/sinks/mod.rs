//! QuantumLog Sinks 模块
//!
//! 提供各种日志输出目标的实现，包括控制台、文件、数据库和网络输出。
//!
//! # 0.2.0 版本新特性
//!
//! - 统一的 Sink trait 接口
//! - 管道系统支持多 sink 协调
//! - 独占型和可叠加型 sink 区分
//! - 默认标准输出实现

pub mod console;
pub mod default_stdout;
pub mod file;
pub mod file_common;
pub mod level_file;
pub mod network;
pub mod pipeline;
pub mod rolling_file;
pub mod stdout;
pub mod traits;

// 条件编译数据库功能
#[cfg(feature = "db")]
pub mod database;

// InfluxDB功能
pub mod influxdb;

// 重新导出主要类型
pub use console::ConsoleSink;
pub use file::FileSink;
pub use level_file::LevelFileSink;
pub use network::NetworkSink;
pub use pipeline::{ErrorStrategy, Pipeline, PipelineBuilder, PipelineConfig};
pub use rolling_file::RollingFileSink;
pub use stdout::StdoutSink;
pub use traits::{
    ExclusiveSink, QuantumSink, SinkError, SinkFactory, SinkMetadata, SinkResult, SinkType,
};

#[cfg(feature = "db")]
pub use database::DatabaseSink;

// InfluxDB Sink
pub use influxdb::InfluxDBSink;

// 可叠加型 Sink 仅供包内部使用，不对外公开
