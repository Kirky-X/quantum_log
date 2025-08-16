//! QuantumLog Pipeline Manager
//!
//! 管道系统用于管理多个 sink 的协调工作，实现以下规则：
//! - 只有标准输出类型的 sink 可以叠加到管道中
//! - 其他类型的 sink 是独占的，不能与其他 sink 同时使用
//! - 提供统一的事件分发和错误处理机制
//!
//! # 使用示例
//!
//! ```rust
//! use quantum_log::sinks::pipeline::{Pipeline, PipelineBuilder};
//! use quantum_log::sinks::default_stdout::DefaultStdoutSink;
//! use quantum_log::sinks::traits::{SinkMetadata, QuantumSink};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let sink = DefaultStdoutSink::new(Default::default()).await?;
//!     let metadata = sink.metadata();
//!     
//!     let pipeline = PipelineBuilder::new()
//!         .add_stackable_sink(sink, metadata)
//!         .build();
//!     
//!     // 使用管道发送事件
//!     // pipeline.send_event(event).await?;
//!     
//!     // 关闭管道
//!     pipeline.shutdown().await?;
//!     Ok(())
//! }
//! ```

use crate::core::event::QuantumLogEvent;
use crate::sinks::traits::{
    ExclusiveSink, QuantumSinkDyn, SinkError, SinkMetadata, SinkResult, SinkType, StackableSink,
};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// 管道配置
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// 管道名称
    pub name: String,
    /// 是否启用并行处理
    pub parallel_processing: bool,
    /// 最大重试次数
    pub max_retries: u32,
    /// 错误处理策略
    pub error_strategy: ErrorStrategy,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            name: "default_pipeline".to_string(),
            parallel_processing: true,
            max_retries: 3,
            error_strategy: ErrorStrategy::LogAndContinue,
        }
    }
}

/// 错误处理策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorStrategy {
    /// 记录错误并继续处理
    LogAndContinue,
    /// 遇到错误时停止管道
    StopOnError,
    /// 忽略错误
    Ignore,
}

/// Sink 包装器，用于在管道中管理 sink
#[derive(Debug)]
struct SinkWrapper {
    /// Sink 实例
    sink: Arc<dyn QuantumSinkDyn>,
    /// Sink 元数据
    metadata: SinkMetadata,
    /// 是否启用
    enabled: bool,
}

/// 管道实现
///
/// 管道负责协调多个 sink 的工作，确保事件正确分发到所有启用的 sink。
#[derive(Debug)]
pub struct Pipeline {
    /// 管道配置
    config: PipelineConfig,
    /// 独占型 sink（最多只能有一个）
    exclusive_sink: Arc<RwLock<Option<SinkWrapper>>>,
    /// 可叠加型 sink 列表
    stackable_sinks: Arc<RwLock<HashMap<String, SinkWrapper>>>,
    /// 管道状态
    is_running: Arc<RwLock<bool>>,
}

impl Pipeline {
    /// 创建新的管道实例
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            exclusive_sink: Arc::new(RwLock::new(None)),
            stackable_sinks: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(true)),
        }
    }

    /// 新增：允许在构造时初始化已有的 sinks
    fn with_initial(
        config: PipelineConfig,
        exclusive: Option<SinkWrapper>,
        stackables: HashMap<String, SinkWrapper>,
    ) -> Self {
        Self {
            config,
            exclusive_sink: Arc::new(RwLock::new(exclusive)),
            stackable_sinks: Arc::new(RwLock::new(stackables)),
            is_running: Arc::new(RwLock::new(true)),
        }
    }

    /// 添加独占型 sink
    ///
    /// 如果已经存在独占型 sink 或可叠加型 sink，则返回错误
    pub async fn add_exclusive_sink<T>(&self, sink: T, metadata: SinkMetadata) -> SinkResult<()>
    where
        T: ExclusiveSink<Error = SinkError> + 'static,
    {
        let mut exclusive = self.exclusive_sink.write().await;
        let stackable = self.stackable_sinks.read().await;

        // 检查是否已有独占型 sink
        if exclusive.is_some() {
            return Err(SinkError::Config(
                "Pipeline already contains an exclusive sink".to_string(),
            ));
        }

        // 检查是否已有可叠加型 sink
        if !stackable.is_empty() {
            return Err(SinkError::Config(
                "Cannot add exclusive sink when stackable sinks are present".to_string(),
            ));
        }

        let wrapper = SinkWrapper {
            sink: Arc::new(sink),
            metadata,
            enabled: true,
        };

        *exclusive = Some(wrapper);
        debug!("Added exclusive sink to pipeline: {}", self.config.name);
        Ok(())
    }

    /// 添加可叠加型 sink
    ///
    /// 如果已经存在独占型 sink，则返回错误
    pub async fn add_stackable_sink<T>(&self, sink: T, metadata: SinkMetadata) -> SinkResult<()>
    where
        T: StackableSink<Error = SinkError> + 'static,
    {
        let exclusive = self.exclusive_sink.read().await;
        let mut stackable = self.stackable_sinks.write().await;

        // 检查是否已有独占型 sink
        if exclusive.is_some() {
            return Err(SinkError::Config(
                "Cannot add stackable sink when exclusive sink is present".to_string(),
            ));
        }

        let wrapper = SinkWrapper {
            sink: Arc::new(sink),
            metadata: metadata.clone(),
            enabled: true,
        };

        stackable.insert(metadata.name.clone(), wrapper);
        debug!(
            "Added stackable sink '{}' to pipeline: {}",
            metadata.name, self.config.name
        );
        Ok(())
    }

    /// 移除可叠加型 sink
    pub async fn remove_stackable_sink(&self, name: &str) -> SinkResult<()> {
        let mut stackable = self.stackable_sinks.write().await;

        if let Some(wrapper) = stackable.remove(name) {
            // 优雅关闭被移除的 sink
            if let Err(e) = wrapper.sink.shutdown_dyn().await {
                warn!("Error shutting down removed sink '{}': {}", name, e);
            }
            debug!(
                "Removed stackable sink '{}' from pipeline: {}",
                name, self.config.name
            );
            Ok(())
        } else {
            Err(SinkError::Config(format!("Sink '{}' not found", name)))
        }
    }

    /// 启用或禁用指定的可叠加型 sink
    pub async fn set_stackable_sink_enabled(&self, name: &str, enabled: bool) -> SinkResult<()> {
        let mut stackable = self.stackable_sinks.write().await;

        if let Some(wrapper) = stackable.get_mut(name) {
            wrapper.enabled = enabled;
            debug!(
                "Set stackable sink '{}' enabled={} in pipeline: {}",
                name, enabled, self.config.name
            );
            Ok(())
        } else {
            Err(SinkError::Config(format!("Sink '{}' not found", name)))
        }
    }

    /// 发送事件到所有启用的 sink
    pub async fn send_event(&self, event: QuantumLogEvent) -> SinkResult<()> {
        let is_running = *self.is_running.read().await;
        if !is_running {
            return Err(SinkError::Closed);
        }

        // 处理独占型 sink
        if let Some(wrapper) = &*self.exclusive_sink.read().await {
            if wrapper.enabled {
                self.send_to_sink(&wrapper.sink, event.clone()).await?;
            }
            return Ok(());
        }

        // 处理可叠加型 sink
        let stackable = self.stackable_sinks.read().await;
        if stackable.is_empty() {
            warn!("No sinks available in pipeline: {}", self.config.name);
            return Ok(());
        }

        if self.config.parallel_processing {
            // 并行处理
            let tasks: Vec<_> = stackable
                .values()
                .filter(|wrapper| wrapper.enabled)
                .map(|wrapper| {
                    let sink = wrapper.sink.clone();
                    let event = event.clone();
                    let error_strategy = self.config.error_strategy.clone();
                    tokio::spawn(async move {
                        match sink.send_event_dyn(event).await {
                            Ok(()) => Ok(()),
                            Err(e) => match error_strategy {
                                ErrorStrategy::StopOnError => Err(e),
                                ErrorStrategy::Ignore => Ok(()),
                                ErrorStrategy::LogAndContinue => {
                                    warn!("Sink error in parallel processing: {}", e);
                                    Ok(())
                                }
                            },
                        }
                    })
                })
                .collect();

            // 等待所有任务完成
            for task in tasks {
                match task.await {
                    Ok(Ok(())) => {} // 任务成功完成
                    Ok(Err(e)) => {
                        // 任务返回了业务错误
                        if matches!(self.config.error_strategy, ErrorStrategy::StopOnError) {
                            return Err(e);
                        }
                    }
                    Err(e) => {
                        // 任务执行错误（panic等）
                        error!("Task execution error: {}", e);
                        if matches!(self.config.error_strategy, ErrorStrategy::StopOnError) {
                            return Err(SinkError::Generic(format!(
                                "Task execution failed: {}",
                                e
                            )));
                        }
                    }
                }
            }
        } else {
            // 串行处理
            for wrapper in stackable.values() {
                if wrapper.enabled {
                    self.send_to_sink(&wrapper.sink, event.clone()).await?;
                }
            }
        }

        Ok(())
    }

    /// 发送事件到指定 sink，包含错误处理和重试逻辑
    async fn send_to_sink(
        &self,
        sink: &Arc<dyn QuantumSinkDyn>,
        event: QuantumLogEvent,
    ) -> SinkResult<()> {
        let mut retries = 0;

        loop {
            match sink.send_event_dyn(event.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    retries += 1;

                    match self.config.error_strategy {
                        ErrorStrategy::StopOnError => return Err(e),
                        ErrorStrategy::Ignore => return Ok(()),
                        ErrorStrategy::LogAndContinue => {
                            if retries <= self.config.max_retries {
                                warn!(
                                    "Sink error (retry {}/{}): {}. Retrying...",
                                    retries, self.config.max_retries, e
                                );
                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    100 * retries as u64,
                                ))
                                .await;
                                continue;
                            } else {
                                error!(
                                    "Sink error after {} retries: {}. Giving up.",
                                    self.config.max_retries, e
                                );
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }

    /// 获取管道统计信息
    pub async fn get_stats(&self) -> PipelineStats {
        let mut stats = PipelineStats {
            name: self.config.name.clone(),
            is_running: *self.is_running.read().await,
            exclusive_sink_count: 0,
            stackable_sink_count: 0,
            enabled_sink_count: 0,
            sink_details: Vec::new(),
        };

        // 统计独占型 sink
        if let Some(wrapper) = &*self.exclusive_sink.read().await {
            stats.exclusive_sink_count = 1;
            if wrapper.enabled {
                stats.enabled_sink_count += 1;
            }
            stats.sink_details.push(SinkStats {
                name: wrapper.metadata.name.clone(),
                sink_type: SinkType::Exclusive,
                enabled: wrapper.enabled,
                healthy: wrapper.sink.is_healthy_dyn().await,
                stats: wrapper.sink.stats_dyn(),
            });
        }

        // 统计可叠加型 sink
        let stackable = self.stackable_sinks.read().await;
        stats.stackable_sink_count = stackable.len();

        for wrapper in stackable.values() {
            if wrapper.enabled {
                stats.enabled_sink_count += 1;
            }
            stats.sink_details.push(SinkStats {
                name: wrapper.metadata.name.clone(),
                sink_type: SinkType::Stackable,
                enabled: wrapper.enabled,
                healthy: wrapper.sink.is_healthy_dyn().await,
                stats: wrapper.sink.stats_dyn(),
            });
        }

        stats
    }

    /// 优雅关闭管道
    pub async fn shutdown(&self) -> SinkResult<()> {
        debug!("Shutting down pipeline: {}", self.config.name);

        // 标记管道为非运行状态
        *self.is_running.write().await = false;

        // 关闭独占型 sink
        if let Some(wrapper) = &*self.exclusive_sink.read().await {
            if let Err(e) = wrapper.sink.shutdown_dyn().await {
                error!("Error shutting down exclusive sink: {}", e);
            }
        }

        // 关闭所有可叠加型 sink
        let stackable = self.stackable_sinks.read().await;
        for (name, wrapper) in stackable.iter() {
            if let Err(e) = wrapper.sink.shutdown_dyn().await {
                error!("Error shutting down stackable sink '{}': {}", name, e);
            }
        }

        debug!("Pipeline shutdown completed: {}", self.config.name);
        Ok(())
    }

    /// 检查管道健康状态
    pub async fn health_check(&self) -> HealthStatus {
        let mut healthy_count = 0;
        let mut unhealthy_count = 0;

        // 检查独占型 sink
        if let Some(wrapper) = &*self.exclusive_sink.read().await {
            if wrapper.enabled {
                if wrapper.sink.is_healthy_dyn().await {
                    healthy_count += 1;
                } else {
                    unhealthy_count += 1;
                }
            }
        }

        // 检查可叠加型 sink
        let stackable = self.stackable_sinks.read().await;
        for wrapper in stackable.values() {
            if wrapper.enabled {
                if wrapper.sink.is_healthy_dyn().await {
                    healthy_count += 1;
                } else {
                    unhealthy_count += 1;
                }
            }
        }

        HealthStatus {
            healthy_sinks: healthy_count,
            unhealthy_sinks: unhealthy_count,
            overall_healthy: unhealthy_count == 0 && healthy_count > 0,
        }
    }
}

/// 管道统计信息
#[derive(Debug, Clone)]
pub struct PipelineStats {
    pub name: String,
    pub is_running: bool,
    pub exclusive_sink_count: usize,
    pub stackable_sink_count: usize,
    pub enabled_sink_count: usize,
    pub sink_details: Vec<SinkStats>,
}

/// Sink 统计信息
#[derive(Debug, Clone)]
pub struct SinkStats {
    pub name: String,
    pub sink_type: SinkType,
    pub enabled: bool,
    pub healthy: bool,
    pub stats: String,
}

/// 管道健康状态
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy_sinks: usize,
    pub unhealthy_sinks: usize,
    pub overall_healthy: bool,
}

/// 管道构建器
///
/// 提供流畅的 API 来构建管道
#[derive(Debug, Default)]
pub struct PipelineBuilder {
    config: PipelineConfig,
    /// 缓存待注入的 sinks
    pending_exclusive: Option<SinkWrapper>,
    pending_stackables: HashMap<String, SinkWrapper>,
}

impl PipelineBuilder {
    /// 创建新的管道构建器
    pub fn new() -> Self {
        Self {
            config: PipelineConfig::default(),
            pending_exclusive: None,
            pending_stackables: HashMap::new(),
        }
    }

    /// 使用指定配置创建管道构建器
    pub fn with_config(config: PipelineConfig) -> Self {
        Self {
            config,
            pending_exclusive: None,
            pending_stackables: HashMap::new(),
        }
    }

    /// 设置管道名称
    pub fn with_name(mut self, name: String) -> Self {
        self.config.name = name;
        self
    }

    /// 设置并行处理
    pub fn with_parallel_processing(mut self, enabled: bool) -> Self {
        self.config.parallel_processing = enabled;
        self
    }

    /// 设置最大重试次数
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// 设置错误处理策略
    pub fn with_error_strategy(mut self, strategy: ErrorStrategy) -> Self {
        self.config.error_strategy = strategy;
        self
    }

    /// 添加可堆叠的Sink
    pub fn add_stackable_sink<T>(mut self, sink: T, metadata: SinkMetadata) -> Self
    where
        T: StackableSink<Error = SinkError> + 'static,
    {
        let name = metadata.name.clone();
        let wrapper = SinkWrapper {
            sink: Arc::new(sink),
            metadata: metadata.clone(),
            enabled: metadata.enabled,
        };
        self.pending_stackables.insert(name, wrapper);
        self
    }

    /// 实际缓存 exclusive sink（若多次设置则覆盖）
    pub fn set_exclusive_sink<T>(mut self, sink: T, metadata: SinkMetadata) -> Self
    where
        T: ExclusiveSink<Error = SinkError> + 'static,
    {
        let wrapper = SinkWrapper {
            sink: Arc::new(sink),
            metadata: metadata.clone(),
            enabled: metadata.enabled,
        };
        self.pending_exclusive = Some(wrapper);
        self
    }

    /// 使用 with_initial 将缓存的 sinks 注入 Pipeline
    pub fn build(self) -> Pipeline {
        Pipeline::with_initial(self.config, self.pending_exclusive, self.pending_stackables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::QuantumLogEvent;
    use crate::sinks::traits::QuantumSink;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    // 测试用的 Mock Sink
    #[derive(Debug)]
    struct MockStackableSink {
        name: String,
        event_count: Arc<AtomicU64>,
        should_fail: bool,
    }

    impl MockStackableSink {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
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
    impl QuantumSink for MockStackableSink {
        type Config = ();
        type Error = SinkError;

        async fn send_event(&self, _event: QuantumLogEvent) -> Result<(), Self::Error> {
            if self.should_fail {
                return Err(SinkError::Generic("Mock failure".to_string()));
            }
            self.event_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn is_healthy(&self) -> bool {
            !self.should_fail
        }

        fn name(&self) -> &'static str {
            "mock_stackable"
        }

        fn stats(&self) -> String {
            format!("MockStackableSink: events={}", self.event_count())
        }

        fn metadata(&self) -> SinkMetadata {
            SinkMetadata {
                name: self.name.clone(),
                sink_type: SinkType::Stackable,
                enabled: true,
                description: Some("Mock stackable sink for testing".to_string()),
            }
        }
    }

    #[async_trait]
    impl StackableSink for MockStackableSink {
        async fn send_event_internal(
            &self,
            event: &QuantumLogEvent,
            _strategy: crate::config::BackpressureStrategy,
        ) -> SinkResult<()> {
            self.send_event(event.clone()).await
        }
    }

    #[derive(Debug)]
    struct MockExclusiveSink {
        name: String,
        event_count: Arc<AtomicU64>,
        should_fail: bool,
    }

    impl MockExclusiveSink {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
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
    impl QuantumSink for MockExclusiveSink {
        type Config = ();
        type Error = SinkError;

        async fn send_event(&self, _event: QuantumLogEvent) -> Result<(), Self::Error> {
            if self.should_fail {
                return Err(SinkError::Generic("Mock failure".to_string()));
            }
            self.event_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn shutdown(&self) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn is_healthy(&self) -> bool {
            !self.should_fail
        }

        fn name(&self) -> &'static str {
            "mock_exclusive"
        }

        fn stats(&self) -> String {
            format!("MockExclusiveSink: events={}", self.event_count())
        }

        fn metadata(&self) -> SinkMetadata {
            SinkMetadata {
                name: self.name.clone(),
                sink_type: SinkType::Exclusive,
                enabled: true,
                description: Some("Mock exclusive sink for testing".to_string()),
            }
        }
    }

    impl ExclusiveSink for MockExclusiveSink {}

    fn create_test_event() -> QuantumLogEvent {
        use crate::core::event::{ContextInfo, QuantumLogEvent};
        use chrono::Utc;
        use std::collections::HashMap;

        QuantumLogEvent {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            message: "Test message".to_string(),
            target: "test_target".to_string(),
            file: Some("test.rs".to_string()),
            line: Some(42),
            module_path: Some("test_module".to_string()),
            thread_name: std::thread::current().name().map(|s| s.to_string()),
            thread_id: format!("{:?}", std::thread::current().id()),
            fields: HashMap::new(),
            context: ContextInfo::default(),
        }
    }

    #[tokio::test]
    async fn test_pipeline_builder_basic() {
        let pipeline = PipelineBuilder::new()
            .with_name("test_pipeline".to_string())
            .with_parallel_processing(false)
            .with_max_retries(5)
            .with_error_strategy(ErrorStrategy::StopOnError)
            .build();

        let stats = pipeline.get_stats().await;
        assert_eq!(stats.name, "test_pipeline");
        assert!(stats.is_running);
        assert_eq!(stats.exclusive_sink_count, 0);
        assert_eq!(stats.stackable_sink_count, 0);
        assert_eq!(stats.enabled_sink_count, 0);

        pipeline.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pipeline_add_stackable_sinks() {
        let pipeline = Pipeline::new(PipelineConfig::default());
        let sink1 = MockStackableSink::new("sink1");
        let sink2 = MockStackableSink::new("sink2");

        let metadata1 = SinkMetadata {
            name: "sink1".to_string(),
            sink_type: SinkType::Stackable,
            enabled: true,
            description: Some("Test sink 1".to_string()),
        };

        let metadata2 = SinkMetadata {
            name: "sink2".to_string(),
            sink_type: SinkType::Stackable,
            enabled: true,
            description: Some("Test sink 2".to_string()),
        };

        pipeline.add_stackable_sink(sink1, metadata1).await.unwrap();
        pipeline.add_stackable_sink(sink2, metadata2).await.unwrap();

        let stats = pipeline.get_stats().await;
        assert_eq!(stats.stackable_sink_count, 2);
        assert_eq!(stats.enabled_sink_count, 2);

        pipeline.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_pipeline_send_event_to_stackable_sinks() {
        let pipeline = Pipeline::new(PipelineConfig::default());
        let sink1 = MockStackableSink::new("sink1");
        let sink2 = MockStackableSink::new("sink2");

        let sink1_count = sink1.event_count.clone();
        let sink2_count = sink2.event_count.clone();

        let metadata1 = SinkMetadata {
            name: "sink1".to_string(),
            sink_type: SinkType::Stackable,
            enabled: true,
            description: Some("Test sink 1".to_string()),
        };

        let metadata2 = SinkMetadata {
            name: "sink2".to_string(),
            sink_type: SinkType::Stackable,
            enabled: true,
            description: Some("Test sink 2".to_string()),
        };

        pipeline.add_stackable_sink(sink1, metadata1).await.unwrap();
        pipeline.add_stackable_sink(sink2, metadata2).await.unwrap();

        let event = create_test_event();
        pipeline.send_event(event).await.unwrap();

        // 由于是并行处理，需要稍等
        sleep(Duration::from_millis(10)).await;

        assert_eq!(sink1_count.load(Ordering::Relaxed), 1);
        assert_eq!(sink2_count.load(Ordering::Relaxed), 1);

        pipeline.shutdown().await.unwrap();
    }
}
