//! 格式化层
//!
//! 此层负责将日志事件格式化为指定的输出格式，支持多种格式如文本、JSON、CSV等。

use crate::config::{LogFormatConfig, LogFormatType};
use crate::core::event::{ContextInfo, QuantumLogEvent};
use crate::core::layers::context_injector::ContextInjectorLayer;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::Event;
// use tracing_subscriber::fmt::FormatEvent; // 暂时未使用
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// 格式化层
///
/// 此层负责将 tracing 事件转换为 QuantumLogEvent 并进行格式化。
#[derive(Clone)]
pub struct FormatterLayer {
    /// 格式化配置
    config: LogFormatConfig,
    /// 上下文注入器
    context_injector: Arc<ContextInjectorLayer>,
}

impl FormatterLayer {
    /// 创建新的格式化层
    pub fn new(config: LogFormatConfig, context_injector: Arc<ContextInjectorLayer>) -> Self {
        Self {
            config,
            context_injector,
        }
    }

    /// 将 tracing 事件转换为 QuantumLogEvent
    async fn convert_event(
        &self,
        event: &Event<'_>,
        context_info: ContextInfo,
    ) -> QuantumLogEvent {
        let metadata = event.metadata();
        
        // 提取事件字段
        let mut fields = HashMap::new();
        let mut message = String::new();
        
        // 创建一个访问器来提取字段
        struct FieldVisitor<'a> {
            fields: &'a mut HashMap<String, serde_json::Value>,
            message: &'a mut String,
        }
        
        impl<'a> tracing::field::Visit for FieldVisitor<'a> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                let value_str = format!("{:?}", value);
                if field.name() == "message" {
                    *self.message = value_str;
                } else {
                    self.fields.insert(
                        field.name().to_string(),
                        serde_json::Value::String(value_str),
                    );
                }
            }
            
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    *self.message = value.to_string();
                } else {
                    self.fields.insert(
                        field.name().to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
            }
            
            fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
                self.fields.insert(
                    field.name().to_string(),
                    serde_json::Value::Number(serde_json::Number::from(value)),
                );
            }
            
            fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                self.fields.insert(
                    field.name().to_string(),
                    serde_json::Value::Number(serde_json::Number::from(value)),
                );
            }
            
            fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
                self.fields.insert(
                    field.name().to_string(),
                    serde_json::Value::Bool(value),
                );
            }
        }
        
        let mut visitor = FieldVisitor {
            fields: &mut fields,
            message: &mut message,
        };
        
        event.record(&mut visitor);
        
        // 如果没有明确的消息字段，使用目标作为消息
        if message.is_empty() {
            message = format!("Event from {}", metadata.target());
        }
        
        QuantumLogEvent::new(
            *metadata.level(),
            message,
            metadata,
            fields,
            context_info,
        )
    }

    /// 格式化事件为指定格式
    pub fn format_event(&self, event: &QuantumLogEvent) -> String {
        match self.config.format_type {
            LogFormatType::Text => self.format_as_text(event),
            LogFormatType::Json => self.format_as_json(event),
            LogFormatType::Csv => self.format_as_csv(event),
        }
    }

    /// 格式化为文本格式
    fn format_as_text(&self, event: &QuantumLogEvent) -> String {
        let timestamp_format = &self.config.timestamp_format;
        let timestamp = event.timestamp.format(timestamp_format).to_string();
        
        let mut result = format!(
            "[{}] {} {}",
            timestamp,
            event.level,
            event.message
        );
        
        // 添加上下文信息
        // Note: include_context field removed from LogFormatConfig
        if true { // Always include context for now
            result.push_str(&format!(
                " [pid:{} tid:{}",
                event.context.pid,
                event.context.tid
            ));
            
            if let Some(ref username) = event.context.username {
                result.push_str(&format!(" user:{}", username));
            }
            
            if let Some(ref hostname) = event.context.hostname {
                result.push_str(&format!(" host:{}", hostname));
            }
            
            if let Some(mpi_rank) = event.context.mpi_rank {
                result.push_str(&format!(" mpi:{}", mpi_rank));
            }
            
            result.push(']');
        }
        
        // 添加位置信息（基于模板配置）
        if self.config.template.contains("{file}") || self.config.template.contains("{line}") {
            if let (Some(ref file), Some(line)) = (&event.file, event.line) {
                result.push_str(&format!(" at {}:{}", file, line));
            }
        }
        
        // 添加自定义字段
        if !event.fields.is_empty() {
            result.push_str(" {");
            for (key, value) in &event.fields {
                result.push_str(&format!(" {}={}", key, value));
            }
            result.push_str(" }");
        }
        
        result
    }

    /// 格式化为 JSON 格式
    fn format_as_json(&self, event: &QuantumLogEvent) -> String {
        event.to_json().unwrap_or_else(|_| "{\"error\":\"serialization_failed\"}".to_string())
    }

    /// 格式化为 CSV 格式
    fn format_as_csv(&self, event: &QuantumLogEvent) -> String {
        let row = event.to_csv_row();
        // 简单的 CSV 格式化，实际应用中可能需要更复杂的转义
        row.iter()
            .map(|field| {
                if field.contains(',') || field.contains('"') || field.contains('\n') {
                    format!("\"{}\"", field.replace('"', "\"\""))
                } else {
                    field.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl<S> Layer<S> for FormatterLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &Event<'_>,
        _ctx: Context<'_, S>,
    ) {
        // 在实际实现中，格式化层通常不直接处理事件
        // 而是被其他层（如 DispatchLayer）调用来格式化事件
        // 这里我们保持接口的完整性
        let _ = event; // 避免未使用警告
    }
}

// 为了实际使用，我们提供一个辅助方法
impl FormatterLayer {
    /// 异步格式化事件
    /// 
    /// 这个方法会被其他层调用来格式化事件
    pub async fn format_event_async(&self, event: &Event<'_>) -> String {
        let context_info = self.context_injector.create_context_for_event().await;
        let quantum_event = self.convert_event(event, context_info).await;
        self.format_event(&quantum_event)
    }
    
    /// 将事件转换为 QuantumLogEvent
    pub async fn to_quantum_event(&self, event: &Event<'_>) -> QuantumLogEvent {
        let context_info = self.context_injector.create_context_for_event().await;
        self.convert_event(event, context_info).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LogFormatType;
    use tracing::Level;

    fn create_test_config() -> LogFormatConfig {
        LogFormatConfig {
            format_type: LogFormatType::Text,
            timestamp_format: "%Y-%m-%d %H:%M:%S%.3f".to_string(),
            template: "{timestamp} [{level}] {target}: {message}".to_string(),
            csv_columns: None,
            json_flatten_fields: false,
            json_fields_key: "fields".to_string(),
        }
    }

    fn create_test_event() -> QuantumLogEvent {
        // 创建一个简单的 callsite identifier
        static METADATA: tracing::Metadata<'static> = tracing::Metadata::new(
            "test_event",
            "quantum_log::formatter::test",
            Level::INFO,
            Some(file!()),
            Some(line!()),
            Some(module_path!()),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );
        static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&METADATA);
        let fields = tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE));
        
        let metadata = tracing::Metadata::new(
            "test_event",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            fields,
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

        QuantumLogEvent::new(
            Level::INFO,
            "Test message".to_string(),
            &metadata,
            HashMap::new(),
            context,
        )
    }

    #[test]
    fn test_text_formatting() {
        let config = create_test_config();
        let context_injector = Arc::new(ContextInjectorLayer::new());
        let formatter = FormatterLayer::new(config, context_injector);
        let event = create_test_event();
        
        let formatted = formatter.format_event(&event);
        
        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("pid:1234"));
        assert!(formatted.contains("tid:5678"));
        assert!(formatted.contains("user:test_user"));
        assert!(formatted.contains("host:test_host"));
        assert!(formatted.contains("mpi:0"));
    }

    #[test]
    fn test_json_formatting() {
        let mut config = create_test_config();
        config.format_type = LogFormatType::Json;
        let context_injector = Arc::new(ContextInjectorLayer::new());
        let formatter = FormatterLayer::new(config, context_injector);
        let event = create_test_event();
        
        let formatted = formatter.format_event(&event);
        
        assert!(formatted.starts_with('{'));
        assert!(formatted.ends_with('}'));
        assert!(formatted.contains("\"level\":\"INFO\""));
        assert!(formatted.contains("\"message\":\"Test message\""));
    }

    #[test]
    fn test_csv_formatting() {
        let mut config = create_test_config();
        config.format_type = LogFormatType::Csv;
        let context_injector = Arc::new(ContextInjectorLayer::new());
        let formatter = FormatterLayer::new(config, context_injector);
        let event = create_test_event();
        
        let formatted = formatter.format_event(&event);
        
        let fields: Vec<&str> = formatted.split(',').collect();
        assert!(fields.len() >= 10); // 至少应该有基本字段
        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("Test message"));
    }
}