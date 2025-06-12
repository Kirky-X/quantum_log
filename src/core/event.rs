//! QuantumLog 事件定义
//!
//! 此模块定义了 QuantumLog 系统中使用的核心事件结构。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{Level, Metadata};

/// QuantumLog 事件结构
///
/// 包含日志事件的所有信息，包括时间戳、级别、消息、字段和上下文信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumLogEvent {
    /// 事件时间戳
    pub timestamp: DateTime<Utc>,
    /// 日志级别
    pub level: String,
    /// 日志消息
    pub message: String,
    /// 目标模块
    pub target: String,
    /// 文件名
    pub file: Option<String>,
    /// 行号
    pub line: Option<u32>,
    /// 模块路径
    pub module_path: Option<String>,
    /// 自定义字段
    pub fields: HashMap<String, serde_json::Value>,
    /// 上下文信息
    pub context: ContextInfo,
}

/// 上下文信息
///
/// 包含进程、线程、用户等背景信息。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextInfo {
    /// 进程 ID
    pub pid: u32,
    /// 线程 ID
    pub tid: u64,
    /// 用户名
    pub username: Option<String>,
    /// 主机名
    pub hostname: Option<String>,
    /// MPI Rank (如果可用)
    pub mpi_rank: Option<i32>,
    /// 自定义上下文字段
    pub custom_fields: HashMap<String, serde_json::Value>,
}

impl QuantumLogEvent {
    /// 创建新的 QuantumLog 事件
    pub fn new(
        level: Level,
        message: String,
        metadata: &Metadata<'_>,
        fields: HashMap<String, serde_json::Value>,
        context: ContextInfo,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            level: level.to_string(),
            message,
            target: metadata.target().to_string(),
            file: metadata.file().map(|s| s.to_string()),
            line: metadata.line(),
            module_path: metadata.module_path().map(|s| s.to_string()),
            fields,
            context,
        }
    }

    /// 获取事件的 JSON 表示
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 获取事件的格式化字符串表示
    pub fn to_formatted_string(&self, format: &str) -> String {
        match format {
            "json" => self.to_json().unwrap_or_else(|_| "<serialization error>".to_string()),
            "compact" => {
                format!(
                    "[{}] {} {}: {}",
                    self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                    self.level,
                    self.target,
                    self.message
                )
            }
            "full" => {
                let mut result = format!(
                    "[{}] {} {}:{} {}",
                    self.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
                    self.level,
                    self.target,
                    self.line.map_or("?".to_string(), |l| l.to_string()),
                    self.message
                );
                
                if !self.fields.is_empty() {
                    result.push_str(" {");
                    for (key, value) in &self.fields {
                        result.push_str(&format!(" {}={}", key, value));
                    }
                    result.push_str(" }");
                }
                
                result
            }
            _ => self.message.clone(),
        }
    }

    /// 获取 CSV 行数据
    pub fn to_csv_row(&self) -> Vec<String> {
        vec![
            self.timestamp.to_rfc3339(),
            self.level.clone(),
            self.target.clone(),
            self.file.clone().unwrap_or_default(),
            self.line.map_or(String::new(), |l| l.to_string()),
            self.module_path.clone().unwrap_or_default(),
            self.message.clone(),
            self.context.pid.to_string(),
            self.context.tid.to_string(),
            self.context.username.clone().unwrap_or_default(),
            self.context.hostname.clone().unwrap_or_default(),
            self.context.mpi_rank.map_or(String::new(), |r| r.to_string()),
            serde_json::to_string(&self.fields).unwrap_or_default(),
            serde_json::to_string(&self.context.custom_fields).unwrap_or_default(),
        ]
    }

    /// 获取 CSV 头部
    pub fn csv_headers() -> Vec<String> {
        vec![
            "timestamp".to_string(),
            "level".to_string(),
            "target".to_string(),
            "file".to_string(),
            "line".to_string(),
            "module_path".to_string(),
            "message".to_string(),
            "pid".to_string(),
            "tid".to_string(),
            "username".to_string(),
            "hostname".to_string(),
            "mpi_rank".to_string(),
            "fields".to_string(),
            "custom_context".to_string(),
        ]
    }
}



impl ContextInfo {
    /// 创建新的上下文信息
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置自定义字段
    pub fn with_custom_field(mut self, key: String, value: serde_json::Value) -> Self {
        self.custom_fields.insert(key, value);
        self
    }

    /// 添加自定义字段
    pub fn add_custom_field(&mut self, key: String, value: serde_json::Value) {
        self.custom_fields.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;

    #[test]
    fn test_quantum_log_event_creation() {
        static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
            "test",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        ));
        let metadata = tracing::Metadata::new(
            "test",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        let context = ContextInfo {
            pid: 1234,
            tid: 5678,
            username: Some("test_user".to_string()),
            hostname: Some("test_host".to_string()),
            mpi_rank: Some(0),
            custom_fields: HashMap::new(),
        };

        let event = QuantumLogEvent::new(
            Level::INFO,
            "Test message".to_string(),
            &metadata,
            HashMap::new(),
            context,
        );

        assert_eq!(event.level, "INFO");
        assert_eq!(event.message, "Test message");
        assert_eq!(event.target, "test_target");
        assert_eq!(event.file, Some("test.rs".to_string()));
        assert_eq!(event.line, Some(42));
        assert_eq!(event.context.pid, 1234);
        assert_eq!(event.context.tid, 5678);
    }

    #[test]
    fn test_event_formatting() {
        static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
            "test",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        ));
        let context = ContextInfo::default();
        let metadata = tracing::Metadata::new(
            "test",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        let event = QuantumLogEvent::new(
            Level::INFO,
            "Test message".to_string(),
            &metadata,
            HashMap::new(),
            context,
        );

        let compact = event.to_formatted_string("compact");
        assert!(compact.contains("INFO"));
        assert!(compact.contains("test_target"));
        assert!(compact.contains("Test message"));

        let json = event.to_formatted_string("json");
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
    }

    #[test]
    fn test_csv_functionality() {
        static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
            "test",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        ));
        let context = ContextInfo::default();
        let metadata = tracing::Metadata::new(
            "test",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        let event = QuantumLogEvent::new(
            Level::INFO,
            "Test message".to_string(),
            &metadata,
            HashMap::new(),
            context,
        );

        let headers = QuantumLogEvent::csv_headers();
        let row = event.to_csv_row();
        
        assert_eq!(headers.len(), row.len());
        assert!(headers.contains(&"timestamp".to_string()));
        assert!(headers.contains(&"level".to_string()));
        assert!(headers.contains(&"message".to_string()));
    }
}