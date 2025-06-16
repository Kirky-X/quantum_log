//! 数据库 Sink 模块
//!
//! 此模块提供了将日志写入各种数据库的功能，支持 SQLite、MySQL 和 PostgreSQL。

pub mod models;
pub mod schema;
pub mod sink;

pub use models::*;
pub use schema::*;
pub use sink::DatabaseSink;
