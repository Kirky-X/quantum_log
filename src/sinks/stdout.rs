use crate::config::StdoutConfig;
use crate::core::event::{ContextInfo, QuantumLogEvent};
use crate::error::QuantumLogError;
use tracing::Level;

/// A sink that outputs log events to stdout
#[derive(Debug, Clone)]
pub struct StdoutSink {
    /// Whether to use colored output
    colored: bool,
    /// Whether to include context information
    include_context: bool,
}

impl StdoutSink {
    /// Creates a new stdout sink
    pub fn new(config: crate::config::StdoutConfig) -> Self {
        Self {
            colored: config.color_enabled.unwrap_or(true),
            include_context: true,
        }
    }

    /// Creates a new stdout sink with custom settings
    pub fn with_options(colored: bool, include_context: bool) -> Self {
        Self {
            colored,
            include_context,
        }
    }

    /// Sends an event to the stdout sink
    pub async fn send_event(
        &self,
        event: QuantumLogEvent,
        _strategy: &crate::config::BackpressureStrategy,
    ) -> Result<(), QuantumLogError> {
        let formatted = self.format_event(&event);
        println!("{}", formatted);
        Ok(())
    }

    /// Shuts down the stdout sink
    pub async fn shutdown(self) -> Result<(), QuantumLogError> {
        // Stdout sink doesn't need special shutdown logic
        Ok(())
    }

    /// Formats the event for output
    fn format_event(&self, event: &QuantumLogEvent) -> String {
        let timestamp = event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f");
        let level_enum = event.level.parse().unwrap_or(Level::INFO);
        let level = self.format_level(&level_enum);
        let target = &event.target;
        let message = &event.message;

        let mut output = format!("[{}] {} {}: {}", timestamp, level, target, message);

        // Add fields if present
        if !event.fields.is_empty() {
            let fields_str = event
                .fields
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" ");
            output.push_str(&format!(" {}", fields_str));
        }

        // Add context if enabled
        if self.include_context {
            let context_str = self.format_context(&event.context);
            if !context_str.is_empty() {
                output.push_str(&format!(" [{}]", context_str));
            }
        }

        output
    }

    /// Formats the log level with optional coloring
    fn format_level(&self, level: &Level) -> String {
        if self.colored {
            match *level {
                Level::ERROR => "\x1b[31mERROR\x1b[0m".to_string(),
                Level::WARN => "\x1b[33mWARN\x1b[0m".to_string(),
                Level::INFO => "\x1b[32mINFO\x1b[0m".to_string(),
                Level::DEBUG => "\x1b[36mDEBUG\x1b[0m".to_string(),
                Level::TRACE => "\x1b[37mTRACE\x1b[0m".to_string(),
            }
        } else {
            format!("{}", level)
        }
    }

    /// Formats context information
    fn format_context(&self, context: &ContextInfo) -> String {
        let mut parts = Vec::new();

        parts.push(format!("pid:{}", context.pid));
        parts.push(format!("tid:{}", context.tid));

        if let Some(ref username) = context.username {
            parts.push(format!("user:{}", username));
        }

        if let Some(ref hostname) = context.hostname {
            parts.push(format!("host:{}", hostname));
        }

        if let Some(rank) = context.mpi_rank {
            parts.push(format!("rank:{}", rank));
        }

        parts.join(",")
    }
}

impl Default for StdoutSink {
    fn default() -> Self {
        Self::new(StdoutConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_event() -> QuantumLogEvent {
        static CALLSITE: tracing::callsite::DefaultCallsite =
            tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
                "test",
                "quantum_log::stdout::test",
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
            Level::INFO,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
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

        QuantumLogEvent {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            target: "test_target".to_string(),
            message: "Test message".to_string(),
            fields: HashMap::new(),
            file: Some("test.rs".to_string()),
            line: Some(42),
            module_path: Some("test::module".to_string()),
            context: context,
        }
    }

    #[test]
    fn test_stdout_sink_creation() {
        let sink = StdoutSink::new(crate::config::StdoutConfig::default());
        assert!(sink.colored);
        assert!(sink.include_context);
    }

    #[test]
    fn test_stdout_sink_with_options() {
        let sink = StdoutSink::with_options(false, false);
        assert!(!sink.colored);
        assert!(!sink.include_context);
    }

    #[test]
    fn test_format_level_colored() {
        let sink = StdoutSink::new(crate::config::StdoutConfig::default());
        assert!(sink.format_level(&Level::ERROR).contains("ERROR"));
        assert!(sink.format_level(&Level::WARN).contains("WARN"));
        assert!(sink.format_level(&Level::INFO).contains("INFO"));
        assert!(sink.format_level(&Level::DEBUG).contains("DEBUG"));
        assert!(sink.format_level(&Level::TRACE).contains("TRACE"));
    }

    #[test]
    fn test_format_level_no_color() {
        let sink = StdoutSink::with_options(false, true);
        assert_eq!(sink.format_level(&Level::INFO), "INFO");
    }

    #[test]
    fn test_format_context() {
        let sink = StdoutSink::new(crate::config::StdoutConfig::default());
        let context = ContextInfo {
            pid: 1234,
            tid: 5678,
            username: Some("test_user".to_string()),
            hostname: Some("test_host".to_string()),
            mpi_rank: Some(0),
            custom_fields: HashMap::new(),
        };

        let formatted = sink.format_context(&context);
        assert!(formatted.contains("pid:1234"));
        assert!(formatted.contains("tid:5678"));
        assert!(formatted.contains("user:test_user"));
        assert!(formatted.contains("host:test_host"));
        assert!(formatted.contains("rank:0"));
    }

    #[test]
    fn test_format_event() {
        let sink = StdoutSink::new(crate::config::StdoutConfig::default());
        let event = create_test_event();
        let formatted = sink.format_event(&event);

        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("test_target"));
        assert!(formatted.contains("Test message"));
        assert!(formatted.contains("pid:1234"));
    }

    #[test]
    fn test_format_event_no_context() {
        let sink = StdoutSink::with_options(true, false);
        let event = create_test_event();
        let formatted = sink.format_event(&event);

        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("test_target"));
        assert!(formatted.contains("Test message"));
        assert!(!formatted.contains("pid:1234"));
    }

    #[tokio::test]
    async fn test_send_event() {
        let sink = StdoutSink::new(crate::config::StdoutConfig {
            enabled: true,
            level: None,
            color_enabled: Some(true),
            level_colors: None,
            format: crate::config::OutputFormat::Text,
            colored: true,
        });
        let event = create_test_event();
        let strategy = crate::config::BackpressureStrategy::Block;
        let result = sink.send_event(event, &strategy).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let sink = StdoutSink::new(crate::config::StdoutConfig {
            enabled: true,
            level: None,
            color_enabled: Some(true),
            level_colors: None,
            format: crate::config::OutputFormat::Text,
            colored: true,
        });
        let result = sink.shutdown().await;
        assert!(result.is_ok());
    }
}
