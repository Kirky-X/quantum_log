# QuantumLog

[![Crates.io](https://img.shields.io/crates/v/quantum_log.svg)](https://crates.io/crates/quantum_log)
[![Documentation](https://docs.rs/quantum_log/badge.svg)](https://docs.rs/quantum_log)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://github.com/Kirky-X/quantum_log/actions/workflows/rust.yml/badge.svg)](https://github.com/Kirky-X/quantum_log/actions/workflows/rust.yml)

**[中文](README.md)** | **[文档](https://docs.rs/lingo)**

**QuantumLog** is an asynchronous logging library designed for high-performance computing environments, supporting multiple output formats and targets including files, databases, and standard output. It provides powerful configuration options, graceful shutdown mechanisms, and detailed diagnostic information.

## 🚀 Features

- **Asynchronous High Performance**: Tokio-based asynchronous architecture supporting high-concurrency logging
- **Multiple Output Targets**: Support for stdout, files, databases, and other output methods
- **Flexible Configuration**: Support for TOML configuration files and programmatic configuration
- **Graceful Shutdown**: Complete shutdown mechanism ensuring no log loss
- **Diagnostic Information**: Built-in diagnostic system for monitoring logging system performance
- **MPI Support**: Optimized for high-performance computing environments with MPI support
- **Backpressure Handling**: Intelligent handling of log backpressure under high load
- **Structured Logging**: Support for structured logging and multiple output formats

## 📦 Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
quantum_log = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"

# Optional features
[dependencies.quantum_log]
version = "0.1.0"
features = ["database", "mpi"]  # Enable database and MPI support
```

## 🎯 Quick Start

### Basic Usage

```rust
use quantum_log::{init, shutdown};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize QuantumLog
    init().await?;
    
    // Use standard tracing macros
    info!("Application started");
    warn!("This is a warning");
    error!("This is an error");
    
    // Graceful shutdown
    shutdown().await?;
    Ok(())
}
```

### Using Design Document Recommended API

```rust
use quantum_log::init_quantum_logger;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the main API from design document
    let shutdown_handle = init_quantum_logger().await?;
    
    info!("Logging with QuantumLog");
    warn!("Warning message");
    error!("Error message");
    
    // Use returned handle for graceful shutdown
    shutdown_handle.shutdown().await?;
    Ok(())
}
```

## 🆕 Version 0.2.0 New Features

### Unified Sink Trait System

QuantumLog 0.2.0 introduces a revolutionary unified Sink trait system that provides unprecedented flexibility and extensibility for log output handling.

#### Core Trait Design

**QuantumSink - The Foundation Trait**

```rust
use quantum_log::sinks::{QuantumSink, SinkError, SinkMetadata};
use quantum_log::core::event::QuantumLogEvent;
use async_trait::async_trait;

#[async_trait]
pub trait QuantumSink: Send + Sync + std::fmt::Debug {
    type Config: Send + Sync;
    type Error: std::error::Error + Send + Sync + 'static;
    
    // Core functionality
    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error>;
    async fn shutdown(&self) -> Result<(), Self::Error>;
    async fn is_healthy(&self) -> bool;
    
    // Metadata and diagnostics
    fn name(&self) -> &'static str;
    fn stats(&self) -> String;
    fn metadata(&self) -> SinkMetadata;
}
```

**Stackable vs Exclusive Sinks**

```rust
// Stackable Sink - Can be combined with others
pub trait StackableSink: QuantumSink {}

// Exclusive Sink - Operates independently
pub trait ExclusiveSink: QuantumSink {}
```

#### Pipeline Management System

**Core Pipeline Features**

```rust
use quantum_log::pipeline::{Pipeline, PipelineConfig, ErrorStrategy};
use std::time::Duration;

// Create and configure pipeline
let config = PipelineConfig {
    parallel_processing: true,
    buffer_size: 1000,
    error_strategy: ErrorStrategy::RetryThenContinue,
    max_retries: 3,
    retry_delay: Duration::from_millis(100),
    health_check_interval: Duration::from_secs(30),
};

let pipeline = Pipeline::with_config(config);
```

**Builder Pattern Configuration**

```rust
use quantum_log::pipeline::PipelineBuilder;

let pipeline = PipelineBuilder::new()
    .with_parallel_processing(true)
    .with_buffer_size(2000)
    .with_error_strategy(ErrorStrategy::LogAndContinue)
    .with_health_check_interval(Duration::from_secs(60))
    .build();
```

#### Error Handling Strategies

```rust
use quantum_log::pipeline::ErrorStrategy;

// Available strategies
let strategies = [
    ErrorStrategy::FailFast,           // Stop on first error
    ErrorStrategy::LogAndContinue,     // Log error and continue
    ErrorStrategy::RetryThenContinue,  // Retry then continue
    ErrorStrategy::RetryThenFail,      // Retry then fail
];
```

#### Health Check Mechanism

```rust
// Check pipeline health
let health = pipeline.health_check().await;
if health.overall_healthy {
    println!("All sinks are healthy");
} else {
    for sink_health in health.sink_details {
        if !sink_health.healthy {
            eprintln!("Sink {} is unhealthy: {:?}", 
                     sink_health.name, sink_health.last_error);
        }
    }
}
```

#### Default Standard Output Sink

```rust
use quantum_log::sinks::DefaultStdoutSink;
use quantum_log::core::level::Level;

// Basic usage
let stdout_sink = DefaultStdoutSink::new();

// With configuration
let stdout_sink = DefaultStdoutSink::with_config(StdoutConfig {
    colored: true,
    format: OutputFormat::Text,
    level_filter: Some(Level::INFO),
    timestamp_format: "%Y-%m-%d %H:%M:%S".to_string(),
});

// Convenience functions
let stdout_sink = DefaultStdoutSink::colored();
let stdout_sink = DefaultStdoutSink::json_format();
let stdout_sink = DefaultStdoutSink::with_level_filter(Level::WARN);
```

### Custom Sink Implementation

#### Stackable Sink Example

```rust
use quantum_log::sinks::{QuantumSink, StackableSink, SinkError, SinkMetadata, SinkType};
use quantum_log::core::event::QuantumLogEvent;
use async_trait::async_trait;

#[derive(Debug)]
struct MetricsSink {
    endpoint: String,
    event_count: std::sync::atomic::AtomicU64,
}

impl MetricsSink {
    fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            event_count: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl QuantumSink for MetricsSink {
    type Config = String;
    type Error = SinkError;
    
    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
        // Send metrics to monitoring system
        self.event_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        println!("Sending metrics to {}: {} - {}", self.endpoint, event.level, event.message);
        Ok(())
    }
    
    async fn shutdown(&self) -> Result<(), Self::Error> {
        println!("Shutting down metrics sink: {}", self.endpoint);
        Ok(())
    }
    
    async fn is_healthy(&self) -> bool {
        true // Check if endpoint is reachable
    }
    
    fn name(&self) -> &'static str {
        "metrics_sink"
    }
    
    fn stats(&self) -> String {
        format!("MetricsSink[{}]: {} events sent", 
                self.endpoint, 
                self.event_count.load(std::sync::atomic::Ordering::Relaxed))
    }
    
    fn metadata(&self) -> SinkMetadata {
        SinkMetadata {
            name: "metrics_sink".to_string(),
            sink_type: SinkType::Network,
            version: "1.0.0".to_string(),
            description: "Metrics collection sink".to_string(),
        }
    }
}

// Mark as stackable
impl StackableSink for MetricsSink {}
```

#### Exclusive Sink Example

```rust
use quantum_log::sinks::{QuantumSink, ExclusiveSink};
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Debug)]
struct CustomFileSink {
    file_path: String,
    writer: std::sync::Mutex<std::fs::File>,
}

impl CustomFileSink {
    async fn new(file_path: String) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
            
        Ok(Self {
            file_path,
            writer: std::sync::Mutex::new(file),
        })
    }
}

#[async_trait]
impl QuantumSink for CustomFileSink {
    type Config = String;
    type Error = SinkError;
    
    async fn send_event(&self, event: QuantumLogEvent) -> Result<(), Self::Error> {
        let formatted = format!("{} [{}] {}\n", 
                               event.timestamp, event.level, event.message);
        
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(formatted.as_bytes())
            .map_err(|e| SinkError::WriteError(e.to_string()))?;
        writer.flush()
            .map_err(|e| SinkError::WriteError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn shutdown(&self) -> Result<(), Self::Error> {
        let mut writer = self.writer.lock().unwrap();
        writer.flush()
            .map_err(|e| SinkError::WriteError(e.to_string()))?;
        Ok(())
    }
    
    async fn is_healthy(&self) -> bool {
        std::path::Path::new(&self.file_path).exists()
    }
    
    fn name(&self) -> &'static str {
        "custom_file_sink"
    }
    
    fn stats(&self) -> String {
        format!("CustomFileSink[{}]", self.file_path)
    }
    
    fn metadata(&self) -> SinkMetadata {
        SinkMetadata {
            name: "custom_file_sink".to_string(),
            sink_type: SinkType::File,
            version: "1.0.0".to_string(),
            description: "Custom file output sink".to_string(),
        }
    }
}

// Mark as exclusive
impl ExclusiveSink for CustomFileSink {}
```

### Best Practices

#### 1. Sink Selection Guide

- **Stackable Sink Use Cases**:
  - Console output
  - Network sending (HTTP, UDP)
  - Metrics collection
  - Notification systems
  - Cache writing

- **Exclusive Sink Use Cases**:
  - File writing
  - Database connections
  - Message queues
  - Operations requiring transactions

#### 2. Error Handling Strategy Selection

```rust
// Production environment recommendation
let config = PipelineConfig {
    error_strategy: ErrorStrategy::RetryThenContinue,
    max_retries: 3,
    retry_delay: Duration::from_millis(100),
    ..Default::default()
};

// Development environment recommendation
let config = PipelineConfig {
    error_strategy: ErrorStrategy::LogAndContinue,
    ..Default::default()
};

// Critical system recommendation
let config = PipelineConfig {
    error_strategy: ErrorStrategy::FailFast,
    ..Default::default()
};
```

#### 3. Performance Optimization Tips

- Use parallel processing to improve throughput
- Set appropriate buffer sizes
- Monitor health status and statistics
- Regular resource cleanup

#### 4. Troubleshooting

```rust
// Check pipeline health
let health = pipeline.health_check().await;
if !health.overall_healthy {
    for sink_health in health.sink_details {
        if !sink_health.healthy {
            eprintln!("Sink {} is unhealthy: {:?}", 
                     sink_health.name, sink_health.last_error);
        }
    }
}

// Get detailed statistics
let stats = pipeline.get_stats().await;
println!("Pipeline stats: {}", stats);
```

### Backward Compatibility

- ✅ Fully backward compatible with existing APIs
- ✅ Existing code runs without modification
- ✅ Progressive migration support
- ✅ Detailed migration guide

### Migration Guide

#### Migrating from 0.1.x to 0.2.0

1. **No code changes required** - All existing APIs remain compatible
2. **Optional upgrade** - Gradually adopt the new Pipeline system
3. **Configuration migration** - Existing config files need no changes

#### Recommended Migration Steps

1. Update dependency version to 0.2.0
2. Run existing tests to ensure compatibility
3. Gradually introduce Pipeline system
4. Leverage new health check and statistics features
5. Consider implementing custom Sinks

## 📖 Detailed Examples

### 1. Custom Configuration

```rust
use quantum_log::{QuantumLogConfig, init_with_config, shutdown};
use tracing::{info, debug, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = QuantumLogConfig {
        global_level: "DEBUG".to_string(),
        pre_init_buffer_size: Some(1000),
        pre_init_stdout_enabled: true,
        ..Default::default()
    };
    
    init_with_config(config).await?;
    
    debug!("Debug messages will now be logged");
    info!("Application configured");
    error!("This is an error");
    
    shutdown().await?;
    Ok(())
}
```

### 2. Using Builder Pattern

```rust
use quantum_log::{init_with_builder, shutdown};
use tracing::{info, span, Level};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_with_builder(|builder| {
        builder
            .max_buffer_size(5000)
            .custom_field("service", "my-app")
            .custom_field("version", "1.0.0")
            .custom_field("environment", "production")
    }).await?;
    
    // Create span for structured logging
    let span = span!(Level::INFO, "user_operation", user_id = 12345);
    let _enter = span.enter();
    
    info!("User operation started");
    info!(action = "login", result = "success", "User login successful");
    
    shutdown().await?;
    Ok(())
}
```

### 3. Loading from Configuration File

First create a configuration file `quantum_log.toml`:

```toml
global_level = "INFO"
pre_init_buffer_size = 1000
pre_init_stdout_enabled = true

[context_fields]
timestamp = true
level = true
target = true
file_line = false
pid = true
tid = false
mpi_rank = false
username = false
hostname = true
span_info = true

[format]
type = "json"
timestamp_format = "%Y-%m-%d %H:%M:%S%.3f"
log_template = "{timestamp} [{level}] {target} - {message}"
json_fields_key = "fields"

[stdout]
enabled = true
level = "INFO"
format = { type = "text" }

[file]
enabled = true
level = "DEBUG"
path = "./logs"
filename_base = "quantum"
max_file_size_mb = 100
max_files = 10
buffer_size = 8192
format = { type = "json" }

[file.rotation]
strategy = "size"
max_size_mb = 50
max_files = 5

[database]
enabled = false
level = "WARN"
connection_string = "postgresql://user:pass@localhost/logs"
table_name = "quantum_logs"
batch_size = 100
pool_size = 5
connection_timeout_ms = 5000
format = { type = "json" }
```

Then use it in code:

```rust
use quantum_log::{load_config_from_file, init_with_config, shutdown};
use tracing::{info, warn, error, debug};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from file
    let config = load_config_from_file("quantum_log.toml").await?;
    
    // Initialize with loaded configuration
    init_with_config(config).await?;
    
    debug!("Debug message");
    info!("Info log");
    warn!("Warning log");
    error!("Error log");
    
    shutdown().await?;
    Ok(())
}
```

### 4. Structured Logging

```rust
use quantum_log::{init, shutdown};
use tracing::{info, warn, error, span, Level};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init().await?;
    
    // Basic structured logging
    info!(user_id = 12345, action = "login", "User login");
    warn!(error_code = 404, path = "/api/users", "API path not found");
    
    // Complex data structures
    let user_data = json!({
        "id": 12345,
        "name": "John Doe",
        "email": "john@example.com",
        "roles": ["user", "admin"]
    });
    info!(user = %user_data, "User data updated");
    
    // Using spans for context tracking
    let request_span = span!(Level::INFO, "http_request", 
        method = "POST", 
        path = "/api/users", 
        request_id = "req-123"
    );
    
    let _enter = request_span.enter();
    info!("Processing HTTP request");
    info!(status = 200, duration_ms = 45, "Request completed");
    
    // Nested spans
    let db_span = span!(Level::DEBUG, "database_query", table = "users");
    let _db_enter = db_span.enter();
    info!(query = "SELECT * FROM users WHERE id = ?", "Executing database query");
    
    shutdown().await?;
    Ok(())
}
```

### 5. Error Handling and Diagnostics

```rust
use quantum_log::{init, shutdown, get_diagnostics, get_buffer_stats, is_initialized};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check initialization status
    assert!(!is_initialized());
    
    init().await?;
    assert!(is_initialized());
    
    // Log some messages
    for i in 0..100 {
        info!(iteration = i, "Processing iteration {}", i);
        if i % 10 == 0 {
            error!(iteration = i, "Simulated error");
        }
    }
    
    // Get buffer statistics
    if let Some(stats) = get_buffer_stats() {
        info!(
            current_size = stats.current_size,
            max_size = stats.max_size,
            dropped_count = stats.dropped_count,
            "Buffer statistics"
        );
    }
    
    // Get diagnostic information
    let diagnostics = get_diagnostics();
    info!(
        events_processed = diagnostics.events_processed,
        events_dropped = diagnostics.events_dropped_backpressure + diagnostics.events_dropped_error,
        sink_errors = diagnostics.sink_errors,
        uptime_seconds = diagnostics.uptime().as_secs(),
        success_rate = format!("{:.2}%", diagnostics.success_rate() * 100.0),
        "Diagnostic information"
    );
    
    shutdown().await?;
    Ok(())
}
```

### 6. Advanced Configuration Example

```rust
use quantum_log::{QuantumLogConfig, init_with_config, shutdown};
use quantum_log::config::*;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = QuantumLogConfig {
        global_level: "DEBUG".to_string(),
        pre_init_buffer_size: Some(2000),
        pre_init_stdout_enabled: true,
        backpressure_strategy: BackpressureStrategy::Drop,
        
        // Configure stdout
        stdout: Some(StdoutConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            format: OutputFormat::Json,
        }),
        
        // Configure file output
        file: Some(FileSinkConfig {
            enabled: true,
            level: Some("DEBUG".to_string()),
            path: "./logs".to_string(),
            filename_base: "quantum".to_string(),
            max_file_size_mb: Some(100),
            max_files: Some(10),
            buffer_size: Some(16384),
            format: OutputFormat::Json,
            rotation: Some(RotationConfig {
                strategy: RotationStrategy::Size,
                max_size_mb: Some(50),
                max_files: Some(5),
                time_pattern: None,
            }),
            writer_cache_ttl_seconds: Some(600),
            writer_cache_capacity: Some(2048),
        }),
        
        // Configure context fields
        context_fields: ContextFieldsConfig {
            timestamp: true,
            level: true,
            target: true,
            file_line: true,
            pid: true,
            tid: true,
            mpi_rank: false,
            username: true,
            hostname: true,
            span_info: true,
        },
        
        // Configure format
        format: LogFormatConfig {
            format_type: LogFormatType::Json,
            timestamp_format: "%Y-%m-%d %H:%M:%S%.6f".to_string(),
            log_template: "{timestamp} [{level}] {target}:{line} - {message}".to_string(),
            json_fields_key: "data".to_string(),
        },
        
        // Database configuration (requires database feature)
        #[cfg(feature = "database")]
        database: Some(DatabaseSinkConfig {
            enabled: true,
            level: Some("WARN".to_string()),
            connection_string: "postgresql://user:pass@localhost/logs".to_string(),
            table_name: "quantum_logs".to_string(),
            batch_size: Some(200),
            pool_size: Some(10),
            connection_timeout_ms: Some(10000),
            format: OutputFormat::Json,
        }),
        
        #[cfg(not(feature = "database"))]
        database: None,
    };
    
    init_with_config(config).await?;
    
    info!("Advanced configuration initialized");
    warn!(component = "auth", "Authentication module warning");
    error!(error_code = "E001", module = "database", "Database connection failed");
    
    shutdown().await?;
    Ok(())
}
```

### 7. MPI Environment Usage (requires mpi feature)

```rust
#[cfg(feature = "mpi")]
use quantum_log::mpi::*;
use quantum_log::{init_with_config, QuantumLogConfig, shutdown};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "mpi")]
    {
        // Initialize MPI (if available)
        if let Ok(_) = init_mpi() {
            let rank = get_mpi_rank().unwrap_or(0);
            let size = get_mpi_size().unwrap_or(1);
            
            let config = QuantumLogConfig {
                global_level: "INFO".to_string(),
                context_fields: quantum_log::config::ContextFieldsConfig {
                    mpi_rank: true,
                    ..Default::default()
                },
                file: Some(quantum_log::config::FileSinkConfig {
                    enabled: true,
                    level: Some("DEBUG".to_string()),
                    path: format!("./logs/rank_{}", rank),
                    filename_base: format!("quantum_rank_{}", rank),
                    ..Default::default()
                }),
                ..Default::default()
            };
            
            init_with_config(config).await?;
            
            info!(rank = rank, size = size, "MPI process started");
            
            // Simulate MPI workload
            for i in 0..10 {
                info!(rank = rank, iteration = i, "Processing data chunk {}", i);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            warn!(rank = rank, "MPI process about to finish");
            
            shutdown().await?;
            finalize_mpi()?;
        } else {
            println!("MPI not available, using standard mode");
            quantum_log::init().await?;
            info!("Standard mode started");
            quantum_log::shutdown().await?;
        }
    }
    
    #[cfg(not(feature = "mpi"))]
    {
        println!("MPI feature not enabled");
        quantum_log::init().await?;
        info!("Standard mode started");
        quantum_log::shutdown().await?;
    }
    
    Ok(())
}
```

### 8. Performance Testing and Benchmarking

```rust
use quantum_log::{init, shutdown, get_diagnostics};
use tracing::{info, warn, error};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init().await?;
    
    let start_time = Instant::now();
    let num_messages = 10000;
    
    info!("Starting performance test, will log {} messages", num_messages);
    
    // High-frequency logging test
    for i in 0..num_messages {
        match i % 4 {
            0 => info!(id = i, "Info log {}", i),
            1 => warn!(id = i, "Warning log {}", i),
            2 => error!(id = i, "Error log {}", i),
            _ => info!(id = i, data = format!("complex_data_{}", i), "Structured log {}", i),
        }
        
        // Pause every 1000 messages to simulate real application
        if i % 1000 == 0 && i > 0 {
            sleep(Duration::from_millis(1)).await;
        }
    }
    
    let elapsed = start_time.elapsed();
    let messages_per_second = num_messages as f64 / elapsed.as_secs_f64();
    
    info!(
        total_messages = num_messages,
        elapsed_ms = elapsed.as_millis(),
        messages_per_second = format!("{:.2}", messages_per_second),
        "Performance test completed"
    );
    
    // Get final diagnostic information
    let diagnostics = get_diagnostics();
    info!(
        events_processed = diagnostics.events_processed,
        events_dropped = diagnostics.events_dropped_backpressure + diagnostics.events_dropped_error,
        success_rate = format!("{:.4}%", diagnostics.success_rate() * 100.0),
        "Final statistics"
    );
    
    shutdown().await?;
    Ok(())
}
```

## 🔧 Configuration Options

### Global Configuration

- `global_level`: Global log level ("TRACE", "DEBUG", "INFO", "WARN", "ERROR")
- `pre_init_buffer_size`: Pre-initialization buffer size
- `pre_init_stdout_enabled`: Whether to enable pre-initialization stdout
- `backpressure_strategy`: Backpressure strategy ("Block" or "Drop")

### Output Target Configuration

#### Standard Output (stdout)
```toml
[stdout]
enabled = true
level = "INFO"
format = { type = "text" }  # or { type = "json" }
```

#### File Output (file)
```toml
[file]
enabled = true
level = "DEBUG"
path = "./logs"
filename_base = "quantum"
max_file_size_mb = 100
max_files = 10
buffer_size = 8192
format = { type = "json" }

[file.rotation]
strategy = "size"  # or "time"
max_size_mb = 50
max_files = 5
```

#### Database Output (database)
```toml
[database]
enabled = true
level = "WARN"
connection_string = "postgresql://user:pass@localhost/logs"
table_name = "quantum_logs"
batch_size = 100
pool_size = 5
connection_timeout_ms = 5000
format = { type = "json" }
```

### Context Fields Configuration
```toml
[context_fields]
timestamp = true
level = true
target = true
file_line = false
pid = true
tid = false
mpi_rank = false
username = false
hostname = true
span_info = true
```

### Format Configuration
```toml
[format]
type = "json"  # or "text"
timestamp_format = "%Y-%m-%d %H:%M:%S%.3f"
log_template = "{timestamp} [{level}] {target} - {message}"
json_fields_key = "fields"
```

## 📊 Diagnostics and Monitoring

QuantumLog provides detailed diagnostic information to monitor logging system performance:

```rust
use quantum_log::get_diagnostics;

let diagnostics = get_diagnostics();
println!("Events processed: {}", diagnostics.events_processed);
println!("Events dropped: {}", diagnostics.events_dropped_backpressure);
println!("Success rate: {:.2}%", diagnostics.success_rate() * 100.0);
println!("Uptime: {:?}", diagnostics.uptime());
```

## 🚨 Error Handling

QuantumLog provides detailed error types and handling mechanisms:

```rust
use quantum_log::{QuantumLogError, Result};

match quantum_log::init().await {
    Ok(_) => println!("Initialization successful"),
    Err(QuantumLogError::InitializationError(msg)) => {
        eprintln!("Initialization failed: {}", msg);
    },
    Err(QuantumLogError::ConfigError(msg)) => {
        eprintln!("Configuration error: {}", msg);
    },
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## 🔄 Graceful Shutdown

QuantumLog supports multiple graceful shutdown methods:

```rust
use quantum_log::shutdown::{ShutdownCoordinator, ShutdownSignal};

// Method 1: Simple shutdown
quantum_log::shutdown().await?;

// Method 2: Using handle shutdown
let handle = quantum_log::init_quantum_logger().await?;
handle.shutdown().await?;

// Method 3: Coordinator managing multiple components
let mut coordinator = ShutdownCoordinator::new();
let handle = quantum_log::init_quantum_logger().await?;
coordinator.register_component("quantum_log", handle);

// Execute coordinated shutdown
coordinator.shutdown_all(ShutdownSignal::Graceful).await?;
```

## 🧪 Testing

Run tests:

```bash
# Run all tests
cargo test

# Run tests for specific features
cargo test --features database
cargo test --features mpi

# Run examples
cargo run --example basic_usage
cargo run --example complete_examples
cargo run --example config_file_example
```

## 📝 License

This project is licensed under the Apache--2.0 License. See the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) to learn how to participate in project development.

## 📞 Support

If you encounter issues or have suggestions, please:

1. Check the [documentation](https://docs.rs/quantum_log)
2. Search or create an [Issue](https://github.com/Kirky-X/quantum_log/issues)
3. Join the [discussion](https://github.com/Kirky-X/quantum_log/discussions)
