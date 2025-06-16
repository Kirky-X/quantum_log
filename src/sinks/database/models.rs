//! QuantumLog 数据库模型定义
//!
//! 此模块定义了与数据库表对应的 Rust 结构体，用于 Diesel ORM 操作。

#[cfg(feature = "database")]
use chrono::{DateTime, NaiveDateTime, Utc};
#[cfg(feature = "database")]
use crate::sinks::database::schema::quantum_logs;
#[cfg(feature = "database")]
use diesel::prelude::*;
#[cfg(feature = "database")]
use serde::{Deserialize, Serialize};

/// 用于插入新日志记录的结构体
#[cfg(feature = "database")]
#[derive(Debug, Clone, Insertable, Serialize, Deserialize)]
#[diesel(table_name = quantum_logs)]
pub struct NewQuantumLogEntry {
    /// 日志时间戳
    pub timestamp: NaiveDateTime,

    /// 日志级别
    pub level: String,

    /// 日志目标
    pub target: String,

    /// 日志消息
    pub message: String,

    /// 额外字段（JSON 格式）
    pub fields: Option<String>,

    /// Span ID
    pub span_id: Option<String>,

    /// Span 名称
    pub span_name: Option<String>,

    /// 进程 ID
    pub process_id: i32,

    /// 线程 ID
    pub thread_id: String,

    /// 主机名
    pub hostname: String,

    /// 用户名
    pub username: String,

    /// MPI Rank
    pub mpi_rank: Option<i32>,

    /// 源文件路径
    pub file_path: Option<String>,

    /// 源代码行号
    pub line_number: Option<i32>,

    /// 模块路径
    pub module_path: Option<String>,
}

/// 从数据库查询的完整日志条目
///
/// 此结构体包含数据库中的所有字段，包括自动生成的 ID。
#[cfg(feature = "database")]
#[derive(Queryable, Selectable, Debug, Clone, Serialize, Deserialize)]
#[diesel(table_name = quantum_logs)]
pub struct QuantumLogEntry {
    /// 主键，自增 ID
    pub id: i32,

    /// 日志时间戳（UTC）
    pub timestamp: NaiveDateTime,

    /// 日志级别（如 "INFO", "ERROR" 等）
    pub level: String,

    /// 日志目标（通常是模块路径）
    pub target: String,

    /// 日志消息内容
    pub message: String,

    /// 额外字段（JSON 格式，可选）
    pub fields: Option<String>,

    /// Span ID
    pub span_id: Option<String>,

    /// Span 名称
    pub span_name: Option<String>,

    /// 进程 ID
    pub process_id: i32,

    /// 线程 ID
    pub thread_id: String,

    /// 主机名
    pub hostname: String,

    /// 用户名
    pub username: String,

    /// MPI Rank 号（可选）
    pub mpi_rank: Option<i32>,

    /// 源文件路径
    pub file_path: Option<String>,

    /// 源代码行号（可选）
    pub line_number: Option<i32>,

    /// 模块路径
    pub module_path: Option<String>,
}

#[cfg(feature = "database")]
impl NewQuantumLogEntry {
    /// 创建一个新的日志条目
    pub fn new(
        timestamp: NaiveDateTime,
        level: String,
        target: String,
        message: String,
        process_id: i32,
        thread_id: String,
        hostname: String,
        username: String,
    ) -> Self {
        Self {
            timestamp,
            level,
            target,
            message,
            fields: None,
            span_id: None,
            span_name: None,
            process_id,
            thread_id,
            hostname,
            username,
            mpi_rank: None,
            file_path: None,
            line_number: None,
            module_path: None,
        }
    }

    /// 设置源文件信息
    pub fn with_file_info(mut self, file_path: Option<String>, line_number: Option<i32>) -> Self {
        self.file_path = file_path;
        self.line_number = line_number;
        self
    }

    /// 设置模块路径
    pub fn with_module_path(mut self, module_path: Option<String>) -> Self {
        self.module_path = module_path;
        self
    }

    /// 设置 MPI Rank 信息
    pub fn with_mpi_rank(mut self, mpi_rank: Option<i32>) -> Self {
        self.mpi_rank = mpi_rank;
        self
    }

    /// 设置 Span 信息
    pub fn with_span_info(mut self, span_id: Option<String>, span_name: Option<String>) -> Self {
        self.span_id = span_id;
        self.span_name = span_name;
        self
    }

    /// 设置额外字段
    pub fn with_fields(mut self, fields: Option<String>) -> Self {
        self.fields = fields;
        self
    }
}

#[cfg(feature = "database")]
impl QuantumLogEntry {
    /// 获取格式化的时间戳字符串
    pub fn formatted_timestamp(&self, format: &str) -> String {
        self.timestamp.format(format).to_string()
    }

    /// 检查是否包含指定的字段
    pub fn has_field(&self, field_name: &str) -> bool {
        if let Some(fields_json) = &self.fields {
            if let Ok(fields) = serde_json::from_str::<serde_json::Value>(fields_json) {
                return fields.get(field_name).is_some();
            }
        }
        false
    }

    /// 获取指定字段的值
    pub fn get_field(&self, field_name: &str) -> Option<serde_json::Value> {
        if let Some(fields_json) = &self.fields {
            if let Ok(fields) = serde_json::from_str::<serde_json::Value>(fields_json) {
                return fields.get(field_name).cloned();
            }
        }
        None
    }

    /// 获取解析后的 Span 信息
    pub fn get_span_info(&self) -> Option<serde_json::Value> {
        if let Some(span_json) = &self.span_id {
            serde_json::from_str(span_json).ok()
        } else {
            None
        }
    }
}

/// 用于批量插入的辅助结构
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct LogBatch {
    /// 批次中的日志条目
    pub entries: Vec<NewQuantumLogEntry>,
    /// 批次创建时间
    pub created_at: DateTime<Utc>,
}

#[cfg(feature = "database")]
impl LogBatch {
    /// 创建一个新的日志批次
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            created_at: Utc::now(),
        }
    }

    /// 添加日志条目到批次
    pub fn add_entry(&mut self, entry: NewQuantumLogEntry) {
        self.entries.push(entry);
    }

    /// 获取批次大小
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 检查批次是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 清空批次
    pub fn clear(&mut self) {
        self.entries.clear();
        self.created_at = Utc::now();
    }

    /// 检查批次是否已满
    pub fn is_full(&self, max_size: usize) -> bool {
        self.entries.len() >= max_size
    }

    /// 检查批次是否超时
    pub fn is_expired(&self, timeout_seconds: u64) -> bool {
        let elapsed = Utc::now().signed_duration_since(self.created_at);
        elapsed.num_seconds() as u64 >= timeout_seconds
    }
}

#[cfg(feature = "database")]
impl Default for LogBatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_new_quantum_log_entry_creation() {
        let timestamp = Utc::now().naive_utc();
        let entry = NewQuantumLogEntry::new(
            timestamp,
            "INFO".to_string(),
            "test::module".to_string(),
            "Test message".to_string(),
            1234,
            "thread-1".to_string(),
            "localhost".to_string(),
            "testuser".to_string(),
        );

        assert_eq!(entry.timestamp, timestamp);
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.target, "test::module");
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.process_id, 1234);
        assert_eq!(entry.thread_id, "thread-1");
        assert_eq!(entry.hostname, "localhost");
        assert_eq!(entry.username, "testuser");
        assert!(entry.file_path.is_none());
        assert!(entry.line_number.is_none());
    }

    #[test]
    fn test_entry_builder_methods() {
        let timestamp = Utc::now().naive_utc();
        let entry = NewQuantumLogEntry::new(
            timestamp,
            "ERROR".to_string(),
            "test::module".to_string(),
            "Error message".to_string(),
            1234,
            "thread-1".to_string(),
            "hostname".to_string(),
            "user".to_string(),
        )
        .with_file_info(Some("test.rs".to_string()), Some(42))
        .with_module_path(Some("test::module".to_string()))
        .with_mpi_rank(Some(0))
        .with_span_info(Some("abc123".to_string()), Some("test_span".to_string()))
        .with_fields(Some("{\"custom\":\"value\"}".to_string()));

        assert_eq!(entry.file_path, Some("test.rs".to_string()));
        assert_eq!(entry.line_number, Some(42));
        assert_eq!(entry.module_path, Some("test::module".to_string()));
        assert_eq!(entry.process_id, 1234);
        assert_eq!(entry.thread_id, "thread-1");
        assert_eq!(entry.mpi_rank, Some(0));
        assert_eq!(entry.username, "user");
        assert_eq!(entry.hostname, "hostname");
        assert_eq!(entry.span_id, Some("abc123".to_string()));
        assert_eq!(entry.span_name, Some("test_span".to_string()));
        assert_eq!(entry.fields, Some("{\"custom\":\"value\"}".to_string()));
    }

    #[test]
    fn test_log_batch_operations() {
        let mut batch = LogBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);

        let entry = NewQuantumLogEntry::new(
            Utc::now().naive_utc(),
            "INFO".to_string(),
            "test".to_string(),
            "message".to_string(),
            1234,
            "5678".to_string(),
            "localhost".to_string(),
            "testuser".to_string(),
        );

        batch.add_entry(entry);
        assert!(!batch.is_empty());
        assert_eq!(batch.len(), 1);

        assert!(batch.is_full(1));
        assert!(!batch.is_full(2));

        batch.clear();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_batch_expiration() {
        let mut batch = LogBatch::new();
        // 新创建的批次不应该过期
        assert!(!batch.is_expired(60));

        // 手动设置一个过去的时间
        batch.created_at = Utc::now() - chrono::Duration::seconds(120);
        assert!(batch.is_expired(60));
    }

    #[test]
    fn test_quantum_log_entry_field_operations() {
        let entry = QuantumLogEntry {
            id: 1,
            timestamp: Utc::now().naive_utc(),
            level: "ERROR".to_string(),
            target: "test::ops".to_string(),
            message: "Field operations test".to_string(),
            file_path: Some("ops.rs".to_string()),
            line_number: Some(100),
            module_path: Some("test::ops".to_string()),
            process_id: 9999,
            thread_id: "thread-8888".to_string(),
            hostname: "ophost".to_string(),
            username: "opuser".to_string(),
            mpi_rank: Some(1),
            span_id: Some("{\"operation\":\"test\"}".to_string()),
            span_name: Some("test_operation".to_string()),
            fields: Some("{\"severity\":\"high\"}".to_string()),
        };

        // 测试字段访问
        assert_eq!(entry.id, 1);
        assert_eq!(entry.level, "ERROR");
        assert_eq!(entry.file_path, Some("ops.rs".to_string()));
        assert_eq!(entry.line_number, Some(100));
        assert_eq!(entry.process_id, 9999);
        assert_eq!(entry.thread_id, "thread-8888");

        // 测试 JSON 解析
        let span_info = entry.get_span_info();
        assert!(span_info.is_some());

        assert!(entry.has_field("severity"));
        assert!(!entry.has_field("nonexistent"));

        let severity_value = entry.get_field("severity");
        assert!(severity_value.is_some());
        assert_eq!(
            severity_value.unwrap(),
            serde_json::Value::String("high".to_string())
        );
    }
}
