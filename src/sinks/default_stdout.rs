//! 默认标准输出 Sink 实现
//!
//! 提供开箱即用的标准输出日志功能，实现了 StackableSink trait，
//! 可以与其他可叠加型 sink 同时使用。
//!
//! # 特性
//!
//! - 支持彩色输出
//! - 支持多种输出格式
//! - 支持日志级别过滤
//! - 线程安全的异步实现
//! - 可配置的输出目标（stdout/stderr）
//!
//! # 使用示例
//!
//! ```rust
//! use quantum_log::sinks::default_stdout::{DefaultStdoutSink, DefaultStdoutConfig, StdoutTarget};
//! use quantum_log::sinks::pipeline::PipelineBuilder;
//! use quantum_log::sinks::traits::{SinkMetadata, QuantumSink};
//! use quantum_log::config::OutputFormat;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = DefaultStdoutConfig {
//!         format: OutputFormat::Json,
//!         colored: true,
//!         target: StdoutTarget::Stdout,
//!         ..Default::default()
//!     };
//!
//!     let sink = DefaultStdoutSink::new(config).await?;
//!     let metadata = sink.metadata();
//!     
//!     let pipeline = PipelineBuilder::new()
//!         .add_stackable_sink(sink, metadata)
//!         .build();
//!
//!     Ok(())
//! }
//! ```

use crate::config::OutputFormat;
use crate::core::event::QuantumLogEvent;
use crate::sinks::traits::{QuantumSink, SinkError, SinkResult, StackableSink};
use async_trait::async_trait;
use serde_json;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::Level;

/// 标准输出目标
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdoutTarget {
    /// 标准输出
    Stdout,
    /// 标准错误
    Stderr,
}

impl Default for StdoutTarget {
    fn default() -> Self {
        Self::Stdout
    }
}

/// 默认标准输出 Sink 配置
#[derive(Debug, Clone)]
pub struct DefaultStdoutConfig {
    /// 输出格式
    pub format: OutputFormat,
    /// 是否启用彩色输出
    pub colored: bool,
    /// 输出目标
    pub target: StdoutTarget,
    /// 最小日志级别
    pub min_level: Option<Level>,
    /// 是否包含时间戳
    pub include_timestamp: bool,
    /// 是否包含线程信息
    pub include_thread: bool,
    /// 是否包含模块路径
    pub include_module: bool,
    /// 自定义前缀
    pub prefix: Option<String>,
}

impl Default for DefaultStdoutConfig {
    fn default() -> Self {
        Self {
            format: OutputFormat::Text,
            colored: true,
            target: StdoutTarget::Stdout,
            min_level: None,
            include_timestamp: true,
            include_thread: false,
            include_module: true,
            prefix: None,
        }
    }
}

/// 默认标准输出 Sink 实现
///
/// 这是一个可叠加型 sink，可以与其他 sink 同时使用。
/// 提供了丰富的配置选项和高性能的异步输出。
#[derive(Debug)]
pub struct DefaultStdoutSink {
    /// 配置
    config: DefaultStdoutConfig,
    /// 输出锁，确保线程安全
    output_lock: Arc<Mutex<()>>,
    /// 是否已关闭
    is_closed: AtomicBool,
    /// 发送的事件计数
    event_count: AtomicU64,
    /// 错误计数
    error_count: AtomicU64,
}

impl DefaultStdoutSink {
    /// 创建新的默认标准输出 sink
    pub async fn new(config: DefaultStdoutConfig) -> SinkResult<Self> {
        Ok(Self {
            config,
            output_lock: Arc::new(Mutex::new(())),
            is_closed: AtomicBool::new(false),
            event_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        })
    }

    /// 使用默认配置创建 sink
    pub async fn with_default_config() -> SinkResult<Self> {
        Self::new(DefaultStdoutConfig::default()).await
    }

    /// 检查事件是否应该被过滤
    fn should_filter_event(&self, event: &QuantumLogEvent) -> bool {
        if let Some(min_level) = &self.config.min_level {
            // 简单的级别比较，实际实现可能需要更复杂的逻辑
            match (event.level.as_str(), min_level) {
                ("ERROR", _) => false,
                ("WARN", &Level::ERROR) => true,
                ("WARN", _) => false,
                ("INFO", &Level::ERROR | &Level::WARN) => true,
                ("INFO", _) => false,
                ("DEBUG", &Level::ERROR | &Level::WARN | &Level::INFO) => true,
                ("DEBUG", _) => false,
                ("TRACE", &Level::TRACE) => false,
                ("TRACE", _) => true,
                _ => false,
            }
        } else {
            false
        }
    }

    /// 格式化事件为文本
    fn format_as_text(&self, event: &QuantumLogEvent) -> String {
        use std::fmt::Write;
        let mut output = String::with_capacity(256);

        // 添加自定义前缀
        if let Some(prefix) = &self.config.prefix {
            output.push_str(prefix);
            output.push(' ');
        }

        // 添加时间戳
        if self.config.include_timestamp {
            output.push('[');
            write!(output, "{}", event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f")).unwrap();
            output.push_str("] ");
        }

        // 添加日志级别
        if self.config.colored {
            let colored_level = match event.level.as_str() {
                "ERROR" => "\x1b[31mERROR\x1b[0m", // 红色
                "WARN" => "\x1b[33mWARN\x1b[0m",   // 黄色
                "INFO" => "\x1b[32mINFO\x1b[0m",   // 绿色
                "DEBUG" => "\x1b[36mDEBUG\x1b[0m", // 青色
                "TRACE" => "\x1b[37mTRACE\x1b[0m", // 白色
                _ => &event.level,
            };
            output.push('[');
            output.push_str(colored_level);
            output.push_str("] ");
        } else {
            output.push('[');
            output.push_str(&event.level);
            output.push_str("] ");
        }

        // 添加模块路径
        if self.config.include_module {
            if let Some(module) = &event.module_path {
                output.push('[');
                output.push_str(module);
                output.push_str("] ");
            }
        }

        // 添加线程信息
        if self.config.include_thread {
            if let Some(thread_name) = &event.thread_name {
                output.push('[');
                output.push_str(thread_name);
                output.push_str("] ");
            } else {
                output.push('[');
                write!(output, "{}", event.context.tid).unwrap();
                output.push_str("] ");
            }
        }

        // 添加消息
        output.push_str(&event.message);

        // 添加字段
        if !event.fields.is_empty() {
            output.push_str(" {");
            for (i, (key, value)) in event.fields.iter().enumerate() {
                if i > 0 {
                    output.push_str(", ");
                }
                output.push_str(key);
                output.push_str(": ");
                match value {
                    serde_json::Value::String(s) => output.push_str(s),
                    _ => write!(output, "{}", value).unwrap(),
                }
            }
            output.push('}');
        }

        output
    }

    /// 格式化事件为 JSON
    fn format_as_json(&self, event: &QuantumLogEvent) -> SinkResult<String> {
        let mut json_event = serde_json::Map::with_capacity(10);

        // 基本字段
        json_event.insert(
            "timestamp".into(),
            serde_json::Value::String(event.timestamp.to_rfc3339()),
        );
        json_event.insert(
            "level".into(),
            serde_json::Value::String(event.level.clone()),
        );
        json_event.insert(
            "message".into(),
            serde_json::Value::String(event.message.clone()),
        );

        // 可选字段
        if let Some(module) = &event.module_path {
            json_event.insert(
                "module".into(),
                serde_json::Value::String(module.clone()),
            );
        }
        if let Some(file) = &event.file {
            json_event.insert("file".into(), serde_json::Value::String(file.clone()));
        }
        if let Some(line) = event.line {
            json_event.insert(
                "line".into(),
                serde_json::Value::Number(serde_json::Number::from(line)),
            );
        }
        // 添加线程信息
        // 添加线程信息
        json_event.insert(
            "thread_id".into(),
            serde_json::Value::String(event.context.tid.to_string()),
        );

        // 自定义前缀
        if let Some(prefix) = &self.config.prefix {
            json_event.insert(
                "prefix".into(),
                serde_json::Value::String(prefix.clone()),
            );
        }

        // 字段
        if !event.fields.is_empty() {
            let fields_map: serde_json::Map<String, serde_json::Value> = event.fields.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            json_event.insert("fields".into(), serde_json::Value::Object(fields_map));
        }

        serde_json::to_string(&json_event).map_err(|e| SinkError::Serialization(e.to_string()))
    }

    /// 格式化事件为 CSV
    fn format_as_csv(&self, event: &QuantumLogEvent) -> SinkResult<String> {
        let mut csv_fields = Vec::with_capacity(10);
        
        // 基本字段
        csv_fields.push(self.escape_csv_field(&event.timestamp.to_rfc3339()));
        csv_fields.push(self.escape_csv_field(&event.level));
        csv_fields.push(self.escape_csv_field(&event.message));
        // 可选字段
        csv_fields.push(self.escape_csv_field(event.module_path.as_deref().unwrap_or("")));
        csv_fields.push(self.escape_csv_field(event.file.as_deref().unwrap_or("")));
        
        // 行号处理 - 避免不必要的字符串分配
        let line_str;
        let line_field = if let Some(line) = event.line {
            line_str = line.to_string();
            &line_str
        } else {
            ""
        };
        csv_fields.push(self.escape_csv_field(line_field));
        
        // 线程ID - 避免不必要的字符串分配
        let tid_str = event.context.tid.to_string();
        csv_fields.push(self.escape_csv_field(&tid_str));
        csv_fields.push(self.escape_csv_field(""));  // 保持 CSV 格式一致性
        // 自定义前缀
        csv_fields.push(self.escape_csv_field(self.config.prefix.as_deref().unwrap_or("")));

        // 字段（序列化为JSON字符串）
        let fields_json = if event.fields.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&event.fields)
                .map_err(|e| SinkError::Serialization(e.to_string()))?
        };
        csv_fields.push(self.escape_csv_field(&fields_json));

        Ok(csv_fields.join(","))
    }

    /// 转义 CSV 字段
    fn escape_csv_field(&self, field: &str) -> String {
        if field.contains(',') || field.contains('"') || field.contains('\n') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }

    /// 输出格式化的内容
    async fn write_output(&self, content: &str) -> SinkResult<()> {
        let _lock = self.output_lock.lock().await;

        match self.config.target {
            StdoutTarget::Stdout => {
                print!("{}", content);
                io::stdout().flush().map_err(SinkError::Io)?
            }
            StdoutTarget::Stderr => {
                eprint!("{}", content);
                io::stderr().flush().map_err(SinkError::Io)?
            }
        }

        Ok(())
    }
}

#[async_trait]
impl QuantumSink for DefaultStdoutSink {
    type Config = DefaultStdoutConfig;
    type Error = SinkError;

    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
        if self.is_closed.load(Ordering::Relaxed) {
            return Err(SinkError::Closed);
        }

        // 检查是否应该过滤此事件
        if self.should_filter_event(&event) {
            return Ok(());
        }

        // 格式化事件
        let formatted = match self.config.format {
            OutputFormat::Text => {
                format!("{}\n", self.format_as_text(&event))
            }
            OutputFormat::Json => {
                format!("{}\n", self.format_as_json(&event)?)
            }
            OutputFormat::Csv => {
                format!("{}\n", self.format_as_csv(&event)?)
            }
        };

        // 输出内容
        match self.write_output(&formatted).await {
            Ok(()) => {
                self.event_count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                self.error_count.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    async fn shutdown(&self) -> Result<(), Self::Error> {
        self.is_closed.store(true, Ordering::Relaxed);

        // 确保所有输出都被刷新
        let _lock = self.output_lock.lock().await;
        match self.config.target {
            StdoutTarget::Stdout => io::stdout().flush().map_err(SinkError::Io)?,
            StdoutTarget::Stderr => io::stderr().flush().map_err(SinkError::Io)?,
        }

        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        !self.is_closed.load(Ordering::Relaxed)
    }

    fn name(&self) -> &'static str {
        "default_stdout"
    }

    fn stats(&self) -> String {
        format!(
            "DefaultStdoutSink: events={}, errors={}, target={:?}, format={:?}",
            self.event_count.load(Ordering::Relaxed),
            self.error_count.load(Ordering::Relaxed),
            self.config.target,
            self.config.format
        )
    }

    fn metadata(&self) -> crate::sinks::traits::SinkMetadata {
        crate::sinks::traits::SinkMetadata {
            name: "default_stdout".to_string(),
            sink_type: crate::sinks::traits::SinkType::Stackable,
            enabled: !self.is_closed.load(std::sync::atomic::Ordering::Relaxed),
            description: Some(format!(
                "Default stdout sink targeting {:?} with {:?} format",
                self.config.target, self.config.format
            )),
        }
    }
}

// 实现 StackableSink trait，标记这是一个可叠加的 sink
#[async_trait]
impl StackableSink for DefaultStdoutSink {
    async fn send_event_internal(
        &self,
        event: &QuantumLogEvent,
        strategy: crate::config::BackpressureStrategy,
    ) -> SinkResult<()> {
        use crate::diagnostics::get_diagnostics_instance;
        use tokio::time::{timeout, Duration};

        // 先应用级别过滤
        if self.should_filter_event(event) {
            return Ok(());
        }

        let diagnostics = get_diagnostics_instance();

        // 根据配置格式化
        let content = match self.config.format {
            OutputFormat::Text => self.format_as_text(event),
            OutputFormat::Json => self.format_as_json(event)?,
            OutputFormat::Csv => self.format_as_csv(event)?,
        };

        // 写入输出，支持背压策略
        let write_fut = self.write_output(&content);
        let result = match strategy {
            crate::config::BackpressureStrategy::Block => write_fut.await,
            crate::config::BackpressureStrategy::Drop => {
                match timeout(Duration::from_millis(100), write_fut).await {
                    Ok(res) => res,
                    Err(_) => return Err(SinkError::Backpressure),
                }
            }
        };

        match result {
            Ok(()) => {
                if let Some(d) = diagnostics {
                    d.increment_stdout_writes();
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

/// 便利函数：创建默认配置的标准输出 sink
pub async fn create_default_stdout_sink() -> SinkResult<DefaultStdoutSink> {
    DefaultStdoutSink::with_default_config().await
}

/// 便利函数：创建 JSON 格式的标准输出 sink
pub async fn create_json_stdout_sink() -> SinkResult<DefaultStdoutSink> {
    let config = DefaultStdoutConfig {
        format: OutputFormat::Json,
        ..Default::default()
    };
    DefaultStdoutSink::new(config).await
}

/// 便利函数：创建无彩色的标准输出 sink
pub async fn create_plain_stdout_sink() -> SinkResult<DefaultStdoutSink> {
    let config = DefaultStdoutConfig {
        colored: false,
        ..Default::default()
    };
    DefaultStdoutSink::new(config).await
}

/// 便利函数：创建标准错误输出 sink
pub async fn create_stderr_sink() -> SinkResult<DefaultStdoutSink> {
    let config = DefaultStdoutConfig {
        target: StdoutTarget::Stderr,
        ..Default::default()
    };
    DefaultStdoutSink::new(config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::QuantumLogEvent;
    use chrono::Utc;
    use std::collections::HashMap;
    use tokio::time::{sleep, Duration};
    use tracing::Level;

    fn create_test_event(level: &str, message: &str) -> QuantumLogEvent {
        QuantumLogEvent {
            timestamp: Utc::now(),
            level: level.to_string(),
            target: "test".to_string(),
            message: message.to_string(),
            module_path: Some("test::module".to_string()),
            file: Some("test.rs".to_string()),
            line: Some(42),
            thread_name: Some("test-thread".to_string()),
            thread_id: "test-thread-id".to_string(),
            fields: HashMap::new(),
            context: crate::core::event::ContextInfo {
                pid: std::process::id(),
                tid: 12345,
                username: None,
                hostname: None,
                mpi_rank: None,
                custom_fields: std::collections::HashMap::new(),
            },
        }
    }

    #[tokio::test]
    async fn test_default_stdout_sink_creation() {
        let sink = DefaultStdoutSink::with_default_config().await;
        assert!(sink.is_ok());

        let sink = sink.unwrap();
        assert!(matches!(sink.config.format, OutputFormat::Text));
        assert!(sink.config.colored);
        assert!(matches!(sink.config.target, StdoutTarget::Stdout));
    }

    #[tokio::test]
    async fn test_custom_config_creation() {
        let config = DefaultStdoutConfig {
            format: OutputFormat::Json,
            colored: false,
            target: StdoutTarget::Stderr,
            min_level: Some(Level::WARN),
            include_timestamp: false,
            include_thread: true,
            include_module: false,
            prefix: Some("TEST".to_string()),
        };

        let sink = DefaultStdoutSink::new(config).await;
        assert!(sink.is_ok());

        let sink = sink.unwrap();
        assert!(matches!(sink.config.format, OutputFormat::Json));
        assert!(!sink.config.colored);
        assert!(matches!(sink.config.target, StdoutTarget::Stderr));
        assert!(sink.config.min_level.is_some());
    }

    #[tokio::test]
    async fn test_send_event_basic() {
        let sink = DefaultStdoutSink::with_default_config().await.unwrap();
        let event = create_test_event("INFO", "Test message");

        let result = sink.send_event(event).await;
        assert!(result.is_ok());

        assert_eq!(sink.event_count.load(Ordering::Relaxed), 1);
        assert_eq!(sink.error_count.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_level_filtering() {
        let config = DefaultStdoutConfig {
            min_level: Some(Level::WARN),
            ..Default::default()
        };
        let sink = DefaultStdoutSink::new(config).await.unwrap();

        // INFO 级别应该被过滤
        let info_event = create_test_event("INFO", "Info message");
        let result = sink.send_event(info_event).await;
        assert!(result.is_ok());
        assert_eq!(sink.event_count.load(Ordering::Relaxed), 0);

        // ERROR 级别应该通过
        let error_event = create_test_event("ERROR", "Error message");
        let result = sink.send_event(error_event).await;
        assert!(result.is_ok());
        assert_eq!(sink.event_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_format_as_text() {
        let config = DefaultStdoutConfig {
            colored: false,
            include_timestamp: true,
            include_module: true,
            include_thread: true,
            prefix: Some("TEST".to_string()),
            ..Default::default()
        };
        let sink = DefaultStdoutSink::new(config).await.unwrap();

        let mut event = create_test_event("INFO", "Test message");
        event.fields.insert(
            "key1".to_string(),
            serde_json::Value::String("value1".to_string()),
        );
        event.fields.insert(
            "key2".to_string(),
            serde_json::Value::String("value2".to_string()),
        );

        let formatted = sink.format_as_text(&event);

        assert!(formatted.contains("TEST"));
        assert!(formatted.contains("[INFO]"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("test::module"));
        assert!(formatted.contains("test-thread"));
        assert!(formatted.contains("key1: value1"));
        assert!(formatted.contains("key2: value2"));
    }

    #[tokio::test]
    async fn test_format_as_json() {
        let config = DefaultStdoutConfig {
            format: OutputFormat::Json,
            prefix: Some("TEST".to_string()),
            ..Default::default()
        };
        let sink = DefaultStdoutSink::new(config).await.unwrap();

        let mut event = create_test_event("INFO", "Test message");
        event.fields.insert(
            "key1".to_string(),
            serde_json::Value::String("value1".to_string()),
        );

        let formatted = sink.format_as_json(&event);
        assert!(formatted.is_ok());

        let json_str = formatted.unwrap();
        assert!(json_str.contains("\"level\":\"INFO\""));
        assert!(json_str.contains("\"message\":\"Test message\""));
        assert!(json_str.contains("\"module\":\"test::module\""));
        assert!(json_str.contains("\"prefix\":\"TEST\""));
        assert!(json_str.contains("\"fields\""));
        assert!(json_str.contains("\"key1\":\"value1\""));
    }

    #[tokio::test]
    async fn test_colored_output() {
        let config = DefaultStdoutConfig {
            colored: true,
            ..Default::default()
        };
        let sink = DefaultStdoutSink::new(config).await.unwrap();

        let levels = vec!["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

        for level in levels {
            let event = create_test_event(level, "Test message");
            let formatted = sink.format_as_text(&event);

            // 应该包含 ANSI 颜色代码
            assert!(formatted.contains("\x1b["));
            assert!(formatted.contains("\x1b[0m"));
        }
    }

    #[tokio::test]
    async fn test_shutdown() {
        let sink = DefaultStdoutSink::with_default_config().await.unwrap();

        assert!(sink.is_healthy().await);

        let result = sink.shutdown().await;
        assert!(result.is_ok());

        assert!(!sink.is_healthy().await);

        // 关闭后发送事件应该失败
        let event = create_test_event("INFO", "Test message");
        let result = sink.send_event(event).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SinkError::Closed));
    }

    #[tokio::test]
    async fn test_quantum_sink_trait() {
        let sink = DefaultStdoutSink::with_default_config().await.unwrap();

        assert_eq!(sink.name(), "default_stdout");
        assert!(sink.is_healthy().await);

        let stats = sink.stats();
        assert!(stats.contains("DefaultStdoutSink"));
        assert!(stats.contains("events=0"));
        assert!(stats.contains("errors=0"));
    }

    #[tokio::test]
    async fn test_stackable_sink_trait() {
        let sink = DefaultStdoutSink::with_default_config().await.unwrap();

        // 确保实现了 StackableSink trait
        // 注意：由于关联类型的存在，这里不能直接使用 dyn StackableSink
        // 但我们可以通过调用方法来验证trait的实现
        let _metadata = sink.metadata();
        assert!(_metadata.enabled);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let sink = std::sync::Arc::new(DefaultStdoutSink::with_default_config().await.unwrap());

        let mut handles = vec![];

        // 并发发送事件
        for i in 0..10 {
            let sink_clone = std::sync::Arc::clone(&sink);
            let handle = tokio::spawn(async move {
                let event = create_test_event("INFO", &format!("Message {}", i));
                sink_clone.send_event(event).await
            });
            handles.push(handle);
        }

        // 等待所有任务完成
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }

        assert_eq!(sink.event_count.load(Ordering::Relaxed), 10);
    }

    #[tokio::test]
    async fn test_convenience_functions() {
        // 测试便利函数
        let default_sink = create_default_stdout_sink().await;
        assert!(default_sink.is_ok());

        let json_sink = create_json_stdout_sink().await;
        assert!(json_sink.is_ok());
        let json_sink = json_sink.unwrap();
        assert!(matches!(json_sink.config.format, OutputFormat::Json));

        let plain_sink = create_plain_stdout_sink().await;
        assert!(plain_sink.is_ok());
        let plain_sink = plain_sink.unwrap();
        assert!(!plain_sink.config.colored);

        let stderr_sink = create_stderr_sink().await;
        assert!(stderr_sink.is_ok());
        let stderr_sink = stderr_sink.unwrap();
        assert!(matches!(stderr_sink.config.target, StdoutTarget::Stderr));
    }

    #[tokio::test]
    async fn test_should_filter_event() {
        let config = DefaultStdoutConfig {
            min_level: Some(Level::WARN),
            ..Default::default()
        };
        let sink = DefaultStdoutSink::new(config).await.unwrap();

        let trace_event = create_test_event("TRACE", "Trace message");
        let debug_event = create_test_event("DEBUG", "Debug message");
        let info_event = create_test_event("INFO", "Info message");
        let warn_event = create_test_event("WARN", "Warn message");
        let error_event = create_test_event("ERROR", "Error message");

        assert!(sink.should_filter_event(&trace_event));
        assert!(sink.should_filter_event(&debug_event));
        assert!(sink.should_filter_event(&info_event));
        assert!(!sink.should_filter_event(&warn_event));
        assert!(!sink.should_filter_event(&error_event));
    }

    #[tokio::test]
    async fn test_stdout_target_default() {
        let default_target = StdoutTarget::default();
        assert!(matches!(default_target, StdoutTarget::Stdout));
    }

    #[tokio::test]
    async fn test_config_default() {
        let default_config = DefaultStdoutConfig::default();
        assert!(matches!(default_config.format, OutputFormat::Text));
        assert!(default_config.colored);
        assert!(matches!(default_config.target, StdoutTarget::Stdout));
        assert!(default_config.min_level.is_none());
        assert!(default_config.include_timestamp);
        assert!(!default_config.include_thread);
        assert!(default_config.include_module);
        assert!(default_config.prefix.is_none());
    }
}
