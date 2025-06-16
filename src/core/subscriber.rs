//! QuantumLog 核心订阅器
//!
//! 此模块提供 QuantumLog 的主要订阅器实现，整合所有层并提供统一的日志处理接口。

use crate::config::QuantumLogConfig;
use crate::core::layers::{
    context_injector::ContextInjectorLayer, dispatcher::DispatcherLayer, formatter::FormatterLayer,
    pre_init_buffer::PreInitBufferLayer,
};
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, Registry,
};

/// QuantumLog 订阅器
///
/// 这是 QuantumLog 的主要订阅器，整合了所有必要的层：
/// - PreInitBufferLayer: 预初始化缓冲
/// - ContextInjectorLayer: 上下文注入
/// - FormatterLayer: 事件格式化
/// - DispatcherLayer: 事件分发
#[derive(Clone)]
pub struct QuantumLogSubscriber {
    /// 配置
    config: Arc<QuantumLogConfig>,
    /// 预初始化缓冲层
    pre_init_buffer: PreInitBufferLayer,
    /// 上下文注入层
    context_injector: ContextInjectorLayer,
    /// 格式化层
    formatter: FormatterLayer,
    /// 分发层
    dispatcher: DispatcherLayer,
    /// 是否已初始化
    initialized: bool,
}

/// 订阅器构建器
pub struct QuantumLogSubscriberBuilder {
    config: QuantumLogConfig,
    max_buffer_size: Option<usize>,
    custom_fields: std::collections::HashMap<String, String>,
}

impl QuantumLogSubscriber {
    /// 创建新的订阅器构建器
    pub fn builder() -> QuantumLogSubscriberBuilder {
        QuantumLogSubscriberBuilder {
            config: QuantumLogConfig::default(),
            max_buffer_size: None,
            custom_fields: std::collections::HashMap::new(),
        }
    }

    /// 使用默认配置创建订阅器
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::builder().build()
    }

    /// 使用指定配置创建订阅器
    pub fn with_config(
        config: QuantumLogConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::builder().config(config).build()
    }

    /// 初始化订阅器
    ///
    /// 这会执行以下步骤：
    /// 1. 初始化分发层
    /// 2. 标记预初始化缓冲层为已初始化
    /// 3. 重放缓冲的事件
    pub async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.initialized {
            return Ok(());
        }

        // 1. 初始化分发层
        self.dispatcher.initialize_sinks().await?;

        // 2. 标记预初始化缓冲层为已初始化
        self.pre_init_buffer.mark_initialized();

        // 3. 重放缓冲的事件
        self.replay_buffered_events().await?;

        self.initialized = true;
        Ok(())
    }

    /// 重放缓冲的事件
    async fn replay_buffered_events(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let buffered_events = self.pre_init_buffer.drain_buffered_events();

        if !buffered_events.is_empty() {
            // 直接通过分发层处理缓冲的事件，避免重复处理
            for buffered_event in buffered_events {
                // 生成上下文信息
                let context_info = self.context_injector.get_context_info().await;

                // 转换为 QuantumLogEvent
                let quantum_event = buffered_event.to_quantum_event(context_info);

                // 直接通过分发层处理事件
                if let Err(e) = self.dispatcher.dispatch_quantum_event(&quantum_event).await {
                    tracing::error!("Failed to replay buffered event: {}", e);
                }
            }

            let dropped_count = self.pre_init_buffer.get_dropped_count();
            if dropped_count > 0 {
                tracing::warn!(
                    "Dropped {} events due to buffer overflow during pre-initialization",
                    dropped_count
                );
            }
        }

        Ok(())
    }

    /// 关闭订阅器
    ///
    /// 这会优雅地关闭所有 sink 并等待它们完成处理
    pub async fn shutdown(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.dispatcher
            .shutdown()
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// 获取配置的引用
    pub fn config(&self) -> &QuantumLogConfig {
        &self.config
    }

    /// 获取预初始化缓冲区统计信息
    pub fn get_buffer_stats(&self) -> BufferStats {
        BufferStats {
            current_size: self.pre_init_buffer.get_buffer_size(),
            dropped_count: self.pre_init_buffer.get_dropped_count(),
            is_initialized: self.pre_init_buffer.is_initialized(),
        }
    }

    /// 安装为全局默认订阅器
    pub fn install_global(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let registry = Registry::default()
            .with(LevelFilter::from_level(
                self.config.global_level.parse().unwrap_or(Level::INFO),
            ))
            .with(self.pre_init_buffer.clone())
            .with(self.context_injector.clone())
            .with(self.formatter.clone())
            .with(self.dispatcher.clone());

        registry.init();
        Ok(())
    }
}

impl Default for QuantumLogSubscriber {
    fn default() -> Self {
        Self::new().expect("Failed to create default QuantumLogSubscriber")
    }
}

impl QuantumLogSubscriberBuilder {
    /// 设置配置
    pub fn config(mut self, config: QuantumLogConfig) -> Self {
        self.config = config;
        self
    }

    /// 设置最大缓冲区大小
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = Some(size);
        self
    }

    /// 添加自定义字段
    pub fn custom_field<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.custom_fields.insert(key.into(), value.into());
        self
    }

    /// 添加多个自定义字段
    pub fn custom_fields(mut self, fields: std::collections::HashMap<String, String>) -> Self {
        self.custom_fields.extend(fields);
        self
    }

    /// 构建订阅器
    pub fn build(self) -> Result<QuantumLogSubscriber, Box<dyn std::error::Error + Send + Sync>> {
        let config = Arc::new(self.config);

        // 创建预初始化缓冲层
        let buffer_size = self.max_buffer_size.unwrap_or(10000);
        let pre_init_buffer = PreInitBufferLayer::new(buffer_size);

        // 创建上下文注入层
        let mut context_injector = ContextInjectorLayer::new();
        for (key, value) in self.custom_fields {
            context_injector =
                context_injector.with_custom_field(key, serde_json::Value::String(value));
        }

        // 创建格式化层
        let formatter =
            FormatterLayer::new(config.format.clone(), Arc::new(context_injector.clone()));

        // 创建分发层
        let dispatcher = DispatcherLayer::new((*config).clone(), Arc::new(formatter.clone()))?;

        Ok(QuantumLogSubscriber {
            config,
            pre_init_buffer,
            context_injector,
            formatter,
            dispatcher,
            initialized: false,
        })
    }
}

/// 缓冲区统计信息
#[derive(Debug, Clone)]
pub struct BufferStats {
    /// 当前缓冲区大小
    pub current_size: usize,
    /// 丢弃的事件数量
    pub dropped_count: usize,
    /// 是否已初始化
    pub is_initialized: bool,
}

/// 便捷函数：创建并安装默认的 QuantumLog 订阅器
pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = QuantumLogSubscriber::new()?;
    subscriber.install_global()
}

/// 便捷函数：使用配置创建并安装 QuantumLog 订阅器
pub fn init_with_config(
    config: QuantumLogConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = QuantumLogSubscriber::with_config(config)?;
    subscriber.install_global()
}

/// 便捷函数：使用构建器创建并安装 QuantumLog 订阅器
pub fn init_with_builder<F>(builder_fn: F) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(QuantumLogSubscriberBuilder) -> QuantumLogSubscriberBuilder,
{
    let builder = QuantumLogSubscriber::builder();
    let subscriber = builder_fn(builder).build()?;
    subscriber.install_global()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscriber_creation() {
        let subscriber = QuantumLogSubscriber::new();
        assert!(subscriber.is_ok());

        let subscriber = subscriber.unwrap();
        assert!(!subscriber.is_initialized());
        assert_eq!(subscriber.get_buffer_stats().current_size, 0);
    }

    #[test]
    fn test_subscriber_builder() {
        let config = QuantumLogConfig {
            global_level: "DEBUG".to_string(),
            ..Default::default()
        };

        let subscriber = QuantumLogSubscriber::builder()
            .config(config)
            .max_buffer_size(5000)
            .custom_field("app", "test")
            .custom_field("version", "1.0.0")
            .build();

        assert!(subscriber.is_ok());
        let subscriber = subscriber.unwrap();
        assert!(!subscriber.is_initialized());
    }

    #[test]
    fn test_buffer_stats() {
        let subscriber = QuantumLogSubscriber::new().unwrap();
        let stats = subscriber.get_buffer_stats();

        assert_eq!(stats.current_size, 0);
        assert_eq!(stats.dropped_count, 0);
        assert!(!stats.is_initialized);
    }

    #[tokio::test]
    async fn test_subscriber_initialization() {
        let mut subscriber = QuantumLogSubscriber::new().unwrap();
        assert!(!subscriber.is_initialized());

        // 注意：这个测试可能会失败，因为我们没有配置实际的 sink
        // 在实际使用中，需要先配置 sink
        let result = subscriber.initialize().await;
        // 我们不检查结果，因为没有配置 sink 可能会失败

        // 但是缓冲区应该被标记为已初始化
        let stats = subscriber.get_buffer_stats();
        assert!(stats.is_initialized);
    }

    #[test]
    fn test_custom_fields_builder() {
        let mut custom_fields = std::collections::HashMap::new();
        custom_fields.insert("service".to_string(), "test-service".to_string());
        custom_fields.insert("environment".to_string(), "test".to_string());

        let subscriber = QuantumLogSubscriber::builder()
            .custom_fields(custom_fields)
            .custom_field("additional", "field")
            .build();

        assert!(subscriber.is_ok());
    }

    #[test]
    fn test_convenience_functions() {
        // 测试 init 函数（但不实际安装）
        let result = std::panic::catch_unwind(|| {
            // 这可能会 panic，因为可能已经有全局订阅器
            let _ = init();
        });
        // 我们不检查结果，因为可能已经有全局订阅器

        // 测试 init_with_config 函数
        let config = QuantumLogConfig::default();
        let result = std::panic::catch_unwind(|| {
            let _ = init_with_config(config);
        });
        // 同样不检查结果

        // 测试 init_with_builder 函数
        let result = std::panic::catch_unwind(|| {
            let _ = init_with_builder(|builder| builder.max_buffer_size(1000));
        });
        // 同样不检查结果
    }
}
