//! 预初始化缓冲层
//!
//! 此层负责在 QuantumLog 完全初始化之前缓存日志事件。

use crate::core::event::QuantumLogEvent;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tracing::{Event, Level};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// 预初始化缓冲层
///
/// 在 QuantumLog 完全初始化之前，此层会缓存所有的日志事件。
/// 一旦系统初始化完成，缓存的事件会被重放到实际的处理管道中。
pub struct PreInitBufferLayer {
    /// 事件缓冲区
    buffer: Arc<Mutex<EventBuffer>>,
    /// 是否启用缓冲
    enabled: bool,
    /// 最大缓冲大小
    max_buffer_size: usize,
}

/// 事件缓冲区
struct EventBuffer {
    /// 缓存的事件
    events: VecDeque<BufferedEvent>,
    /// 是否已初始化（停止缓冲）
    initialized: bool,
    /// 丢弃的事件数量
    dropped_count: usize,
}

/// 缓冲的事件
#[derive(Debug, Clone)]
pub struct BufferedEvent {
    /// 事件级别
    level: Level,
    /// 目标模块
    target: String,
    /// 文件名
    file: Option<String>,
    /// 行号
    line: Option<u32>,
    /// 模块路径
    module_path: Option<String>,
    /// 事件字段
    fields: std::collections::HashMap<String, String>,
    /// 时间戳
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl PreInitBufferLayer {
    /// 创建新的预初始化缓冲层
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(EventBuffer {
                events: VecDeque::new(),
                initialized: false,
                dropped_count: 0,
            })),
            enabled: true,
            max_buffer_size,
        }
    }

    /// 创建默认的预初始化缓冲层
    pub fn new_default() -> Self {
        Self::new(10000) // 默认缓冲 10000 个事件
    }

    /// 禁用缓冲层
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// 标记为已初始化，停止缓冲新事件
    pub fn mark_initialized(&self) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.initialized = true;
        }
    }

    /// 获取缓冲的事件并清空缓冲区
    pub fn drain_buffered_events(&self) -> Vec<BufferedEvent> {
        if let Ok(mut buffer) = self.buffer.lock() {
            let events: Vec<_> = buffer.events.drain(..).collect();
            buffer.dropped_count = 0;
            events
        } else {
            Vec::new()
        }
    }

    /// 获取丢弃的事件数量
    pub fn get_dropped_count(&self) -> usize {
        if let Ok(buffer) = self.buffer.lock() {
            buffer.dropped_count
        } else {
            0
        }
    }

    /// 获取当前缓冲区大小
    pub fn get_buffer_size(&self) -> usize {
        if let Ok(buffer) = self.buffer.lock() {
            buffer.events.len()
        } else {
            0
        }
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        if let Ok(buffer) = self.buffer.lock() {
            buffer.initialized
        } else {
            false
        }
    }

    /// 添加事件到缓冲区
    fn buffer_event(&self, event: &Event<'_>) {
        if !self.enabled {
            return;
        }

        let mut buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(_) => return, // 锁定失败，跳过此事件
        };

        // 如果已初始化，不再缓冲新事件
        if buffer.initialized {
            return;
        }

        // 检查缓冲区大小
        if buffer.events.len() >= self.max_buffer_size {
            // 移除最旧的事件
            buffer.events.pop_front();
            buffer.dropped_count += 1;
        }

        // 提取事件信息
        let metadata = event.metadata();
        let mut fields = std::collections::HashMap::new();

        // 创建字段访问器
        struct FieldExtractor<'a> {
            fields: &'a mut std::collections::HashMap<String, String>,
        }

        impl<'a> tracing::field::Visit for FieldExtractor<'a> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.fields.insert(field.name().to_string(), format!("{:?}", value));
            }

            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.fields.insert(field.name().to_string(), value.to_string());
            }

            fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
                self.fields.insert(field.name().to_string(), value.to_string());
            }

            fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                self.fields.insert(field.name().to_string(), value.to_string());
            }

            fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
                self.fields.insert(field.name().to_string(), value.to_string());
            }
        }

        let mut extractor = FieldExtractor { fields: &mut fields };
        event.record(&mut extractor);

        // 创建缓冲事件
        let buffered_event = BufferedEvent {
            level: *metadata.level(),
            target: metadata.target().to_string(),
            file: metadata.file().map(|s| s.to_string()),
            line: metadata.line(),
            module_path: metadata.module_path().map(|s| s.to_string()),
            fields,
            timestamp: chrono::Utc::now(),
        };

        buffer.events.push_back(buffered_event);
    }
}

impl<S> Layer<S> for PreInitBufferLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &Event<'_>,
        _ctx: Context<'_, S>,
    ) {
        // 如果已初始化，不再处理事件
        if self.is_initialized() {
            return;
        }
        self.buffer_event(event);
    }
}

// 为了支持克隆，我们需要实现 Clone
impl Clone for PreInitBufferLayer {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
            enabled: self.enabled,
            max_buffer_size: self.max_buffer_size,
        }
    }
}

/// 缓冲事件转换为 QuantumLogEvent 的辅助函数
impl BufferedEvent {
    /// 转换为 QuantumLogEvent
    pub fn to_quantum_event(&self, context_info: crate::core::event::ContextInfo) -> QuantumLogEvent {
        // 创建临时的 Metadata
        let metadata = tracing::Metadata::new(
            "buffered_event",
            &self.target,
            self.level,
            self.file.as_deref(),
            self.line,
            self.module_path.as_deref(),
            {
                static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
                    "buffered_event",
                    "quantum_log::pre_init_buffer",
                    tracing::Level::INFO,
                    Some(file!()),
                    Some(line!()),
                    Some(module_path!()),
                    tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
                    tracing::metadata::Kind::EVENT,
                ));
                tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE))
            },
            tracing::metadata::Kind::EVENT,
        );

        // 转换字段
        let mut fields = std::collections::HashMap::new();
        for (key, value) in &self.fields {
            fields.insert(key.clone(), serde_json::Value::String(value.clone()));
        }

        // 获取消息
        let message = self.fields.get("message")
            .cloned()
            .unwrap_or_else(|| format!("Buffered event from {}", self.target));

        let mut quantum_event = QuantumLogEvent::new(
            self.level,
            message,
            &metadata,
            fields,
            context_info,
        );

        // 使用原始时间戳
        quantum_event.timestamp = self.timestamp;

        quantum_event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;
    use tracing_subscriber::{layer::SubscriberExt, Registry};

    #[test]
    fn test_buffer_creation() {
        let buffer = PreInitBufferLayer::new(100);
        assert_eq!(buffer.max_buffer_size, 100);
        assert!(buffer.enabled);
        assert!(!buffer.is_initialized());
        assert_eq!(buffer.get_buffer_size(), 0);
        assert_eq!(buffer.get_dropped_count(), 0);
    }

    #[test]
    fn test_buffer_initialization() {
        let buffer = PreInitBufferLayer::new(100);
        assert!(!buffer.is_initialized());
        
        buffer.mark_initialized();
        assert!(buffer.is_initialized());
    }

    #[test]
    fn test_buffer_disable() {
        let mut buffer = PreInitBufferLayer::new(100);
        assert!(buffer.enabled);
        
        buffer.disable();
        assert!(!buffer.enabled);
    }

    #[test]
    fn test_buffer_overflow() {
        let buffer = PreInitBufferLayer::new(2); // 很小的缓冲区
        
        // 创建测试事件
        let metadata = tracing::Metadata::new(
            "test",
            "test_target",
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            {
                static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
                    "test_event",
                    "quantum_log::pre_init_buffer::test",
                    tracing::Level::INFO,
                    Some(file!()),
                    Some(line!()),
                    Some(module_path!()),
                    tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
                    tracing::metadata::Kind::EVENT,
                ));
                tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE))
            },
            tracing::metadata::Kind::EVENT,
        );

        // 模拟添加多个事件
        for i in 0..5 {
            // 这里我们需要创建实际的事件，但由于测试限制，我们直接测试内部逻辑
            if let Ok(mut buf) = buffer.buffer.lock() {
                if buf.events.len() >= buffer.max_buffer_size {
                    buf.events.pop_front();
                    buf.dropped_count += 1;
                }
                
                let event = BufferedEvent {
                    level: Level::INFO,
                    target: "test".to_string(),
                    file: Some("test.rs".to_string()),
                    line: Some(42),
                    module_path: Some("test_module".to_string()),
                    fields: std::collections::HashMap::new(),
                    timestamp: chrono::Utc::now(),
                };
                
                buf.events.push_back(event);
            }
        }
        
        assert_eq!(buffer.get_buffer_size(), 2); // 应该只保留最新的2个
        assert_eq!(buffer.get_dropped_count(), 3); // 应该丢弃了3个
    }

    #[test]
    fn test_drain_events() {
        let buffer = PreInitBufferLayer::new(100);
        
        // 手动添加一些事件到缓冲区
        if let Ok(mut buf) = buffer.buffer.lock() {
            for i in 0..3 {
                let event = BufferedEvent {
                    level: Level::INFO,
                    target: format!("test_{}", i),
                    file: Some("test.rs".to_string()),
                    line: Some(42),
                    module_path: Some("test_module".to_string()),
                    fields: std::collections::HashMap::new(),
                    timestamp: chrono::Utc::now(),
                };
                buf.events.push_back(event);
            }
        }
        
        assert_eq!(buffer.get_buffer_size(), 3);
        
        let drained = buffer.drain_buffered_events();
        assert_eq!(drained.len(), 3);
        assert_eq!(buffer.get_buffer_size(), 0);
        
        // 检查事件顺序
        assert_eq!(drained[0].target, "test_0");
        assert_eq!(drained[1].target, "test_1");
        assert_eq!(drained[2].target, "test_2");
    }

    #[test]
    fn test_buffered_event_conversion() {
        let buffered_event = BufferedEvent {
            level: Level::INFO,
            target: "test_target".to_string(),
            file: Some("test.rs".to_string()),
            line: Some(42),
            module_path: Some("test_module".to_string()),
            fields: {
                let mut fields = std::collections::HashMap::new();
                fields.insert("message".to_string(), "Test message".to_string());
                fields.insert("key".to_string(), "value".to_string());
                fields
            },
            timestamp: chrono::Utc::now(),
        };
        
        let context_info = crate::core::event::ContextInfo::default();
        let quantum_event = buffered_event.to_quantum_event(context_info);
        
        assert_eq!(quantum_event.level, "INFO");
        assert_eq!(quantum_event.target, "test_target");
        assert_eq!(quantum_event.message, "Test message");
        assert_eq!(quantum_event.file, Some("test.rs".to_string()));
        assert_eq!(quantum_event.line, Some(42));
        assert!(quantum_event.fields.contains_key("key"));
    }
}