//! QuantumLog 0.2.0 版本统一 Sink Trait 使用示例
//!
//! 本示例展示如何使用新的统一 Sink trait 系统，包括：
//! - 使用 Pipeline 管理多个 sink
//! - 创建自定义 sink 实现
//! - 区分独占型和可叠加型 sink
//! - 使用默认标准输出 sink

use quantum_log::{
    config::*,
    core::event::{ContextInfo, QuantumLogEvent},
    sinks::{
        default_stdout::{DefaultStdoutConfig, DefaultStdoutSink, StdoutTarget},
        traits::{ExclusiveSink, QuantumSink, SinkError, SinkMetadata, SinkType, StackableSink},
        ErrorStrategy, Pipeline, PipelineBuilder, PipelineConfig,
    },
};
use std::collections::HashMap;

use async_trait::async_trait;

use tokio::time::{sleep, Duration};
use tracing::Level;

/// 自定义的可叠加型 Sink 示例
#[derive(Debug, Clone)]
pub struct CustomStackableSink {
    name: String,
    prefix: String,
}

impl CustomStackableSink {
    pub fn new(name: String, prefix: String) -> Self {
        Self { name, prefix }
    }
}

#[async_trait]
impl QuantumSink for CustomStackableSink {
    type Config = ();
    type Error = SinkError;

    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
        println!(
            "[{}] {}: {} - {}",
            self.prefix, event.level, event.target, event.message
        );
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), Self::Error> {
        println!("[{}] Shutting down custom sink: {}", self.prefix, self.name);
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "custom_stackable"
    }

    fn stats(&self) -> String {
        format!(
            "CustomStackableSink: name={}, prefix={}",
            self.name, self.prefix
        )
    }

    fn metadata(&self) -> SinkMetadata {
        SinkMetadata::new(self.name.clone(), SinkType::Stackable).with_description(format!(
            "Custom stackable sink with prefix: {}",
            self.prefix
        ))
    }
}

// 标记为可叠加型 sink
impl StackableSink for CustomStackableSink {}

/// 自定义的独占型 Sink 示例
#[derive(Debug, Clone)]
pub struct CustomExclusiveSink {
    name: String,
    buffer: Vec<String>,
}

impl CustomExclusiveSink {
    pub fn new(name: String) -> Self {
        Self {
            name,
            buffer: Vec::new(),
        }
    }

    pub fn get_logs(&self) -> &[String] {
        &self.buffer
    }
}

#[async_trait]
impl QuantumSink for CustomExclusiveSink {
    type Config = ();
    type Error = SinkError;

    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
        // 注意：这里需要内部可变性来修改 buffer
        // 在实际实现中，应该使用 Arc<Mutex<Vec<String>>> 或类似的结构
        println!(
            "[EXCLUSIVE] {} - {} - {}",
            event.level, event.target, event.message
        );
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), Self::Error> {
        println!(
            "[EXCLUSIVE] Shutting down custom exclusive sink: {}",
            self.name
        );
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "custom_exclusive"
    }

    fn stats(&self) -> String {
        format!(
            "CustomExclusiveSink: name={}, buffer_size={}",
            self.name,
            self.buffer.len()
        )
    }

    fn metadata(&self) -> SinkMetadata {
        SinkMetadata::new(self.name.clone(), SinkType::Exclusive)
            .with_description("Custom exclusive sink for testing".to_string())
    }
}

// 标记为独占型 sink
impl ExclusiveSink for CustomExclusiveSink {}

/// 创建测试事件
fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
    QuantumLogEvent {
        timestamp: chrono::Utc::now(),
        level: level.to_string(),
        target: "example".to_string(),
        message: message.to_string(),
        module_path: Some("sink_trait_usage".to_string()),
        file: Some("sink_trait_usage.rs".to_string()),
        line: Some(42),
        thread_name: Some("main".to_string()),
        thread_id: format!("{:?}", std::thread::current().id()),
        fields: HashMap::new(),
        context: ContextInfo {
            pid: std::process::id(),
            tid: 0, // 简化处理
            username: None,
            hostname: None,
            mpi_rank: None,
            custom_fields: HashMap::new(),
        },
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== QuantumLog 0.2.0 统一 Sink Trait 使用示例 ===");

    // 1. 使用默认标准输出 sink
    println!("\n1. 使用默认标准输出 sink");
    let default_config = DefaultStdoutConfig {
        format: OutputFormat::Text,
        colored: true,
        target: StdoutTarget::Stdout,
        ..Default::default()
    };
    let default_stdout = DefaultStdoutSink::new(default_config).await?;

    let json_config = DefaultStdoutConfig {
        format: OutputFormat::Json,
        colored: false,
        target: StdoutTarget::Stdout,
        ..Default::default()
    };
    let json_stdout = DefaultStdoutSink::new(json_config).await?;

    let event1 = create_test_event(Level::INFO, "使用默认标准输出 sink");
    default_stdout.send_event(event1).await?;

    let event2 = create_test_event(Level::WARN, "使用 JSON 格式标准输出 sink");
    json_stdout.send_event(event2).await?;

    // 2. 创建自定义 sink
    println!("\n2. 创建自定义 sink");
    let custom_stackable1 = CustomStackableSink::new("Logger1".to_string(), "CUSTOM1".to_string());
    let custom_stackable2 = CustomStackableSink::new("Logger2".to_string(), "CUSTOM2".to_string());
    let custom_exclusive = CustomExclusiveSink::new("ExclusiveLogger".to_string());

    let event3 = create_test_event(Level::ERROR, "自定义 sink 测试消息");
    custom_stackable1.send_event(event3.clone()).await?;
    custom_stackable2.send_event(event3.clone()).await?;
    custom_exclusive.send_event(event3).await?;

    // 3. 使用 Pipeline 管理多个 sink
    println!("\n3. 使用 Pipeline 管理多个 sink");

    let pipeline_config = PipelineConfig {
        name: "example_pipeline".to_string(),
        parallel_processing: true,
        max_retries: 3,
        error_strategy: ErrorStrategy::LogAndContinue,
    };

    let pipeline = Pipeline::new(pipeline_config);

    // 添加可叠加型 sink（可以有多个）
    pipeline
        .add_stackable_sink(custom_stackable1.clone(), custom_stackable1.metadata())
        .await?;
    pipeline
        .add_stackable_sink(custom_stackable2.clone(), custom_stackable2.metadata())
        .await?;

    // 通过 pipeline 发送事件
    let events = vec![
        create_test_event(Level::INFO, "Pipeline 测试消息 1"),
        create_test_event(Level::WARN, "Pipeline 测试消息 2"),
        create_test_event(Level::ERROR, "Pipeline 测试消息 3"),
    ];

    for event in events {
        pipeline.send_event(event).await?;
    }

    // 等待事件处理
    sleep(Duration::from_millis(100)).await;

    // 4. 获取 pipeline 统计信息
    println!("\n4. Pipeline 统计信息");
    let stats = pipeline.get_stats().await;
    println!("Pipeline 统计: {:?}", stats);

    // 5. 健康检查
    println!("\n5. 健康检查");
    let health = pipeline.health_check().await;
    println!("Pipeline 健康状态: {:?}", health);

    // 6. 优雅关闭
    println!("\n6. 优雅关闭 Pipeline");
    pipeline.shutdown().await?;

    // 7. 单独关闭其他 sink
    println!("\n7. 关闭其他 sink");
    json_stdout.shutdown().await?;

    println!("\n=== 示例完成 ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_custom_stackable_sink() {
        let sink = CustomStackableSink::new("test".to_string(), "TEST".to_string());

        let event = create_test_event(Level::INFO, "测试消息");
        assert!(sink.send_event(event).await.is_ok());
        assert!(sink.is_healthy().await);
        assert_eq!(sink.name(), "custom_stackable");
        assert!(sink.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_custom_exclusive_sink() {
        let sink = CustomExclusiveSink::new("test".to_string());

        let event = create_test_event(Level::ERROR, "测试错误消息");
        assert!(sink.send_event(event).await.is_ok());
        assert!(sink.is_healthy().await);
        assert_eq!(sink.name(), "custom_exclusive");
        assert!(sink.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn test_pipeline_with_custom_sinks() {
        let config = PipelineConfig {
            name: "test_pipeline".to_string(),
            parallel_processing: false,
            max_retries: 2,
            error_strategy: ErrorStrategy::LogAndContinue,
        };

        let pipeline = Pipeline::new(config);

        let stackable = CustomStackableSink::new("test_stackable".to_string(), "TEST".to_string());

        pipeline
            .add_stackable_sink(stackable.clone(), stackable.metadata())
            .await
            .unwrap();

        let event = create_test_event(Level::INFO, "Pipeline 测试");
        assert!(pipeline.send_event(event).await.is_ok());

        sleep(Duration::from_millis(50)).await;

        let health = pipeline.health_check().await;
        assert!(health.overall_healthy);
        assert!(pipeline.shutdown().await.is_ok());
    }

    // 新增：覆盖 add_exclusive_sink API
    #[tokio::test]
    async fn test_pipeline_add_exclusive_sink() {
        let pipeline = Pipeline::new(PipelineConfig::default());
        let exclusive = CustomExclusiveSink::new("exclusive_one".to_string());

        pipeline
            .add_exclusive_sink(exclusive.clone(), exclusive.metadata())
            .await
            .unwrap();

        // 发送事件，验证不报错且统计信息包含独占型 sink
        let event = create_test_event(Level::INFO, "独占型 sink 事件");
        assert!(pipeline.send_event(event).await.is_ok());

        let stats = pipeline.get_stats().await;
        assert_eq!(stats.exclusive_sink_count, 1);
        assert_eq!(stats.enabled_sink_count, 1);

        pipeline.shutdown().await.unwrap();
    }

    // 新增：覆盖 PipelineBuilder::set_exclusive_sink API
    #[tokio::test]
    async fn test_pipeline_builder_set_exclusive_sink() {
        let exclusive = CustomExclusiveSink::new("builder_exclusive".to_string());

        let pipeline = PipelineBuilder::new()
            .with_name("builder_pipeline".to_string())
            .with_parallel_processing(true)
            .with_max_retries(3)
            .with_error_strategy(ErrorStrategy::LogAndContinue)
            .set_exclusive_sink(exclusive.clone(), exclusive.metadata())
            .build();

        // 发送事件并检查统计
        let event = create_test_event(Level::WARN, "通过构建器设置独占型 sink");
        assert!(pipeline.send_event(event).await.is_ok());

        let stats = pipeline.get_stats().await;
        assert_eq!(stats.name, "builder_pipeline");
        assert_eq!(stats.exclusive_sink_count, 1);
        assert_eq!(stats.enabled_sink_count, 1);

        pipeline.shutdown().await.unwrap();
    }

    // 新增：同时存在独占型与可叠加型 sink 的发送路径
    #[tokio::test]
    async fn test_pipeline_with_exclusive_and_stackable_sinks() {
        let pipeline = Pipeline::new(PipelineConfig::default());

        let exclusive = CustomExclusiveSink::new("exclusive".to_string());
        let stackable = CustomStackableSink::new("stackable".to_string(), "S".to_string());

        pipeline
            .add_exclusive_sink(exclusive.clone(), exclusive.metadata())
            .await
            .unwrap();

        // 尝试在存在独占型 sink 时添加可叠加型 sink，应返回配置错误
        let res = pipeline
            .add_stackable_sink(stackable.clone(), stackable.metadata())
            .await;
        assert!(
            matches!(res, Err(SinkError::Config(ref msg)) if msg.contains("Cannot add stackable sink when exclusive sink is present")),
            "expected Config error when adding stackable with exclusive present, got: {:?}",
            res
        );

        // 发送多条事件，确保独占型 sink 可处理，且统计反映互斥约束
        for i in 0..3 {
            let event = create_test_event(Level::INFO, &format!("事件 {}", i));
            assert!(pipeline.send_event(event).await.is_ok());
        }

        let stats = pipeline.get_stats().await;
        assert_eq!(stats.exclusive_sink_count, 1);
        assert_eq!(stats.stackable_sink_count, 0);
        assert_eq!(stats.enabled_sink_count, 1);

        pipeline.shutdown().await.unwrap();
    }
}
