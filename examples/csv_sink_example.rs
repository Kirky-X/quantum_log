//! CSV 输出管道示例
//!
//! 这个示例演示如何实现一个自定义的 CSV 输出管道，将日志数据格式化为 CSV 格式并写入文件。
//! 展示了 QuantumSink trait 的完整实现，包括异步处理、错误处理和资源管理。

use async_trait::async_trait;
use chrono;
use quantum_log::{
    config::*,
    core::event::{ContextInfo, QuantumLogEvent},
    init_with_config, shutdown,
    sinks::{
        traits::{QuantumSink, SinkError, SinkMetadata, SinkType, StackableSink},
        ErrorStrategy, Pipeline, PipelineConfig,
    },
};
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn, Level};

/// CSV 输出管道结构体
///
/// 实现将日志事件格式化为 CSV 格式并写入文件的功能
#[derive(Debug, Clone)]
pub struct CsvSink {
    /// 输出文件路径
    file_path: PathBuf,
    /// 文件写入器，使用 Arc<Mutex<>> 确保线程安全
    writer: Arc<Mutex<Option<BufWriter<std::fs::File>>>>,
    /// 是否包含表头
    include_header: bool,
    /// 是否已写入表头
    header_written: Arc<Mutex<bool>>,
    /// 字段分隔符
    delimiter: char,
}

impl CsvSink {
    /// 创建新的 CSV 输出管道
    ///
    /// # 参数
    ///
    /// * `file_path` - CSV 文件路径
    /// * `include_header` - 是否包含 CSV 表头
    /// * `delimiter` - 字段分隔符，默认为逗号
    pub fn new<P: Into<PathBuf>>(
        file_path: P,
        include_header: bool,
        delimiter: Option<char>,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            writer: Arc::new(Mutex::new(None)),
            include_header,
            header_written: Arc::new(Mutex::new(false)),
            delimiter: delimiter.unwrap_or(','),
        }
    }

    /// 初始化文件写入器
    fn initialize_writer(&self) -> Result<(), SinkError> {
        let mut writer_guard = self
            .writer
            .lock()
            .map_err(|_| SinkError::Config("Failed to acquire writer lock".to_string()))?;

        if writer_guard.is_none() {
            // 创建父目录（如果不存在）
            if let Some(parent) = self.file_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| SinkError::Config(format!("Failed to create directory: {}", e)))?;
            }

            // 打开文件进行追加写入
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
                .map_err(|e| SinkError::Config(format!("Failed to open CSV file: {}", e)))?;

            let buf_writer = BufWriter::new(file);
            *writer_guard = Some(buf_writer);
        }

        Ok(())
    }

    /// 写入 CSV 表头
    fn write_header(&self) -> Result<(), SinkError> {
        if !self.include_header {
            return Ok(());
        }

        let mut header_written = self
            .header_written
            .lock()
            .map_err(|_| SinkError::Config("Failed to acquire header lock".to_string()))?;

        if *header_written {
            return Ok(());
        }

        let mut writer_guard = self
            .writer
            .lock()
            .map_err(|_| SinkError::Config("Failed to acquire writer lock".to_string()))?;

        if let Some(ref mut writer) = *writer_guard {
            let header = format!(
                "timestamp{}level{}target{}message{}file{}line{}module_path{}thread_name{}thread_id{}process_id{}hostname{}username\n",
                self.delimiter, self.delimiter, self.delimiter, self.delimiter,
                self.delimiter, self.delimiter, self.delimiter, self.delimiter,
                self.delimiter, self.delimiter, self.delimiter
            );

            writer
                .write_all(header.as_bytes())
                .map_err(|e| SinkError::Io(e))?;

            writer.flush().map_err(|e| SinkError::Io(e))?;

            *header_written = true;
        }

        Ok(())
    }

    /// 转义 CSV 字段中的特殊字符
    fn escape_csv_field(&self, field: &str) -> String {
        if field.contains(self.delimiter)
            || field.contains('"')
            || field.contains('\n')
            || field.contains('\r')
        {
            format!("\"{}\"", field.replace("\"", "\"\""))
        } else {
            field.to_string()
        }
    }

    /// 将日志事件格式化为 CSV 行
    fn format_event_as_csv(&self, event: &QuantumLogEvent) -> String {
        let timestamp = event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        let level = event.level.to_string();
        let target = self.escape_csv_field(&event.target);
        let message = self.escape_csv_field(&event.message);
        let file = self.escape_csv_field(&event.file.as_deref().unwrap_or(""));
        let line = event.line.map(|l| l.to_string()).unwrap_or_default();
        let module_path = self.escape_csv_field(&event.module_path.as_deref().unwrap_or(""));
        let thread_name = self.escape_csv_field(&event.thread_name.as_deref().unwrap_or(""));
        let thread_id = event.thread_id.to_string();
        let process_id = event.context.pid.to_string();
        let hostname = self.escape_csv_field(&event.context.hostname.as_deref().unwrap_or(""));
        let username = self.escape_csv_field(&event.context.username.as_deref().unwrap_or(""));

        format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
            timestamp,
            self.delimiter,
            level.to_string(),
            self.delimiter,
            target,
            self.delimiter,
            message,
            self.delimiter,
            file,
            self.delimiter,
            line,
            self.delimiter,
            module_path,
            self.delimiter,
            thread_name,
            self.delimiter,
            thread_id,
            self.delimiter,
            process_id,
            self.delimiter,
            hostname,
            self.delimiter,
            username
        )
    }
}

#[async_trait]
impl QuantumSink for CsvSink {
    type Config = ();
    type Error = SinkError;

    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
        // 初始化写入器（如果尚未初始化）
        self.initialize_writer()?;

        // 写入表头（如果需要且尚未写入）
        self.write_header()?;

        // 格式化事件为 CSV 行
        let csv_line = self.format_event_as_csv(&event);

        // 写入 CSV 行
        let mut writer_guard = self
            .writer
            .lock()
            .map_err(|_| SinkError::Config("Failed to acquire writer lock".to_string()))?;

        if let Some(ref mut writer) = *writer_guard {
            writeln!(writer, "{}", csv_line).map_err(|e| SinkError::Io(e))?;

            writer.flush().map_err(|e| SinkError::Io(e))?;
        }

        Ok(())
    }

    async fn shutdown(&self) -> Result<(), Self::Error> {
        let mut writer_guard = self.writer.lock().map_err(|_| {
            SinkError::Config("Failed to acquire writer lock during shutdown".to_string())
        })?;

        if let Some(ref mut writer) = *writer_guard {
            writer.flush().map_err(|e| SinkError::Io(e))?;
        }

        *writer_guard = None;
        Ok(())
    }

    async fn is_healthy(&self) -> bool {
        // 检查文件路径是否可写
        if let Some(parent) = self.file_path.parent() {
            parent.exists()
                && parent
                    .metadata()
                    .map(|m| !m.permissions().readonly())
                    .unwrap_or(false)
        } else {
            true
        }
    }

    fn name(&self) -> &'static str {
        "csv_sink"
    }

    fn metadata(&self) -> SinkMetadata {
        SinkMetadata::new("csv_sink".to_string(), SinkType::Stackable)
            .with_description("CSV file output sink".to_string())
    }
}

impl StackableSink for CsvSink {}

/// 创建测试用的日志事件
fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
    QuantumLogEvent {
        timestamp: chrono::Utc::now(),
        level: level.to_string(),
        target: "csv_example".to_string(),
        message: message.to_string(),
        file: Some("csv_sink_example.rs".to_string()),
        line: Some(42),
        module_path: Some("csv_sink_example".to_string()),
        thread_name: Some("main".to_string()),
        thread_id: format!("{:?}", std::thread::current().id()),
        context: ContextInfo {
            pid: std::process::id(),
            tid: 5678,
            hostname: Some("localhost".to_string()),
            username: Some("user".to_string()),
            mpi_rank: None,
            custom_fields: HashMap::new(),
        },
        fields: HashMap::new(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== CSV 输出管道示例 ===");

    // 创建配置，启用标准输出用于演示
    let config = QuantumLoggerConfig {
        global_level: "info".to_string(),
        pre_init_buffer_size: Some(1000),
        pre_init_stdout_enabled: true,
        backpressure_strategy: BackpressureStrategy::Block,
        stdout: Some(StdoutConfig {
            enabled: true,
            level: Some("info".to_string()),
            color_enabled: Some(true),
            level_colors: None,
            format: OutputFormat::Text,
            colored: true,
        }),
        file: None,
        database: None,
        network: None,
        influxdb: None,
        level_file: None,
        context_fields: ContextFieldsConfig {
            timestamp: true,
            level: true,
            target: true,
            file_line: false,
            pid: false,
            tid: false,
            mpi_rank: false,
            username: false,
            hostname: false,
            span_info: false,
        },
        format: LogFormatConfig {
            timestamp_format: "%Y-%m-%d %H:%M:%S%.3f".to_string(),
            template: "{timestamp} [{level}] {target}: {message}".to_string(),
            csv_columns: Some(vec![
                "timestamp".to_string(),
                "level".to_string(),
                "target".to_string(),
                "message".to_string(),
                "pid".to_string(),
                "file".to_string(),
                "line".to_string(),
            ]),
            json_flatten_fields: false,
            json_fields_key: "fields".to_string(),
            format_type: LogFormatType::Text,
        },
    };

    // 初始化 QuantumLog
    init_with_config(config).await?;
    println!("QuantumLog 已初始化，CSV 输出已配置");

    // 创建 CSV sink
    let csv_sink = CsvSink::new("logs/output.csv", true, Some(','));

    // 创建 Pipeline 来管理 CSV sink
    let pipeline_config = PipelineConfig {
        name: "csv_pipeline".to_string(),
        parallel_processing: false,
        max_retries: 3,
        error_strategy: ErrorStrategy::LogAndContinue,
    };

    let pipeline = Pipeline::new(pipeline_config);

    // 添加 CSV sink 到 pipeline
    let metadata = csv_sink.metadata();
    pipeline
        .add_stackable_sink(csv_sink.clone(), metadata)
        .await?;

    println!("\n1. 测试 CSV sink 初始化");
    println!("CSV sink 名称: {}", csv_sink.name());
    println!("CSV sink 元数据: {:?}", csv_sink.metadata());
    println!("CSV sink 健康状态: {}", csv_sink.is_healthy().await);

    // 记录不同级别的日志到标准输出
    println!("\n2. 记录日志到标准输出");
    info!("这是一条信息日志");
    warn!("这是一条警告日志");
    error!("这是一条错误日志");

    // 记录带有字段的结构化日志
    info!(user_id = 12345, action = "login", "用户登录成功");
    warn!(error_code = 404, path = "/api/users", "API 路径未找到");

    // 直接向 CSV sink 发送事件
    println!("\n3. 直接向 CSV sink 发送事件");
    let events = vec![
        create_test_event(Level::INFO, "CSV 测试信息日志"),
        create_test_event(Level::WARN, "CSV 测试警告日志"),
        create_test_event(Level::ERROR, "CSV 测试错误日志"),
    ];

    for event in events {
        csv_sink.send_event(event).await?;
    }

    // 通过 Pipeline 发送事件
    println!("\n4. 通过 Pipeline 发送事件");
    let pipeline_events = vec![
        create_test_event(Level::INFO, "Pipeline 信息日志"),
        create_test_event(Level::WARN, "Pipeline 警告日志"),
    ];

    for event in pipeline_events {
        pipeline.send_event(event).await?;
    }

    // 等待一段时间确保所有日志都被处理
    sleep(Duration::from_millis(100)).await;

    // 检查 Pipeline 健康状态
    println!("\n5. 检查 Pipeline 健康状态");
    let health = pipeline.health_check().await;
    println!("Pipeline 整体健康状态: {}", health.overall_healthy);
    println!("健康的 sink 数量: {}", health.healthy_sinks);
    println!("不健康的 sink 数量: {}", health.unhealthy_sinks);

    // 优雅关闭
    println!("\n6. 优雅关闭");
    pipeline.shutdown().await?;
    csv_sink.shutdown().await?;
    shutdown().await?;

    println!("\n=== CSV 输出管道示例完成 ===");
    println!("请检查 logs/output.csv 文件查看 CSV 格式的日志输出");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_csv_sink_creation() {
        let temp_dir = tempdir().unwrap();
        let csv_path = temp_dir.path().join("test.csv");

        let sink = CsvSink::new(csv_path, true, Some(','));
        assert_eq!(sink.name(), "csv_sink");
        assert_eq!(sink.metadata().sink_type, SinkType::Stackable);
    }

    #[tokio::test]
    async fn test_csv_sink_send_event() {
        let temp_dir = tempdir().unwrap();
        let csv_path = temp_dir.path().join("test_event.csv");

        let sink = CsvSink::new(&csv_path, true, Some(','));
        let event = create_test_event(Level::INFO, "测试消息");

        assert!(sink.send_event(event).await.is_ok());
        assert!(sink.is_healthy().await);
        assert!(sink.shutdown().await.is_ok());

        // 验证文件是否创建
        assert!(csv_path.exists());
    }

    #[tokio::test]
    async fn test_csv_sink_with_pipeline() {
        let temp_dir = tempdir().unwrap();
        let csv_path = temp_dir.path().join("test_pipeline.csv");

        let sink = CsvSink::new(&csv_path, true, Some(','));

        let config = PipelineConfig {
            name: "test_pipeline".to_string(),
            parallel_processing: false,
            max_retries: 2,
            error_strategy: ErrorStrategy::LogAndContinue,
        };

        let pipeline = Pipeline::new(config);
        let metadata = sink.metadata();
        pipeline
            .add_stackable_sink(sink.clone(), metadata)
            .await
            .unwrap();

        let event = create_test_event(Level::INFO, "Pipeline 测试");
        assert!(pipeline.send_event(event).await.is_ok());

        let health = pipeline.health_check().await;
        assert!(health.overall_healthy);

        assert!(pipeline.shutdown().await.is_ok());
        assert!(sink.shutdown().await.is_ok());
    }
}
