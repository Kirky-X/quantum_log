//! 控制台输出 Sink 实现
//!
//! 提供将日志事件输出到标准输出或标准错误的功能，支持彩色输出和级别过滤。

use crate::config::StdoutConfig;
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};

use std::io::{self, Write};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::Level;

/// 控制台 Sink 消息类型
#[derive(Debug)]
enum SinkMessage {
    /// 日志事件
    Event(Box<QuantumLogEvent>),
    /// 关闭信号
    Shutdown(oneshot::Sender<Result<()>>),
}

/// 控制台输出 Sink
///
/// 支持将日志事件输出到标准输出或标准错误，提供彩色输出和级别过滤功能。
#[derive(Debug)]
pub struct ConsoleSink {
    config: StdoutConfig,
    sender: Option<mpsc::UnboundedSender<SinkMessage>>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl ConsoleSink {
    /// 创建新的控制台 Sink
    pub fn new(config: StdoutConfig) -> Self {
        Self {
            config,
            sender: None,
            handle: None,
        }
    }

    /// 设置级别过滤器
    pub fn with_level_filter(mut self, level: Level) -> Self {
        self.config.level = Some(level.to_string().to_lowercase());
        self
    }

    /// 启动 Sink
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running() {
            return Err(QuantumLogError::ConfigError(
                "ConsoleSink is already running".to_string(),
            ));
        }

        let (sender, receiver) = mpsc::unbounded_channel();
        let processor = ConsoleSinkProcessor::new(self.config.clone(), receiver)?;

        let handle = tokio::spawn(async move { processor.run().await });

        self.sender = Some(sender);
        self.handle = Some(handle);

        tracing::info!("ConsoleSink started");
        Ok(())
    }

    /// 发送事件（阻塞）
    pub async fn send_event(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(ref sender) = self.sender {
            sender
                .send(SinkMessage::Event(Box::new(event)))
                .map_err(|_| {
                    QuantumLogError::ChannelError("Failed to send event to ConsoleSink".to_string())
                })?;
            Ok(())
        } else {
            Err(QuantumLogError::ConfigError(
                "ConsoleSink is not running".to_string(),
            ))
        }
    }

    /// 尝试发送事件（非阻塞）
    pub fn try_send_event(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(ref sender) = self.sender {
            sender
                .send(SinkMessage::Event(Box::new(event)))
                .map_err(|_| {
                    QuantumLogError::ChannelError("Failed to send event to ConsoleSink".to_string())
                })?;
            Ok(())
        } else {
            Err(QuantumLogError::ConfigError(
                "ConsoleSink is not running".to_string(),
            ))
        }
    }

    /// 关闭 Sink
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(sender) = self.sender.take() {
            let (response_sender, response_receiver) = oneshot::channel();

            // 发送关闭信号
            sender
                .send(SinkMessage::Shutdown(response_sender))
                .map_err(|_| {
                    QuantumLogError::ChannelError("Failed to send shutdown signal".to_string())
                })?;

            // 等待处理器完成
            let result = response_receiver.await.map_err(|_| {
                QuantumLogError::ChannelError("Failed to receive shutdown response".to_string())
            })?;

            // 等待任务完成
            if let Some(handle) = self.handle.take() {
                let _ = handle.await;
            }

            tracing::info!("ConsoleSink shutdown completed");
            result
        } else {
            Ok(())
        }
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.sender.is_some() && self.handle.is_some()
    }

    /// 获取配置
    pub fn config(&self) -> &StdoutConfig {
        &self.config
    }
}

/// 控制台 Sink 处理器
#[derive(Debug)]
struct ConsoleSinkProcessor {
    config: StdoutConfig,
    receiver: mpsc::UnboundedReceiver<SinkMessage>,
    level_filter: Option<Level>,
}

impl ConsoleSinkProcessor {
    /// 创建新的处理器
    fn new(config: StdoutConfig, receiver: mpsc::UnboundedReceiver<SinkMessage>) -> Result<Self> {
        // 解析级别过滤器
        let level_filter = if let Some(ref level_str) = config.level {
            Some(level_str.parse::<Level>().map_err(|_| {
                QuantumLogError::ConfigError(format!("Invalid log level: {}", level_str))
            })?)
        } else {
            None
        };

        Ok(Self {
            config,
            receiver,
            level_filter,
        })
    }

    /// 运行处理器
    async fn run(mut self) -> Result<()> {
        while let Some(message) = self.receiver.recv().await {
            match message {
                SinkMessage::Event(event) => {
                    if let Err(e) = self.handle_event(*event) {
                        tracing::error!("Error handling event in ConsoleSink: {}", e);
                    }
                }
                SinkMessage::Shutdown(response) => {
                    let result = self.shutdown();
                    let _ = response.send(result);
                    break;
                }
            }
        }
        Ok(())
    }

    /// 处理事件
    fn handle_event(&self, event: QuantumLogEvent) -> Result<()> {
        // 检查级别过滤
        if let Some(ref filter_level) = self.level_filter {
            let event_level = event.level.parse::<Level>().map_err(|_| {
                QuantumLogError::ConfigError(format!("Invalid log level: {}", event.level))
            })?;

            if event_level < *filter_level {
                return Ok(());
            }
        }

        // 格式化事件
        let formatted = self.format_event(&event)?;

        // 输出到控制台
        if self.should_use_stderr(&event) {
            let mut stderr = io::stderr();
            stderr
                .write_all(formatted.as_bytes())
                .map_err(|e| QuantumLogError::IoError { source: e })?;
            stderr
                .write_all(b"\n")
                .map_err(|e| QuantumLogError::IoError { source: e })?;
            stderr
                .flush()
                .map_err(|e| QuantumLogError::IoError { source: e })?;
        } else {
            let mut stdout = io::stdout();
            stdout
                .write_all(formatted.as_bytes())
                .map_err(|e| QuantumLogError::IoError { source: e })?;
            stdout
                .write_all(b"\n")
                .map_err(|e| QuantumLogError::IoError { source: e })?;
            stdout
                .flush()
                .map_err(|e| QuantumLogError::IoError { source: e })?;
        }

        Ok(())
    }

    /// 判断是否应该使用标准错误输出
    fn should_use_stderr(&self, event: &QuantumLogEvent) -> bool {
        // 错误和警告级别默认使用 stderr
        matches!(event.level.as_str(), "error" | "warn")
    }

    /// 格式化事件
    fn format_event(&self, event: &QuantumLogEvent) -> Result<String> {
        match &self.config.format {
            crate::config::OutputFormat::Text => {
                if self.config.color_enabled.unwrap_or(true) {
                    Ok(self.format_colored_text(event))
                } else {
                    Ok(event.to_formatted_string("full"))
                }
            }
            crate::config::OutputFormat::Json => event
                .to_json()
                .map_err(|e| QuantumLogError::SerializationError { source: e }),
            crate::config::OutputFormat::Csv => {
                let csv_row = event.to_csv_row();
                Ok(csv_row.join(","))
            }
        }
    }

    /// 格式化彩色文本
    fn format_colored_text(&self, event: &QuantumLogEvent) -> String {
        let level_color = match event.level.as_str() {
            "error" => "\x1b[31m", // 红色
            "warn" => "\x1b[33m",  // 黄色
            "info" => "\x1b[32m",  // 绿色
            "debug" => "\x1b[36m", // 青色
            "trace" => "\x1b[37m", // 白色
            _ => "\x1b[0m",        // 默认
        };

        let reset = "\x1b[0m";

        format!(
            "{}{:>5}{} {} [{}] {}",
            level_color,
            event.level.to_uppercase(),
            reset,
            event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            event.target,
            event.message
        )
    }

    /// 关闭处理器
    fn shutdown(&self) -> Result<()> {
        // 刷新输出流
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();

        tracing::info!("ConsoleSink shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LogLevel, OutputFormat};
    use crate::core::event::ContextInfo;
    use tracing::Level;

    fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
        static CALLSITE: tracing::callsite::DefaultCallsite =
            tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
                "test",
                "quantum_log::console::test",
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

    #[tokio::test]
    async fn test_console_sink_creation() {
        let config = StdoutConfig {
            enabled: true,
            level: None,
            colored: true,
            format: OutputFormat::Text,
            color_enabled: Some(true),
            level_colors: None,
        };

        let sink = ConsoleSink::new(config);
        assert!(!sink.is_running());
    }

    #[tokio::test]
    async fn test_console_sink_start_stop() {
        let config = StdoutConfig {
            enabled: true,
            level: None,
            format: OutputFormat::Text,
            color_enabled: Some(false),
            level_colors: None,
            colored: false,
        };

        let mut sink = ConsoleSink::new(config);

        // 启动
        let result = sink.start().await;
        assert!(result.is_ok());
        assert!(sink.is_running());

        // 关闭
        let result = sink.shutdown().await;
        assert!(result.is_ok());
        assert!(!sink.is_running());
    }

    #[tokio::test]
    async fn test_console_sink_send_event() {
        let config = StdoutConfig {
            enabled: true,
            level: None,
            format: OutputFormat::Text,
            color_enabled: Some(false),
            level_colors: None,
            colored: false,
        };

        let mut sink = ConsoleSink::new(config);
        sink.start().await.unwrap();

        // 发送事件
        let event = create_test_event(Level::INFO, "Test console message");
        let result = sink.send_event(event).await;
        assert!(result.is_ok());

        // 等待一下让事件被处理
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        sink.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_console_sink_try_send_event() {
        let config = StdoutConfig {
            enabled: true,
            level: None,
            format: OutputFormat::Text,
            color_enabled: Some(false),
            level_colors: None,
            colored: false,
        };

        let mut sink = ConsoleSink::new(config);
        sink.start().await.unwrap();

        // 尝试发送事件
        let event = create_test_event(Level::INFO, "Test console message");
        let result = sink.try_send_event(event);
        assert!(result.is_ok());

        // 等待一下让事件被处理
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        sink.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_console_sink_level_filter() {
        let config = StdoutConfig {
            enabled: true,
            level: Some("warn".to_string()),
            colored: false,
            format: OutputFormat::Text,
            color_enabled: Some(false),
            level_colors: None,
        };

        let mut sink = ConsoleSink::new(config);
        sink.start().await.unwrap();

        // 发送低级别事件（应该被过滤）
        let debug_event = create_test_event(Level::DEBUG, "Debug message");
        let result = sink.send_event(debug_event).await;
        assert!(result.is_ok());

        // 发送高级别事件（应该被处理）
        let error_event = create_test_event(Level::ERROR, "Error message");
        let result = sink.send_event(error_event).await;
        assert!(result.is_ok());

        // 等待一下让事件被处理
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        sink.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_console_sink_colored_output() {
        let config = StdoutConfig {
            enabled: true,
            level: None,
            colored: true,
            format: OutputFormat::Text,
            color_enabled: Some(true),
            level_colors: None,
        };

        let mut sink = ConsoleSink::new(config);
        sink.start().await.unwrap();

        // 发送不同级别的事件
        let events = vec![
            create_test_event(Level::ERROR, "Error message"),
            create_test_event(Level::WARN, "Warning message"),
            create_test_event(Level::INFO, "Info message"),
            create_test_event(Level::DEBUG, "Debug message"),
            create_test_event(Level::TRACE, "Trace message"),
        ];

        for event in events {
            let result = sink.send_event(event).await;
            assert!(result.is_ok());
        }

        // 等待一下让事件被处理
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        sink.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_console_sink_json_format() {
        let config = StdoutConfig {
            enabled: true,
            level: None,
            colored: false,
            format: OutputFormat::Json,
            color_enabled: Some(false),
            level_colors: None,
        };

        let mut sink = ConsoleSink::new(config);
        sink.start().await.unwrap();

        // 发送事件
        let event = create_test_event(Level::INFO, "Test JSON message");
        let result = sink.send_event(event).await;
        assert!(result.is_ok());

        // 等待一下让事件被处理
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        sink.shutdown().await.unwrap();
    }

    #[test]
    fn test_console_sink_config() {
        let config = StdoutConfig {
            enabled: true,
            level: Some("info".to_string()),
            colored: true,
            format: OutputFormat::Json,
            color_enabled: Some(true),
            level_colors: None,
        };

        let sink = ConsoleSink::new(config.clone());

        // 检查配置
        assert_eq!(sink.config().enabled, config.enabled);
        assert_eq!(sink.config().level, config.level);
        assert_eq!(sink.config().colored, config.colored);
        assert_eq!(sink.config().format, config.format);
        assert_eq!(sink.config().color_enabled, config.color_enabled);
    }
}
