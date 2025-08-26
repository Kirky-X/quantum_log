//! InfluxDB Sink 实现
//!
//! 此模块提供了将日志写入 InfluxDB 的功能，支持 InfluxDB 1.x 和 2.x 版本。
//! 使用批量插入和异步处理来优化性能。

use async_trait::async_trait;
use base64::{self, Engine};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::config::InfluxDBConfig;
use crate::core::event::QuantumLogEvent;
use crate::error::QuantumLogError;
use crate::sinks::traits::{ExclusiveSink, QuantumSink, SinkError, SinkMetadata, SinkType};

type Result<T> = std::result::Result<T, QuantumLogError>;

/// InfluxDB 日志条目
#[derive(Debug, Clone)]
struct InfluxLogEntry {
    /// 时间戳
    timestamp: chrono::DateTime<chrono::Utc>,
    /// 日志级别
    level: String,
    /// 目标模块
    target: String,
    /// 日志消息
    message: String,
    /// 文件路径
    file: Option<String>,
    /// 行号
    line: Option<u32>,
    /// 模块路径
    module_path: Option<String>,
    /// 线程名称
    thread_name: Option<String>,
    /// 线程ID
    _thread_id: String,
    /// 进程ID
    _pid: u32,
    /// 主机名
    hostname: Option<String>,
    /// 用户名
    username: Option<String>,
    /// MPI 排名
    mpi_rank: Option<i32>,
    /// 自定义字段
    fields: std::collections::HashMap<String, serde_json::Value>,
}

/// InfluxDB 日志批次
#[derive(Debug)]
struct LogBatch {
    /// 日志条目
    entries: Vec<InfluxLogEntry>,
    /// 创建时间
    created_at: Instant,
}

impl LogBatch {
    /// 创建新的日志批次
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            created_at: Instant::now(),
        }
    }

    /// 添加日志条目
    fn add_entry(&mut self, entry: InfluxLogEntry) {
        self.entries.push(entry);
    }

    /// 检查批次是否已满
    fn is_full(&self, max_size: usize) -> bool {
        self.entries.len() >= max_size
    }

    /// 检查批次是否过期
    fn is_expired(&self, max_age_seconds: u64) -> bool {
        self.created_at.elapsed() >= Duration::from_secs(max_age_seconds)
    }

    /// 检查批次是否为空
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 获取批次大小
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// 清空批次
    fn clear(&mut self) {
        self.entries.clear();
        self.created_at = Instant::now();
    }

    /// 获取批次中的条目引用
    fn entries(&self) -> &[InfluxLogEntry] {
        &self.entries
    }
}

/// InfluxDB Sink 结构体
#[derive(Debug, Clone)]
pub struct InfluxDBSink {
    /// 配置信息
    config: InfluxDBConfig,
    /// 内部状态（使用Arc<Mutex<>>实现内部可变性）
    inner: Arc<Mutex<InfluxDBSinkInner>>,
}

/// InfluxDB Sink 内部状态
#[derive(Debug)]
struct InfluxDBSinkInner {
    /// 事件发送器
    sender: Option<mpsc::UnboundedSender<QuantumLogEvent>>,
    /// 处理器句柄
    processor_handle: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl InfluxDBSink {
    /// 创建新的 InfluxDB Sink 实例
    ///
    /// # 参数
    /// * `config` - InfluxDB 配置
    ///
    /// # 返回
    /// 返回配置好的 InfluxDBSink 实例或错误
    pub fn new(config: InfluxDBConfig) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(InfluxDBSinkInner {
                sender: None,
                processor_handle: None,
            })),
        }
    }

    /// 启动 InfluxDB Sink 任务
    ///
    /// # 返回
    /// 成功时返回 Ok(())，失败时返回错误信息
    pub async fn start(&mut self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        
        if inner.sender.is_some() {
            return Err(QuantumLogError::ConfigError(
                "InfluxDB Sink already started".into(),
            ));
        }

        let (sender, receiver) = mpsc::unbounded_channel();
        let config = self.config.clone();
        
        let processor = InfluxDBProcessor::new(config).await?;
        let handle = tokio::spawn(async move {
            processor.run(receiver).await
        });

        inner.sender = Some(sender);
        inner.processor_handle = Some(handle);

        info!("InfluxDB Sink started");
        Ok(())
    }

    /// 发送事件到 InfluxDB
    pub async fn send_event(&self, event: QuantumLogEvent) -> Result<()> {
        let inner = self.inner.lock().await;
        
        if let Some(sender) = &inner.sender {
            sender
                .send(event)
                .map_err(|_| QuantumLogError::SinkError("Failed to send event to InfluxDB Sink".into()))?;
            Ok(())
        } else {
            Err(QuantumLogError::SinkError(
                "InfluxDB Sink not started".into(),
            ))
        }
    }

    /// 关闭 InfluxDB Sink
    pub async fn shutdown(&self) -> Result<()> {
        let mut inner = self.inner.lock().await;
        
        if let Some(sender) = inner.sender.take() {
            // 发送关闭信号（通过关闭通道）
            drop(sender);
        }

        if let Some(handle) = inner.processor_handle.take() {
            match tokio::time::timeout(Duration::from_secs(30), handle).await {
                Ok(result) => {
                    match result {
                        Ok(processor_result) => {
                            if let Err(e) = processor_result {
                                error!("InfluxDB processor error during shutdown: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("InfluxDB processor task panicked: {}", e);
                        }
                    }
                }
                Err(_) => {
                    warn!("InfluxDB processor shutdown timeout");
                }
            }
        }

        info!("InfluxDB Sink shutdown completed");
        Ok(())
    }

    /// 检查是否正在运行
    pub async fn is_running(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.sender.is_some()
    }

    /// 获取配置
    pub fn config(&self) -> &InfluxDBConfig {
        &self.config
    }
}

/// InfluxDB 处理器
struct InfluxDBProcessor {
    /// 配置
    config: InfluxDBConfig,
    /// HTTP 客户端
    client: reqwest::Client,
    /// 数据库 URL
    url: String,
    /// 写入路径
    write_path: String,
    /// 认证头
    auth_header: Option<String>,
}

impl InfluxDBProcessor {
    /// 创建新的处理器
    async fn new(mut config: InfluxDBConfig) -> Result<Self> {
        // 从环境变量安全地获取凭证信息
        let (env_token, env_username, env_password) = crate::env_config::get_secure_influxdb_config()?;
        
        // 优先使用环境变量中的凭证
        if let Some(token) = env_token {
            config.token = Some(token);
        }
        if let Some(username) = env_username {
            config.username = Some(username);
        }
        if let Some(password) = env_password {
            config.password = Some(password);
        }
        
        // 从环境变量获取其他配置（如果存在）
        if let Some(url) = crate::env_config::EnvConfig::get_influxdb_url() {
            config.url = url;
        }
        if let Some(database) = crate::env_config::EnvConfig::get_influxdb_database() {
            config.database = database;
        }
        
        // 创建 HTTP 客户端
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| QuantumLogError::NetworkError(format!("Failed to create HTTP client: {}", e)))?;

        // 构建 URL
        let url = if config.use_https {
            if !config.url.starts_with("https://") {
                format!("https://{}", config.url)
            } else {
                config.url.clone()
            }
        } else {
            if !config.url.starts_with("http://") {
                format!("http://{}", config.url)
            } else {
                config.url.clone()
            }
        };

        // 构建写入路径
        let write_path = if config.token.is_some() {
            // InfluxDB 2.x
            let org = crate::env_config::EnvConfig::get_influxdb_org().unwrap_or_else(|| "default".to_string());
            let bucket = crate::env_config::EnvConfig::get_influxdb_bucket().unwrap_or_else(|| config.database.clone());
            format!("/api/v2/write?bucket={}&org={}", bucket, org)
        } else {
            // InfluxDB 1.x
            format!("/write?db={}", config.database)
        };

        // 构建认证头
        let auth_header = if let Some(token) = &config.token {
            // InfluxDB 2.x token 认证
            Some(format!("Token {}", token))
        } else if let (Some(username), Some(password)) = (&config.username, &config.password) {
            // InfluxDB 1.x 基本认证
            let credentials = format!("{}:{}", username, password);
            let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
            Some(format!("Basic {}", encoded))
        } else {
            None
        };

        Ok(Self {
            config,
            client,
            url,
            write_path,
            auth_header,
        })
    }

    /// 运行处理器
    async fn run(self, mut receiver: mpsc::UnboundedReceiver<QuantumLogEvent>) -> Result<()> {
        let mut batch = LogBatch::new();
        let mut flush_interval = interval(Duration::from_secs(self.config.flush_interval_seconds));

        info!("InfluxDB processor started");

        loop {
            tokio::select! {
                // 接收新的日志事件
                event = receiver.recv() => {
                    match event {
                        Some(log_event) => {
                            let entry = self.convert_event_to_entry(&log_event);
                            batch.add_entry(entry);

                            // 检查是否需要刷新批次
                            if batch.is_full(self.config.batch_size) {
                                if let Err(e) = self.flush_batch(&mut batch).await {
                                    error!("Failed to flush InfluxDB batch: {}", e);
                                }
                            }
                        },
                        None => {
                            debug!("InfluxDB event channel closed");
                            // 刷新剩余的批次
                            if !batch.is_empty() {
                                if let Err(e) = self.flush_batch(&mut batch).await {
                                    error!("Failed to flush final InfluxDB batch: {}", e);
                                }
                            }
                            break;
                        }
                    }
                },

                // 定期刷新检查
                _ = flush_interval.tick() => {
                    if !batch.is_empty() && (batch.is_expired(5) || batch.is_full(self.config.batch_size)) {
                        if let Err(e) = self.flush_batch(&mut batch).await {
                            error!("Failed to periodically flush InfluxDB batch: {}", e);
                        }
                    }
                }
            }
        }

        info!("InfluxDB processor stopped");
        Ok(())
    }

    /// 将 QuantumLogEvent 转换为 InfluxLogEntry
    fn convert_event_to_entry(&self, event: &QuantumLogEvent) -> InfluxLogEntry {
        InfluxLogEntry {
            timestamp: event.timestamp,
            level: event.level.clone(),
            target: event.target.clone(),
            message: event.message.clone(),
            file: event.file.clone(),
            line: event.line,
            module_path: event.module_path.clone(),
            thread_name: event.thread_name.clone(),
            _thread_id: event.thread_id.clone(),
            _pid: event.context.pid,
            hostname: event.context.hostname.clone(),
            username: event.context.username.clone(),
            mpi_rank: event.context.mpi_rank,
            fields: event.fields.clone(),
        }
    }

    /// 刷新批次到 InfluxDB
    async fn flush_batch(&self, batch: &mut LogBatch) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        // 将批次转换为 InfluxDB 行协议格式
        let line_protocol_data = self.format_batch_as_line_protocol(batch)?;

        // 发送数据到 InfluxDB
        self.send_to_influxdb(&line_protocol_data).await?;

        debug!("Successfully wrote {} log entries to InfluxDB", batch.len());
        
        if let Some(diagnostics) = crate::diagnostics::get_diagnostics_instance() {
            diagnostics.add_events_processed(batch.len() as u64);
        }

        batch.clear();
        Ok(())
    }

    /// 将批次格式化为 InfluxDB 行协议
    fn format_batch_as_line_protocol(&self, batch: &LogBatch) -> Result<String> {
        let mut lines = Vec::new();

        for entry in batch.entries() {
            // 测量名称（使用 target 作为测量名称，替换非法字符）
            let measurement = entry.target.replace(",", "\\,").replace(" ", "\\ ").replace("=", "\\=");

            // 标签（tag）
            let mut tags = Vec::new();
            tags.push(format!("level={}", Self::escape_string(&entry.level)));
            
            if let Some(ref hostname) = entry.hostname {
                tags.push(format!("hostname={}", Self::escape_string(hostname)));
            }
            
            if let Some(ref thread_name) = entry.thread_name {
                tags.push(format!("thread={}", Self::escape_string(thread_name)));
            }
            
            if let Some(mpi_rank) = entry.mpi_rank {
                tags.push(format!("mpi_rank={}", mpi_rank));
            }

            // 字段（field）
            let mut fields = Vec::new();
            fields.push(format!("message=\"{}\"", Self::escape_string(&entry.message)));
            
            if let Some(ref file) = entry.file {
                fields.push(format!("file=\"{}\"", Self::escape_string(file)));
            }
            
            if let Some(line) = entry.line {
                fields.push(format!("line={}", line));
            }
            
            if let Some(ref module_path) = entry.module_path {
                fields.push(format!("module=\"{}\"", Self::escape_string(module_path)));
            }
            
            if let Some(ref username) = entry.username {
                fields.push(format!("username=\"{}\"", Self::escape_string(username)));
            }
            
            // 添加自定义字段
            for (key, value) in &entry.fields {
                let escaped_key = Self::escape_string(key);
                let field_str = match value {
                    serde_json::Value::String(s) => format!("{}=\"{}\"", escaped_key, Self::escape_string(s)),
                    serde_json::Value::Number(n) => format!("{}={}", escaped_key, n),
                    serde_json::Value::Bool(b) => format!("{}={}", escaped_key, b),
                    _ => format!("{}=\"{}\"", escaped_key, Self::escape_string(&value.to_string())),
                };
                fields.push(field_str);
            }

            // 时间戳（纳秒）
            let timestamp_nanos = entry.timestamp.timestamp_nanos_opt().unwrap_or_else(|| {
                // 如果timestamp_nanos_opt()返回None（在某些平台上），使用timestamp_nanos()
                entry.timestamp.timestamp_nanos_opt().unwrap_or(0)
            });

            // 组合成行协议格式
            let line = format!(
                "{},{} {} {}",
                measurement,
                tags.join(","),
                fields.join(","),
                timestamp_nanos
            );
            
            lines.push(line);
        }

        Ok(lines.join("\n"))
    }

    /// 转义字符串以符合 InfluxDB 行协议要求
    fn escape_string(s: &str) -> String {
        s.replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t")
            .replace(",", "\\,")
            .replace(" ", "\\ ")
            .replace("=", "\\=")
    }

    /// 发送数据到 InfluxDB
    async fn send_to_influxdb(&self, data: &str) -> Result<()> {
        let url = format!("{}{}", self.url, self.write_path);
        
        let mut request = self.client.post(&url).body(data.to_string());
        
        // 添加认证头
        if let Some(ref auth_header) = self.auth_header {
            request = request.header("Authorization", auth_header);
        }
        
        // 添加内容类型头
        request = request.header("Content-Type", "text/plain; charset=utf-8");

        let response = request
            .send()
            .await
            .map_err(|e| QuantumLogError::NetworkError(format!("Failed to send data to InfluxDB: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(QuantumLogError::NetworkError(format!(
                "InfluxDB write failed with status {}: {}",
                status, error_text
            )));
        }

        Ok(())
    }
}

// 实现新的统一 Sink trait
#[async_trait]
impl QuantumSink for InfluxDBSink {
    type Config = InfluxDBConfig;
    type Error = SinkError;

    async fn send_event(&self, event: QuantumLogEvent) -> std::result::Result<(), Self::Error> {
        self.send_event(event).await.map_err(|e| match e {
            QuantumLogError::ChannelError(msg) => SinkError::Generic(msg),
            QuantumLogError::ConfigError(msg) => SinkError::Config(msg),
            QuantumLogError::IoError { source } => SinkError::Io(source),
            QuantumLogError::NetworkError(msg) => SinkError::Network(msg),
            _ => SinkError::Generic(e.to_string()),
        })
    }

    async fn shutdown(&self) -> std::result::Result<(), Self::Error> {
        self.shutdown().await.map_err(|e| match e {
            QuantumLogError::ChannelError(msg) => SinkError::Generic(msg),
            QuantumLogError::ConfigError(msg) => SinkError::Config(msg),
            QuantumLogError::IoError { source } => SinkError::Io(source),
            QuantumLogError::NetworkError(msg) => SinkError::Network(msg),
            _ => SinkError::Generic(e.to_string()),
        })
    }

    async fn is_healthy(&self) -> bool {
        self.is_running().await
    }

    fn name(&self) -> &'static str {
        "influxdb"
    }

    fn stats(&self) -> String {
        format!(
            "InfluxDBSink: running={}, url={}, database={}",
            futures::executor::block_on(self.is_running()),
            self.config.url,
            self.config.database
        )
    }

    fn metadata(&self) -> SinkMetadata {
        SinkMetadata {
            name: "influxdb".to_string(),
            sink_type: SinkType::Exclusive,
            enabled: futures::executor::block_on(self.is_running()),
            description: Some(format!(
                "InfluxDB sink writing to {} database '{}'",
                self.config.url, self.config.database
            )),
        }
    }
}

// 标记 InfluxDBSink 为独占型 Sink
impl ExclusiveSink for InfluxDBSink {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::ContextInfo;
    use tracing::Level;

    fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
        static CALLSITE: tracing::callsite::DefaultCallsite =
            tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
                "test",
                "quantum_log::influxdb::test",
                Level::INFO,
                Some(file!()),
                Some(line!()),
                Some(module_path!()),
                tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
                tracing::metadata::Kind::EVENT,
            ));
        let metadata = tracing::Metadata::new(
            "test",
            "test_target",
            level,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        QuantumLogEvent::new(
            level,
            message.to_string(),
            &metadata,
            std::collections::HashMap::new(),
            ContextInfo::default(),
        )
    }

    #[test]
    fn test_log_batch_operations() {
        let mut batch = LogBatch::new();
        assert!(batch.is_empty());

        let entry = InfluxLogEntry {
            timestamp: chrono::Utc::now(),
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "Test message".to_string(),
            file: Some("test.rs".to_string()),
            line: Some(42),
            module_path: Some("test_module".to_string()),
            thread_name: Some("main".to_string()),
            _thread_id: "thread-1".to_string(),
            _pid: 1234,
            hostname: Some("localhost".to_string()),
            username: Some("testuser".to_string()),
            mpi_rank: Some(0),
            fields: std::collections::HashMap::new(),
        };

        batch.add_entry(entry);
        assert!(!batch.is_empty());
        assert_eq!(batch.len(), 1);

        batch.clear();
        assert!(batch.is_empty());
    }

    #[tokio::test]
    async fn test_influxdb_sink_creation() {
        use crate::env_config::EnvConfig;
        
        // 使用环境变量或默认测试值
        let url = EnvConfig::get_influxdb_url()
            .unwrap_or_else(|| "http://localhost:8086".to_string());
        let username = EnvConfig::get_influxdb_username();
        let password = EnvConfig::get_influxdb_password();
        let token = EnvConfig::get_influxdb_token().unwrap_or(None);
        
        let config = InfluxDBConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            url,
            database: "test_db".to_string(),
            token,
            username,
            password,
            batch_size: 100,
            flush_interval_seconds: 5,
            use_https: false,
            verify_ssl: true,
        };

        let sink = InfluxDBSink::new(config);
        assert!(!futures::executor::block_on(sink.is_running()));
    }

    #[tokio::test]
    async fn test_influxdb_integration() {
        use crate::env_config::EnvConfig;
        
        // 只有在设置了环境变量时才运行集成测试
        if EnvConfig::get_influxdb_token().unwrap_or(None).is_none() {
            println!("Skipping InfluxDB integration test - no token configured");
            return;
        }
        
        let url = EnvConfig::get_influxdb_url()
            .unwrap_or_else(|| "http://localhost:8086".to_string());
        let bucket = EnvConfig::get_influxdb_bucket()
            .unwrap_or_else(|| "test".to_string());
        let token = EnvConfig::get_influxdb_token().unwrap_or(None);
        
        let config = InfluxDBConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            url,
            database: bucket,
            token,
            username: None,
            password: None,
            batch_size: 1,
            flush_interval_seconds: 1,
            use_https: false,
            verify_ssl: true,
        };

        let mut sink = InfluxDBSink::new(config);
        
        // 启动sink
        if let Err(e) = sink.start().await {
            println!("Failed to start InfluxDB sink: {}", e);
            return;
        }
        
        // 等待一下确保sink启动
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // 创建测试事件
        let test_event = create_test_event(Level::INFO, "InfluxDB integration test message");
        
        // 发送事件
        if let Err(e) = sink.send_event(test_event).await {
            println!("Failed to send event to InfluxDB: {}", e);
        } else {
            println!("Successfully sent test event to InfluxDB");
        }
        
        // 等待数据刷新
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // 关闭sink
        let _ = sink.shutdown().await;
    }
}