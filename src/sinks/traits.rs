//! QuantumLog Sink Traits
//!
//! 定义了统一的 Sink trait 接口，允许开发者实现自定义的日志输出目标。
//!
//! # 架构设计
//!
//! - `QuantumSink`: 基础 trait，定义所有 sink 的核心接口
//! - `ExclusiveSink`: 独占型 sink，不能与其他 sink 叠加使用
//! - `StackableSink`: 可叠加型 sink，可以与其他 sink 同时使用
//!
//! # 使用示例
//!
//! ```rust
//! use quantum_log::sinks::traits::{QuantumSink, ExclusiveSink, SinkError, SinkMetadata, SinkType};
//! use quantum_log::core::event::QuantumLogEvent;
//! use async_trait::async_trait;
//!
//! #[derive(Debug)]
//! struct MyCustomSink {
//!     name: String,
//! }
//!
//! #[async_trait]
//! impl QuantumSink for MyCustomSink {
//!     type Config = ();
//!     type Error = SinkError;
//!
//!     async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
//!         // 实现日志事件处理逻辑
//!         println!("Custom sink received event: {:?}", event);
//!         Ok(())
//!     }
//!
//!     async fn shutdown(&self) -> Result<(), Self::Error> {
//!         // 实现优雅关闭逻辑
//!         Ok(())
//!     }
//!
//!     async fn is_healthy(&self) -> bool {
//!         true
//!     }
//!
//!     fn name(&self) -> &'static str {
//!         "my_custom_sink"
//!     }
//!
//!     fn stats(&self) -> String {
//!         "MyCustomSink: active".to_string()
//!     }
//!
//!     fn metadata(&self) -> SinkMetadata {
//!         SinkMetadata {
//!             name: self.name.clone(),
//!             sink_type: SinkType::Exclusive,
//!             enabled: true,
//!             description: Some("Custom sink example".to_string()),
//!         }
//!     }
//! }
//!
//! impl ExclusiveSink for MyCustomSink {}
//! ```

use crate::core::event::QuantumLogEvent;
use async_trait::async_trait;
use std::error::Error as StdError;
use std::fmt::Debug;
use std::result::Result;

/// 基础 Sink trait
///
/// 定义了所有日志输出目标必须实现的核心接口。
/// 所有自定义 sink 都必须实现此 trait。
#[async_trait]
pub trait QuantumSink: Send + Sync + Debug {
    /// 配置类型，用于初始化 sink
    type Config: Send + Sync + Debug + Clone;

    /// 错误类型
    type Error: StdError + Send + Sync + 'static;

    /// 发送日志事件到输出目标
    ///
    /// # 参数
    ///
    /// * `event` - 要发送的日志事件
    ///
    /// # 返回值
    ///
    /// 成功时返回 `Ok(())`，失败时返回具体的错误信息
    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error>;

    /// 优雅关闭 sink
    ///
    /// 此方法应该确保所有待处理的日志事件都被正确处理，
    /// 并释放相关资源。
    async fn shutdown(&self) -> Result<(), Self::Error>;

    /// 检查 sink 是否健康
    ///
    /// 默认实现总是返回 true，子类可以重写此方法
    /// 来提供更精确的健康检查。
    async fn is_healthy(&self) -> bool {
        true
    }

    /// 获取 sink 的名称
    ///
    /// 用于日志和调试目的
    fn name(&self) -> &'static str;

    /// 获取 sink 的统计信息
    ///
    /// 返回一个包含统计信息的字符串，用于监控和调试
    fn stats(&self) -> String {
        format!("Sink: {}, Status: Healthy", self.name())
    }

    /// 获取 sink 的元数据
    ///
    /// 返回包含 sink 基本信息的元数据结构
    fn metadata(&self) -> SinkMetadata;
}

/// 独占型 Sink trait
///
/// 标记 trait，表示此 sink 不能与其他 sink 叠加使用。
/// 当管道中包含独占型 sink 时，不能再添加其他 sink。
///
/// 典型的独占型 sink 包括：
/// - 文件 sink
/// - 数据库 sink  
/// - 网络 sink
/// - 滚动文件 sink
pub trait ExclusiveSink: QuantumSink {}

/// 可叠加型 Sink trait
///
/// 标记 trait，表示此 sink 可以与其他 sink 同时使用。
/// 可叠加型 sink 可以添加到已有管道中，与其他 sink 并行工作。
///
/// 典型的可叠加型 sink 包括：
/// - 标准输出 sink
/// - 控制台 sink
#[async_trait]
pub trait StackableSink: QuantumSink {
    /// 内部事件发送方法，支持背压策略控制
    ///
    /// 此方法允许 dispatcher 传递特定的背压策略来控制
    /// 当输出目标繁忙时的行为。
    ///
    /// # 参数
    ///
    /// * `event` - 要发送的日志事件
    /// * `strategy` - 背压处理策略
    ///
    /// # 返回值
    ///
    /// 成功时返回 `Ok(())`，背压时返回 `SinkError::Backpressure`
    async fn send_event_internal(
        &self,
        event: &QuantumLogEvent,
        strategy: crate::config::BackpressureStrategy,
    ) -> SinkResult<()> {
        // 默认实现：忽略背压策略，直接调用公开的 send_event 接口
        // 为了兼容早期将 StackableSink 作为标记 trait 的示例代码，这里提供了默认实现
        // 如果具体 sink 需要支持背压策略，请在具体类型上覆盖该方法
        let _ = strategy; // 避免未使用警告
        self.send_event(event.clone())
            .await
            .map_err(|e| SinkError::Generic(e.to_string()))
    }
}

/// Sink 工厂 trait
///
/// 用于创建 sink 实例的工厂接口
#[async_trait]
pub trait SinkFactory<T: QuantumSink> {
    /// 从配置创建 sink 实例
    async fn create_sink(config: T::Config) -> Result<T, T::Error>;
}

/// Sink 类型枚举
///
/// 用于在运行时区分不同类型的 sink
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkType {
    /// 独占型 sink
    Exclusive,
    /// 可叠加型 sink
    Stackable,
}

/// Sink 元数据
///
/// 包含 sink 的基本信息和类型
#[derive(Debug, Clone)]
pub struct SinkMetadata {
    /// Sink 名称
    pub name: String,
    /// Sink 类型
    pub sink_type: SinkType,
    /// 是否启用
    pub enabled: bool,
    /// 描述信息
    pub description: Option<String>,
}

impl SinkMetadata {
    /// 创建新的 sink 元数据
    pub fn new(name: String, sink_type: SinkType) -> Self {
        Self {
            name,
            sink_type,
            enabled: true,
            description: None,
        }
    }

    /// 设置描述信息
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 设置启用状态
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// 通用 Sink 错误类型
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// I/O 错误
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// 网络错误
    #[error("Network error: {0}")]
    Network(String),

    /// 数据库错误
    #[cfg(feature = "database")]
    #[error("Database error: {0}")]
    Database(String),

    /// 通用错误
    #[error("Generic error: {0}")]
    Generic(String),

    /// Sink 已关闭
    #[error("Sink is closed")]
    Closed,

    /// 背压错误
    #[error("Backpressure limit exceeded")]
    Backpressure,
}

/// Sink 结果类型
pub type SinkResult<T> = Result<T, SinkError>;

/// 对象安全的统一 Sink 动态接口，用于在运行时以 trait 对象管理不同 Config 的 sinks
#[async_trait]
pub trait QuantumSinkDyn: Send + Sync + Debug {
    async fn send_event_dyn(&self, event: QuantumLogEvent) -> SinkResult<()>;
    async fn shutdown_dyn(&self) -> SinkResult<()>;
    async fn is_healthy_dyn(&self) -> bool;
    fn name_dyn(&self) -> &'static str;
    fn stats_dyn(&self) -> String;
    fn metadata_dyn(&self) -> SinkMetadata;
}

#[async_trait]
impl<T> QuantumSinkDyn for T
where
    T: QuantumSink<Error = SinkError> + Send + Sync + Debug,
{
    async fn send_event_dyn(&self, event: QuantumLogEvent) -> SinkResult<()> {
        self.send_event(event).await
    }

    async fn shutdown_dyn(&self) -> SinkResult<()> {
        self.shutdown().await
    }

    async fn is_healthy_dyn(&self) -> bool {
        self.is_healthy().await
    }

    fn name_dyn(&self) -> &'static str {
        self.name()
    }

    fn stats_dyn(&self) -> String {
        self.stats()
    }

    fn metadata_dyn(&self) -> SinkMetadata {
        self.metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    // 测试用的 Mock Sink
    #[derive(Debug)]
    struct MockSink {
        name: &'static str,
        event_count: Arc<AtomicU64>,
        should_fail: bool,
    }

    impl MockSink {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                event_count: Arc::new(AtomicU64::new(0)),
                should_fail: false,
            }
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }

        fn event_count(&self) -> u64 {
            self.event_count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl QuantumSink for MockSink {
        type Config = ();
        type Error = SinkError;

        async fn send_event(&self, _event: QuantumLogEvent) -> Result<(), Self::Error> {
            if self.should_fail {
                return Err(SinkError::Generic("Mock failure".to_string()));
            }
            self.event_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn is_healthy(&self) -> bool {
            !self.should_fail
        }

        fn name(&self) -> &'static str {
            self.name
        }

        fn stats(&self) -> String {
            format!("MockSink[{}]: {} events", self.name, self.event_count())
        }

        fn metadata(&self) -> SinkMetadata {
            SinkMetadata {
                name: self.name.to_string(),
                sink_type: SinkType::Stackable,
                enabled: true,
                description: Some("Mock sink for testing".to_string()),
            }
        }
    }

    #[async_trait]
    impl StackableSink for MockSink {
        async fn send_event_internal(
            &self,
            event: &QuantumLogEvent,
            _strategy: crate::config::BackpressureStrategy,
        ) -> SinkResult<()> {
            // For testing, just delegate to the regular send_event
            self.send_event(event.clone()).await
        }
    }

    #[derive(Debug)]
    struct MockExclusiveSink {
        inner: MockSink,
    }

    impl MockExclusiveSink {
        fn new(name: &'static str) -> Self {
            Self {
                inner: MockSink::new(name),
            }
        }
    }

    #[async_trait]
    impl QuantumSink for MockExclusiveSink {
        type Config = ();
        type Error = SinkError;

        async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
            self.inner.send_event(event).await
        }

        async fn shutdown(&self) -> Result<(), Self::Error> {
            self.inner.shutdown().await
        }

        async fn is_healthy(&self) -> bool {
            self.inner.is_healthy().await
        }

        fn name(&self) -> &'static str {
            self.inner.name()
        }

        fn stats(&self) -> String {
            self.inner.stats()
        }

        fn metadata(&self) -> SinkMetadata {
            self.inner.metadata()
        }
    }

    impl ExclusiveSink for MockExclusiveSink {}

    fn create_test_event() -> QuantumLogEvent {
        use crate::core::event::ContextInfo;
        use std::collections::HashMap;

        QuantumLogEvent {
            timestamp: chrono::Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "Test message".to_string(),
            module_path: Some("test::module".to_string()),
            file: Some("test.rs".to_string()),
            line: Some(42),
            thread_name: Some("test-thread".to_string()),
            thread_id: "test-thread-id".to_string(),
            fields: HashMap::new(),
            context: ContextInfo {
                pid: std::process::id(),
                tid: 12345,
                username: None,
                hostname: None,
                mpi_rank: None,
                custom_fields: HashMap::new(),
            },
        }
    }

    #[tokio::test]
    async fn test_stackable_sink_basic_functionality() {
        let sink = MockSink::new("test_stackable");
        let event = create_test_event();

        // 测试发送事件
        assert!(sink.send_event(event).await.is_ok());
        assert_eq!(sink.event_count(), 1);

        // 测试健康检查
        assert!(sink.is_healthy().await);

        // 测试统计信息
        let stats = sink.stats();
        assert!(stats.contains("test_stackable"));
        assert!(stats.contains("1 events"));

        // 测试关闭
        assert!(sink.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_exclusive_sink_basic_functionality() {
        let sink = MockExclusiveSink::new("test_exclusive");
        let event = create_test_event();

        // 测试发送事件
        assert!(sink.send_event(event).await.is_ok());

        // 测试健康检查
        assert!(sink.is_healthy().await);

        // 测试元数据
        let metadata = sink.metadata();
        assert_eq!(metadata.name, "test_exclusive");
        assert_eq!(metadata.sink_type, SinkType::Stackable);

        // 测试关闭
        assert!(sink.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_sink_error_handling() {
        let sink = MockSink::new("failing_sink").with_failure();
        let event = create_test_event();

        // 测试失败情况
        let result = sink.send_event(event).await;
        assert!(result.is_err());

        if let Err(SinkError::Generic(msg)) = result {
            assert_eq!(msg, "Mock failure");
        } else {
            panic!("Expected Generic error");
        }

        // 测试健康检查返回 false
        assert!(!sink.is_healthy().await);
    }

    #[tokio::test]
    async fn test_sink_error_types() {
        // 测试不同类型的错误
        let io_error = SinkError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Permission denied",
        ));
        assert!(io_error.to_string().contains("I/O error"));

        let config_error = SinkError::Config("Invalid config".to_string());
        assert!(config_error.to_string().contains("Configuration error"));

        let network_error = SinkError::Network("Connection failed".to_string());
        assert!(network_error.to_string().contains("Network error"));

        let closed_error = SinkError::Closed;
        assert!(closed_error.to_string().contains("Sink is closed"));

        let backpressure_error = SinkError::Backpressure;
        assert!(backpressure_error.to_string().contains("Backpressure"));
    }

    #[tokio::test]
    async fn test_sink_metadata() {
        let sink = MockSink::new("metadata_test");
        let metadata = sink.metadata();

        assert_eq!(metadata.name, "metadata_test");
        assert_eq!(metadata.sink_type, SinkType::Stackable);
        assert!(metadata.enabled);
        assert!(metadata.description.as_ref().unwrap().contains("Mock sink"));
    }

    #[tokio::test]
    async fn test_concurrent_sink_operations() {
        let sink = Arc::new(MockSink::new("concurrent_test"));
        let mut handles = vec![];

        // 并发发送多个事件
        for i in 0..10 {
            let sink_clone = Arc::clone(&sink);
            let handle = tokio::spawn(async move {
                let mut event = create_test_event();
                event.message = format!("Message {}", i);
                sink_clone.send_event(event).await
            });
            handles.push(handle);
        }

        // 等待所有任务完成
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }

        // 验证所有事件都被处理
        assert_eq!(sink.event_count(), 10);
    }
}
