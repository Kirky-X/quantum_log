# QuantumLog (量子日志)

[![Crates.io](https://img.shields.io/crates/v/quantum_log.svg)](https://crates.io/crates/quantum_log)
[![Documentation](https://docs.rs/quantum_log/badge.svg)](https://docs.rs/quantum_log)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build Status](https://github.com/your-username/quantum_log/workflows/CI/badge.svg)](https://github.com/your-username/quantum_log/actions)

**QuantumLog** 是一个专为高性能计算环境设计的异步日志库，支持多种输出格式和目标，包括文件、数据库和标准输出。它提供了强大的配置选项、优雅的关闭机制和详细的诊断信息。

## 🚀 特性

- **异步高性能**: 基于 Tokio 的异步架构，支持高并发日志记录
- **多种输出目标**: 支持标准输出、文件、数据库等多种输出方式
- **灵活配置**: 支持 TOML 配置文件和代码配置
- **优雅关闭**: 提供完善的关闭机制，确保日志不丢失
- **诊断信息**: 内置诊断系统，监控日志系统性能
- **MPI 支持**: 专为高性能计算环境优化，支持 MPI 环境
- **背压处理**: 智能处理高负载情况下的日志背压
- **结构化日志**: 支持结构化日志记录和多种输出格式

## 📦 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
quantum_log = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"

# 可选功能
[dependencies.quantum_log]
version = "0.1.0"
features = ["database", "mpi"]  # 启用数据库和 MPI 支持
```

## 🎯 快速开始

### 基本使用

```rust
use quantum_log::{init, shutdown};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 QuantumLog
    init().await?;
    
    // 使用标准的 tracing 宏
    info!("应用程序启动");
    warn!("这是一个警告");
    error!("这是一个错误");
    
    // 优雅关闭
    shutdown().await?;
    Ok(())
}
```

### 使用设计文档推荐的 API

```rust
use quantum_log::init_quantum_logger;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用设计文档中的主要 API
    let shutdown_handle = init_quantum_logger().await?;
    
    info!("使用 QuantumLog 记录日志");
    warn!("警告信息");
    error!("错误信息");
    
    // 使用返回的句柄进行优雅关闭
    shutdown_handle.shutdown().await?;
    Ok(())
}
```

## 📖 详细示例

### 1. 自定义配置

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
    
    debug!("调试信息现在会被记录");
    info!("应用程序配置完成");
    error!("这是一个错误");
    
    shutdown().await?;
    Ok(())
}
```

### 2. 使用构建器模式

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
    
    // 创建 span 进行结构化日志记录
    let span = span!(Level::INFO, "user_operation", user_id = 12345);
    let _enter = span.enter();
    
    info!("用户操作开始");
    info!(action = "login", result = "success", "用户登录成功");
    
    shutdown().await?;
    Ok(())
}
```

### 3. 从配置文件加载

首先创建配置文件 `quantum_log.toml`：

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

然后在代码中使用：

```rust
use quantum_log::{load_config_from_file, init_with_config, shutdown};
use tracing::{info, warn, error, debug};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 从文件加载配置
    let config = load_config_from_file("quantum_log.toml").await?;
    
    // 使用加载的配置初始化
    init_with_config(config).await?;
    
    debug!("调试信息");
    info!("信息日志");
    warn!("警告日志");
    error!("错误日志");
    
    shutdown().await?;
    Ok(())
}
```

### 4. 结构化日志记录

```rust
use quantum_log::{init, shutdown};
use tracing::{info, warn, error, span, Level};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init().await?;
    
    // 基本结构化日志
    info!(user_id = 12345, action = "login", "用户登录");
    warn!(error_code = 404, path = "/api/users", "API 路径未找到");
    
    // 复杂数据结构
    let user_data = json!({
        "id": 12345,
        "name": "张三",
        "email": "zhangsan@example.com",
        "roles": ["user", "admin"]
    });
    info!(user = %user_data, "用户数据已更新");
    
    // 使用 span 进行上下文跟踪
    let request_span = span!(Level::INFO, "http_request", 
        method = "POST", 
        path = "/api/users", 
        request_id = "req-123"
    );
    
    let _enter = request_span.enter();
    info!("处理 HTTP 请求");
    info!(status = 200, duration_ms = 45, "请求处理完成");
    
    // 嵌套 span
    let db_span = span!(Level::DEBUG, "database_query", table = "users");
    let _db_enter = db_span.enter();
    info!(query = "SELECT * FROM users WHERE id = ?", "执行数据库查询");
    
    shutdown().await?;
    Ok(())
}
```

### 5. 错误处理和诊断

```rust
use quantum_log::{init, shutdown, get_diagnostics, get_buffer_stats, is_initialized};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 检查初始化状态
    assert!(!is_initialized());
    
    init().await?;
    assert!(is_initialized());
    
    // 记录一些日志
    for i in 0..100 {
        info!(iteration = i, "处理第 {} 次迭代", i);
        if i % 10 == 0 {
            error!(iteration = i, "模拟错误");
        }
    }
    
    // 获取缓冲区统计信息
    if let Some(stats) = get_buffer_stats() {
        info!(
            current_size = stats.current_size,
            max_size = stats.max_size,
            dropped_count = stats.dropped_count,
            "缓冲区统计信息"
        );
    }
    
    // 获取诊断信息
    let diagnostics = get_diagnostics();
    info!(
        events_processed = diagnostics.events_processed,
        events_dropped = diagnostics.events_dropped_backpressure + diagnostics.events_dropped_error,
        sink_errors = diagnostics.sink_errors,
        uptime_seconds = diagnostics.uptime().as_secs(),
        success_rate = format!("{:.2}%", diagnostics.success_rate() * 100.0),
        "诊断信息"
    );
    
    shutdown().await?;
    Ok(())
}
```

### 6. 高级配置示例

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
        
        // 配置标准输出
        stdout: Some(StdoutConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            format: OutputFormat::Json,
        }),
        
        // 配置文件输出
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
        
        // 配置上下文字段
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
        
        // 配置格式
        format: LogFormatConfig {
            format_type: LogFormatType::Json,
            timestamp_format: "%Y-%m-%d %H:%M:%S%.6f".to_string(),
            log_template: "{timestamp} [{level}] {target}:{line} - {message}".to_string(),
            json_fields_key: "data".to_string(),
        },
        
        // 数据库配置（需要启用 database 功能）
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
    
    info!("高级配置初始化完成");
    warn!(component = "auth", "认证模块警告");
    error!(error_code = "E001", module = "database", "数据库连接失败");
    
    shutdown().await?;
    Ok(())
}
```

### 7. MPI 环境使用（需要启用 mpi 功能）

```rust
#[cfg(feature = "mpi")]
use quantum_log::mpi::*;
use quantum_log::{init_with_config, QuantumLogConfig, shutdown};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "mpi")]
    {
        // 初始化 MPI（如果可用）
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
            
            info!(rank = rank, size = size, "MPI 进程启动");
            
            // 模拟 MPI 工作负载
            for i in 0..10 {
                info!(rank = rank, iteration = i, "处理数据块 {}", i);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            warn!(rank = rank, "MPI 进程即将结束");
            
            shutdown().await?;
            finalize_mpi()?;
        } else {
            println!("MPI 不可用，使用标准模式");
            quantum_log::init().await?;
            info!("标准模式启动");
            quantum_log::shutdown().await?;
        }
    }
    
    #[cfg(not(feature = "mpi"))]
    {
        println!("MPI 功能未启用");
        quantum_log::init().await?;
        info!("标准模式启动");
        quantum_log::shutdown().await?;
    }
    
    Ok(())
}
```

### 8. 性能测试和基准测试

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
    
    info!("开始性能测试，将记录 {} 条日志", num_messages);
    
    // 高频日志记录测试
    for i in 0..num_messages {
        match i % 4 {
            0 => info!(id = i, "信息日志 {}", i),
            1 => warn!(id = i, "警告日志 {}", i),
            2 => error!(id = i, "错误日志 {}", i),
            _ => info!(id = i, data = format!("复杂数据_{}", i), "结构化日志 {}", i),
        }
        
        // 每 1000 条消息暂停一下，模拟真实应用
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
        "性能测试完成"
    );
    
    // 获取最终诊断信息
    let diagnostics = get_diagnostics();
    info!(
        events_processed = diagnostics.events_processed,
        events_dropped = diagnostics.events_dropped_backpressure + diagnostics.events_dropped_error,
        success_rate = format!("{:.4}%", diagnostics.success_rate() * 100.0),
        "最终统计信息"
    );
    
    shutdown().await?;
    Ok(())
}
```

## 🔧 配置选项

### 全局配置

- `global_level`: 全局日志级别 ("TRACE", "DEBUG", "INFO", "WARN", "ERROR")
- `pre_init_buffer_size`: 预初始化缓冲区大小
- `pre_init_stdout_enabled`: 是否启用预初始化标准输出
- `backpressure_strategy`: 背压策略 ("Block" 或 "Drop")

### 输出目标配置

#### 标准输出 (stdout)
```toml
[stdout]
enabled = true
level = "INFO"
format = { type = "text" }  # 或 { type = "json" }
```

#### 文件输出 (file)
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
strategy = "size"  # 或 "time"
max_size_mb = 50
max_files = 5
```

#### 数据库输出 (database)
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

### 上下文字段配置
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

### 格式配置
```toml
[format]
type = "json"  # 或 "text"
timestamp_format = "%Y-%m-%d %H:%M:%S%.3f"
log_template = "{timestamp} [{level}] {target} - {message}"
json_fields_key = "fields"
```

## 📊 诊断和监控

QuantumLog 提供了详细的诊断信息来监控日志系统的性能：

```rust
use quantum_log::get_diagnostics;

let diagnostics = get_diagnostics();
println!("已处理事件: {}", diagnostics.events_processed);
println!("丢弃事件: {}", diagnostics.events_dropped_backpressure);
println!("成功率: {:.2}%", diagnostics.success_rate() * 100.0);
println!("运行时间: {:?}", diagnostics.uptime());
```

## 🚨 错误处理

QuantumLog 提供了详细的错误类型和处理机制：

```rust
use quantum_log::{QuantumLogError, Result};

match quantum_log::init().await {
    Ok(_) => println!("初始化成功"),
    Err(QuantumLogError::InitializationError(msg)) => {
        eprintln!("初始化失败: {}", msg);
    },
    Err(QuantumLogError::ConfigError(msg)) => {
        eprintln!("配置错误: {}", msg);
    },
    Err(e) => {
        eprintln!("其他错误: {}", e);
    }
}
```

## 🔄 优雅关闭

QuantumLog 支持多种优雅关闭方式：

```rust
use quantum_log::shutdown::{ShutdownCoordinator, ShutdownSignal};

// 方式1: 简单关闭
quantum_log::shutdown().await?;

// 方式2: 使用句柄关闭
let handle = quantum_log::init_quantum_logger().await?;
handle.shutdown().await?;

// 方式3: 协调器管理多个组件
let mut coordinator = ShutdownCoordinator::new();
let handle = quantum_log::init_quantum_logger().await?;
coordinator.register_component("quantum_log", handle);

// 执行协调关闭
coordinator.shutdown_all(ShutdownSignal::Graceful).await?;
```

## 🧪 测试

运行测试：

```bash
# 运行所有测试
cargo test

# 运行特定功能的测试
cargo test --features database
cargo test --features mpi

# 运行示例
cargo run --example basic_usage
cargo run --example complete_examples
cargo run --example config_file_example
```

## 📝 许可证

本项目采用 MIT 许可证。详见 [LICENSE](LICENSE) 文件。

## 🤝 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解如何参与项目开发。

## 📞 支持

如果您遇到问题或有建议，请：

1. 查看 [文档](https://docs.rs/quantum_log)
2. 搜索或创建 [Issue](https://github.com/your-username/quantum_log/issues)
3. 参与 [讨论](https://github.com/your-username/quantum_log/discussions)

## 🔗 相关链接

- [Crates.io](https://crates.io/crates/quantum_log)
- [文档](https://docs.rs/quantum_log)
- [GitHub 仓库](https://github.com/your-username/quantum_log)
- [更新日志](CHANGELOG.md)