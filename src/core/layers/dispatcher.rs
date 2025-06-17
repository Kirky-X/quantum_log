//! 分发层
//!
//! 此层负责将格式化的日志事件分发到各个配置的 Sink。

use crate::config::{BackpressureStrategy, QuantumLoggerConfig};
use crate::core::layers::formatter::FormatterLayer;
use crate::error::Result;
#[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
use crate::sinks::database::DatabaseSink;
use crate::sinks::stdout::StdoutSink;
use crate::sinks::traits::QuantumSink;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::Event;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Sink 类型枚举
#[derive(Debug, Clone)]
pub enum SinkType {
    Stdout(StdoutSink),
    #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
    Database(DatabaseSink),
    // 未来会添加更多类型
    // File(FileSinkProcessor),
}

/// 分发层
///
/// 此层负责将事件分发到所有配置的 Sink。
pub struct DispatcherLayer {
    /// 配置
    config: QuantumLoggerConfig,
    /// 格式化层
    formatter: Arc<FormatterLayer>,
    /// Sink 处理器列表
    sinks: Arc<RwLock<Vec<SinkProcessor>>>,
}

/// Sink 处理器包装器
struct SinkProcessor {
    /// Sink 类型
    sink_type: String,
    /// 是否启用
    enabled: bool,
    /// 背压策略
    backpressure_strategy: BackpressureStrategy,
    /// 实际的处理器
    processor: SinkProcessorInner,
}

/// Sink 处理器内部类型
enum SinkProcessorInner {
    Stdout(Arc<StdoutSink>),
    #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
    Database(Arc<DatabaseSink>),
    // 未来会添加更多类型
}

impl DispatcherLayer {
    /// 创建新的分发层
    pub fn new(config: QuantumLoggerConfig, formatter: Arc<FormatterLayer>) -> Result<Self> {
        let dispatcher = Self {
            config,
            formatter,
            sinks: Arc::new(RwLock::new(Vec::new())),
        };

        Ok(dispatcher)
    }

    /// 初始化所有 Sink
    pub async fn initialize_sinks(&self) -> Result<()> {
        let mut sinks = self.sinks.write().await;

        // 初始化标准输出 Sink
        if let Some(ref stdout_config) = self.config.stdout {
            if stdout_config.enabled {
                let processor = Arc::new(StdoutSink::new(stdout_config.clone()));
                sinks.push(SinkProcessor {
                    sink_type: "stdout".to_string(),
                    enabled: true,
                    backpressure_strategy: self.config.backpressure_strategy.clone(),
                    processor: SinkProcessorInner::Stdout(processor),
                });
            }
        }

        // 初始化数据库 Sink
        #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
        if let Some(ref database_config) = self.config.database {
            if database_config.enabled {
                match DatabaseSink::new(database_config.clone()).await {
                    Ok(processor) => {
                        sinks.push(SinkProcessor {
                            sink_type: "database".to_string(),
                            enabled: true,
                            backpressure_strategy: self.config.backpressure_strategy.clone(),
                            processor: SinkProcessorInner::Database(Arc::new(processor)),
                        });
                    }
                    Err(e) => {
                        eprintln!("Failed to initialize database sink: {}", e);
                    }
                }
            }
        }

        // 未来会在这里初始化其他 Sink 类型
        // if self.config.file_sink.enabled { ... }

        Ok(())
    }

    /// 停机所有 Sink
    pub async fn shutdown(&self) -> Result<()> {
        let mut sinks = self.sinks.write().await;

        // 收集所有需要停机的处理器
        let mut shutdown_tasks = Vec::new();

        // 由于我们需要移动所有权，我们需要重新构建 sinks 向量
        let mut remaining_sinks = Vec::new();

        for sink in sinks.drain(..) {
            match sink.processor {
                SinkProcessorInner::Stdout(processor) => {
                    // 尝试从 Arc 中提取处理器
                    match Arc::try_unwrap(processor) {
                        Ok(processor) => {
                            let task = tokio::spawn(async move { processor.shutdown().await });
                            shutdown_tasks.push(task);
                        }
                        Err(arc_processor) => {
                            // 如果还有其他引用，我们无法停机，保留它
                            remaining_sinks.push(SinkProcessor {
                                sink_type: sink.sink_type,
                                enabled: sink.enabled,
                                backpressure_strategy: sink.backpressure_strategy,
                                processor: SinkProcessorInner::Stdout(arc_processor),
                            });
                        }
                    }
                }
                #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
                SinkProcessorInner::Database(processor) => {
                    // 尝试从 Arc 中提取处理器
                    match Arc::try_unwrap(processor) {
                        Ok(processor) => {
                            let task = tokio::spawn(async move { processor.shutdown().await });
                            shutdown_tasks.push(task);
                        }
                        Err(arc_processor) => {
                            // 如果还有其他引用，我们无法停机，保留它
                            remaining_sinks.push(SinkProcessor {
                                sink_type: sink.sink_type,
                                enabled: sink.enabled,
                                backpressure_strategy: sink.backpressure_strategy,
                                processor: SinkProcessorInner::Database(arc_processor),
                            });
                        }
                    }
                } // 未来会添加其他 Sink 类型的停机处理
            }
        }

        // 恢复无法停机的 Sink
        *sinks = remaining_sinks;

        // 等待所有停机任务完成
        for task in shutdown_tasks {
            if let Err(e) = task.await {
                eprintln!("Error during sink shutdown: {}", e);
            }
        }

        Ok(())
    }
}

impl<S> Layer<S> for DispatcherLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // 异步分发事件
        let dispatcher = self.clone();
        let event_metadata = event.metadata();
        let event_fields = extract_event_fields(event);

        tokio::spawn(async move {
            // 重新构建事件信息进行分发
            // 注意：这里我们需要重新构建事件，因为 Event 不能跨线程传递
            if let Err(e) = dispatcher
                .dispatch_reconstructed_event(event_metadata, &event_fields)
                .await
            {
                eprintln!("Failed to dispatch event: {}", e);
            }
        });
    }
}

// 为了支持克隆，我们需要实现 Clone
impl Clone for DispatcherLayer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            formatter: Arc::clone(&self.formatter),
            sinks: Arc::clone(&self.sinks),
        }
    }
}

/// 提取事件字段
fn extract_event_fields(event: &Event<'_>) -> std::collections::HashMap<String, String> {
    let mut fields = std::collections::HashMap::new();

    struct FieldExtractor<'a> {
        fields: &'a mut std::collections::HashMap<String, String>,
    }

    impl<'a> tracing::field::Visit for FieldExtractor<'a> {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            self.fields
                .insert(field.name().to_string(), format!("{:?}", value));
        }

        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            self.fields
                .insert(field.name().to_string(), value.to_string());
        }
    }

    let mut extractor = FieldExtractor {
        fields: &mut fields,
    };
    event.record(&mut extractor);

    fields
}

impl DispatcherLayer {
    /// 直接分发 QuantumLogEvent
    pub async fn dispatch_quantum_event(
        &self,
        quantum_event: &crate::core::event::QuantumLogEvent,
    ) -> Result<()> {
        // 分发到所有 Sink
        let sinks = self.sinks.read().await;

        for sink in sinks.iter() {
            if !sink.enabled {
                continue;
            }

            let quantum_event_clone = quantum_event.clone();
            let strategy = sink.backpressure_strategy.clone();

            match &sink.processor {
                SinkProcessorInner::Stdout(processor) => {
                    if let Err(e) = QuantumSink::send_event(processor.as_ref(), quantum_event_clone).await {
                        eprintln!("Failed to send event to stdout sink: {}", e);
                    }
                }
                #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
                SinkProcessorInner::Database(processor) => {
                    if let Err(e) = QuantumSink::send_event(processor.as_ref(), quantum_event_clone).await {
                        eprintln!("Failed to send event to database sink: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// 分发重构的事件
    pub async fn dispatch_reconstructed_event(
        &self,
        metadata: &tracing::Metadata<'_>,
        fields: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        // 获取上下文信息
        let context_info = {
            // 这里我们需要访问 formatter 中的 context_injector
            // 由于架构限制，我们直接创建一个新的上下文
            use crate::core::layers::context_injector::ContextInjectorLayer;
            let injector = ContextInjectorLayer::new();
            injector.create_context_for_event().await
        };

        // 构建 QuantumLogEvent
        let message = fields
            .get("message")
            .cloned()
            .unwrap_or_else(|| format!("Event from {}", metadata.target()));

        let mut event_fields = std::collections::HashMap::new();
        for (key, value) in fields {
            if key != "message" {
                event_fields.insert(key.clone(), serde_json::Value::String(value.clone()));
            }
        }

        let quantum_event = crate::core::event::QuantumLogEvent::new(
            *metadata.level(),
            message,
            metadata,
            event_fields,
            context_info,
        );

        // 分发到所有 Sink
        let sinks = self.sinks.read().await;

        for sink in sinks.iter() {
            if !sink.enabled {
                continue;
            }

            let quantum_event_clone = quantum_event.clone();
            let _ = sink.backpressure_strategy.clone();

            match &sink.processor {
                SinkProcessorInner::Stdout(processor) => {
                    if let Err(e) = QuantumSink::send_event(processor.as_ref(), quantum_event_clone).await {
                        eprintln!("Failed to send event to stdout sink: {}", e);
                    }
                }
                #[cfg(any(feature = "sqlite", feature = "postgres", feature = "mysql"))]
                SinkProcessorInner::Database(processor) => {
                    if let Err(e) = QuantumSink::send_event(processor.as_ref(), quantum_event_clone).await {
                        eprintln!("Failed to send event to database sink: {}", e);
                    }
                } // 未来会添加其他 Sink 类型的处理
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StdoutConfig;
    use crate::core::layers::context_injector::ContextInjectorLayer;
    use std::sync::Arc;

    fn create_test_config() -> QuantumLoggerConfig {
        QuantumLoggerConfig {
            global_level: "INFO".to_string(),
            pre_init_buffer_size: Some(1000),
            pre_init_stdout_enabled: true,
            backpressure_strategy: BackpressureStrategy::Drop,
            stdout: Some(StdoutConfig {
                enabled: true,
                level: Some("INFO".to_string()),
                color_enabled: Some(false),
                level_colors: None,
                format: crate::config::OutputFormat::Text,
                colored: false,
            }),
            file: None,
            database: None,
            context_fields: Default::default(),
            format: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_dispatcher_creation() {
        let config = create_test_config();
        let context_injector = Arc::new(ContextInjectorLayer::new());
        let formatter = Arc::new(FormatterLayer::new(config.format.clone(), context_injector));

        let dispatcher = DispatcherLayer::new(config, formatter);
        assert!(dispatcher.is_ok());
    }

    #[tokio::test]
    async fn test_sink_initialization() {
        let config = create_test_config();
        let context_injector = Arc::new(ContextInjectorLayer::new());
        let formatter = Arc::new(FormatterLayer::new(config.format.clone(), context_injector));
        let dispatcher = DispatcherLayer::new(config, formatter).unwrap();

        let result = dispatcher.initialize_sinks().await;
        assert!(result.is_ok());

        // 检查是否创建了 stdout sink
        let sinks = dispatcher.sinks.read().await;
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].sink_type, "stdout");
        assert!(sinks[0].enabled);
    }

    #[tokio::test]
    async fn test_dispatcher_shutdown() {
        let config = create_test_config();
        let context_injector = Arc::new(ContextInjectorLayer::new());
        let formatter = Arc::new(FormatterLayer::new(config.format.clone(), context_injector));
        let dispatcher = DispatcherLayer::new(config, formatter).unwrap();

        dispatcher.initialize_sinks().await.unwrap();

        let result = dispatcher.shutdown().await;
        assert!(result.is_ok());
    }
}
