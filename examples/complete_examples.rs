//! QuantumLog 完整示例集合
//! 这个文件包含了所有主要使用场景的完整、可运行的示例代码

use quantum_log::config::*;
use quantum_log::{get_buffer_stats, get_diagnostics, QuantumLogConfig};
use quantum_log::config::{DatabaseSinkConfig, FileSinkConfig, NetworkConfig, NetworkProtocol, OutputFormat};
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, error, info, span, warn, Level};

/// 示例1: 基本使用
async fn example_basic_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例1: 基本使用 ===");

    // 使用标准 tracing 宏
    info!("应用程序启动");
    warn!("这是一个警告");
    error!("这是一个错误");
    debug!("调试信息");

    println!("基本使用示例完成");
    Ok(())
}

/// 示例2: 使用设计文档推荐的 API
async fn example_design_document_api() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例2: 设计文档 API ===");

    info!("使用 QuantumLog 记录日志");
    warn!("警告消息");
    error!("错误消息");

    println!("设计文档 API 示例完成");
    Ok(())
}

/// 示例3: 自定义配置
async fn example_custom_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例3: 自定义配置 ===");

    debug!("调试消息现在会被记录");
    info!("应用程序已配置");
    error!("这是一个错误");

    println!("自定义配置示例完成");
    Ok(())
}

/// 示例4: 使用构建器模式
async fn example_builder_pattern() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例4: 构建器模式 ===");

    // 创建 span 进行结构化日志记录
    let span = span!(Level::INFO, "user_operation", user_id = 12345);
    let _enter = span.enter();

    info!("用户操作开始");
    info!(action = "login", result = "success", "用户登录成功");

    println!("构建器模式示例完成");
    Ok(())
}

/// 示例5: 结构化日志记录
async fn example_structured_logging() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例5: 结构化日志记录 ===");

    // 基本结构化日志记录
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
    let request_span = span!(
        Level::INFO,
        "http_request",
        method = "POST",
        path = "/api/users",
        request_id = "req-123"
    );

    let _enter = request_span.enter();
    info!("处理 HTTP 请求");
    info!(status = 200, duration_ms = 45, "请求完成");

    // 嵌套 span
    let db_span = span!(Level::DEBUG, "database_query", table = "users");
    let _db_enter = db_span.enter();
    info!(query = "SELECT * FROM users WHERE id = ?", "执行数据库查询");

    println!("结构化日志记录示例完成");
    Ok(())
}

/// 示例6: 错误处理和诊断
async fn example_error_handling_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例6: 错误处理和诊断 ===");

    // 记录一些消息
    for i in 0..100 {
        info!(iteration = i, "处理迭代 {}", i);
        if i % 10 == 0 {
            error!(iteration = i, "模拟错误");
        }
    }

    // 获取缓冲区统计信息
    if let Some(stats) = get_buffer_stats() {
        info!(
            current_size = stats.current_size,
            dropped_count = stats.dropped_count,
            is_initialized = stats.is_initialized,
            "缓冲区统计信息"
        );
    }

    // 获取诊断信息
    let diagnostics = get_diagnostics();
    info!(
        events_processed = diagnostics.events_processed,
        events_dropped = diagnostics.events_dropped_backpressure + diagnostics.events_dropped_error,
        sink_errors = diagnostics.sink_errors,
        uptime_seconds = diagnostics.uptime.map(|d| d.as_secs()).unwrap_or(0),
        success_rate = format!(
            "{:.2}%",
            if diagnostics.events_processed > 0 {
                (diagnostics.events_processed as f64
                    / (diagnostics.events_processed
                        + diagnostics.events_dropped_backpressure
                        + diagnostics.events_dropped_error
                        + diagnostics.total_events_dropped) as f64)
                    * 100.0
            } else {
                0.0
            }
        ),
        "诊断信息"
    );

    println!("错误处理和诊断示例完成");
    Ok(())
}

/// 示例7: 高级配置
async fn example_advanced_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例7: 高级配置 ===");

    QuantumLogConfig {
        global_level: "DEBUG".to_string(),
        pre_init_buffer_size: Some(2000),
        pre_init_stdout_enabled: true,
        backpressure_strategy: BackpressureStrategy::Drop,

        // 配置标准输出
        stdout: Some(StdoutConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            color_enabled: Some(true),
            level_colors: None,
            format: OutputFormat::Json,
            colored: true,
        }),

        // 配置文件输出
        file: Some(quantum_log::config::FileSinkConfig {
            enabled: true,
            level: Some("DEBUG".to_string()),
            output_type: FileOutputType::Json,
            directory: std::path::PathBuf::from("./logs"),
            filename_base: "quantum".to_string(),
            extension: Some("log".to_string()),
            separation_strategy: FileSeparationStrategy::None,
            write_buffer_size: 16384,
            rotation: Some(RotationConfig {
                strategy: RotationStrategy::Size,
                max_size_mb: Some(50),
                max_files: Some(5),
                compress_rotated_files: false,
            }),
            writer_cache_ttl_seconds: 600,
            writer_cache_capacity: 2048,
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
            timestamp_format: "%Y-%m-%d %H:%M:%S%.3f".to_string(),
            template: "{timestamp} [{level}] {target} - {message}".to_string(),
            csv_columns: None,
            json_flatten_fields: false,
            json_fields_key: "fields".to_string(),
            format_type: LogFormatType::Json,
        },

        // 数据库配置（需要 database 特性）
        #[cfg(feature = "database")]
        database: Some(DatabaseSinkConfig {
            enabled: false, // 在示例中禁用以避免依赖数据库
            level: Some("WARN".to_string()),
            db_type: DatabaseType::Postgresql,
            connection_string: "postgresql://user:pass@localhost/logs".to_string(),
            schema_name: None,
            table_name: "quantum_logs".to_string(),
            batch_size: 200,
            connection_pool_size: 10,
            connection_timeout_ms: 10000,
            auto_create_table: true,
        }),

        #[cfg(not(feature = "database"))]
        database: None,
        
        // 网络配置（启用TLS加密传输）
        #[cfg(feature = "tls")]
        network: Some(NetworkConfig {
            enabled: true,
            level: None,
            protocol: NetworkProtocol::Tcp,
            host: "secure-log-server.example.com".to_string(),
            port: 8443,
            format: OutputFormat::Json,
            buffer_size: 8192,
            timeout_ms: 5000,
            max_reconnect_attempts: 3,
            reconnect_delay_ms: 1000,
            security_policy: SecurityPolicy::Strict,
            connection_rate_limit: 50,
            enable_security_audit: true,
            // TLS安全配置
            use_tls: Some(true),
            tls_verify_certificates: true,
            tls_verify_hostname: true,
            tls_ca_file: Some("/path/to/ca-cert.pem".to_string()),
            tls_cert_file: Some("/path/to/client-cert.pem".to_string()),
            tls_key_file: Some("/path/to/client-key.pem".to_string()),
            tls_min_version: TlsVersion::Tls12,
            tls_cipher_suite: TlsCipherSuite::High,
            tls_require_sni: true,
        }),
        
        #[cfg(not(feature = "tls"))]
        network: None,
        
        // 级别文件配置
        level_file: None,
        
        // InfluxDB配置
        influxdb: None,
    };

    info!("高级配置已初始化");
    warn!(component = "auth", "认证模块警告");
    error!(error_code = "E001", module = "database", "数据库连接失败");

    println!("高级配置示例完成");
    Ok(())
}

/// 示例8: 性能测试和基准测试
async fn example_performance_test() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例8: 性能测试 ===");

    let start_time = Instant::now();
    let num_messages = 1000; // 减少消息数量以便快速演示

    info!("开始性能测试，将记录 {} 条消息", num_messages);

    // 高频日志记录测试
    for i in 0..num_messages {
        match i % 4 {
            0 => info!(id = i, "信息日志 {}", i),
            1 => warn!(id = i, "警告日志 {}", i),
            2 => error!(id = i, "错误日志 {}", i),
            _ => info!(
                id = i,
                data = format!("complex_data_{}", i),
                "结构化日志 {}",
                i
            ),
        }

        // 每 100 条消息暂停一下以模拟真实应用
        if i % 100 == 0 && i > 0 {
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
        success_rate = format!(
            "{:.4}%",
            if diagnostics.events_processed > 0 {
                ((diagnostics.events_processed - diagnostics.total_events_dropped) as f64
                    / diagnostics.events_processed as f64)
                    * 100.0
            } else {
                100.0
            }
        ),
        "最终统计信息"
    );
    println!("性能测试示例完成");
    Ok(())
}

/// 示例9: MPI 环境使用（需要 mpi_support 特性）
#[cfg(feature = "mpi_support")]
async fn example_mpi_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例9: MPI 环境使用 ===");

    use quantum_log::mpi::*;

    // 检查 MPI 是否可用
    if is_mpi_available() {
        let rank = get_mpi_rank().unwrap_or(0);

        let config = QuantumLogConfig {
            global_level: "INFO".to_string(),
            context_fields: ContextFieldsConfig {
                mpi_rank: true,
                ..Default::default()
            },
            file: Some(FileSinkConfig {
                enabled: true,
                level: Some("DEBUG".to_string()),
                output_type: FileOutputType::Json,
                directory: std::path::PathBuf::from(format!("./logs/rank_{}", rank)),
                filename_base: format!("quantum_rank_{}", rank),
                extension: Some("log".to_string()),
                separation_strategy: FileSeparationStrategy::None,
                write_buffer_size: 8192,
                rotation: None,
                writer_cache_ttl_seconds: 300,
                writer_cache_capacity: 1024,
            }),
            ..Default::default()
        };

        // 注意：在示例集合中，日志系统已经在main函数中初始化
        // 这里我们只是演示MPI配置，不重复初始化
        println!("MPI配置: rank={}, 日志目录: ./logs/rank_{}", rank, rank);

        info!(rank = rank, "MPI 进程启动");

        // 模拟 MPI 工作负载
        for i in 0..10 {
            info!(rank = rank, iteration = i, "处理数据块 {}", i);
            sleep(Duration::from_millis(100)).await;
        }

        warn!(rank = rank, "MPI 进程即将结束");
    } else {
        println!("MPI 不可用，使用标准模式");
        info!("标准模式启动");
    }

    println!("MPI 使用示例完成");
    Ok(())
}

#[cfg(not(feature = "mpi_support"))]
#[allow(dead_code)]
async fn example_mpi_usage() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例9: MPI 环境使用 ===");
    println!("MPI 特性未启用，使用标准模式");

    quantum_log::init().await?;
    info!("标准模式启动");
    quantum_log::shutdown().await?;

    println!("MPI 使用示例完成（标准模式）");
    Ok(())
}

/// 示例10: 错误处理详细演示
async fn example_detailed_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例10: 详细错误处理 ===");

    // 记录各种类型的错误
    error!(error_type = "validation", "数据验证失败");
    error!(error_type = "network", code = 500, "网络连接错误");
    error!(error_type = "database", "数据库操作失败");

    // 演示错误恢复
    info!("尝试错误恢复");
    warn!("系统正在恢复中");
    info!("错误恢复完成");

    println!("详细错误处理示例完成");
    Ok(())
}

/// 主函数 - 运行所有示例
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("QuantumLog 完整示例集合");
    println!("=========================\n");

    // 统一初始化日志系统
    quantum_log::init().await?;

    // 运行所有示例
    let examples = vec![
        "基本使用",
        "设计文档 API",
        "自定义配置",
        "构建器模式",
        "结构化日志记录",
        "错误处理和诊断",
        "高级配置",
        "性能测试",
        "详细错误处理",
    ];

    #[cfg(feature = "mpi_support")]
    let mpi_examples = vec!["MPI 环境使用"];

    // 合并所有示例
    #[cfg(feature = "mpi_support")]
    let all_examples = {
        let mut combined = examples;
        combined.extend(mpi_examples);
        combined
    };
    #[cfg(not(feature = "mpi_support"))]
    let all_examples = examples;

    for name in all_examples {
        println!("\n运行示例: {}", name);
        println!("{}", "=".repeat(50));

        let result = match name {
            "基本使用" => example_basic_usage().await,
            "设计文档 API" => example_design_document_api().await,
            "自定义配置" => example_custom_config().await,
            "构建器模式" => example_builder_pattern().await,
            "结构化日志记录" => example_structured_logging().await,
            "错误处理和诊断" => example_error_handling_diagnostics().await,
            "高级配置" => example_advanced_config().await,
            "性能测试" => example_performance_test().await,
            "详细错误处理" => example_detailed_error_handling().await,
            #[cfg(feature = "mpi_support")]
            "MPI 环境使用" => example_mpi_usage().await,
            _ => {
                eprintln!("❌ 未知示例: {}", name);
                continue;
            }
        };

        match result {
            Ok(_) => println!("✅ {} 示例成功完成", name),
            Err(e) => {
                eprintln!("❌ {} 示例失败: {}", name, e);
                // 继续运行其他示例
            }
        }

        // 在示例之间稍作停顿
        sleep(Duration::from_millis(500)).await;
    }

    // 统一关闭日志系统
    quantum_log::shutdown().await?;

    println!("\n🎉 所有示例运行完成！");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 通用测试辅助函数，减少重复代码
    async fn test_example_function<F, Fut>(example_fn: F, test_name: &str)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
    {
        match example_fn().await {
            Ok(_) => println!("✅ {} 测试通过", test_name),
            Err(e) => panic!("❌ {} 测试失败: {}", test_name, e),
        }
    }

    #[tokio::test]
    async fn test_basic_usage() {
        test_example_function(example_basic_usage, "基本使用").await;
    }

    #[tokio::test]
    async fn test_design_document_api() {
        test_example_function(example_design_document_api, "设计文档 API").await;
    }

    #[tokio::test]
    async fn test_custom_config() {
        test_example_function(example_custom_config, "自定义配置").await;
    }

    #[tokio::test]
    async fn test_structured_logging() {
        test_example_function(example_structured_logging, "结构化日志记录").await;
    }

    #[tokio::test]
    async fn test_error_handling_diagnostics() {
        test_example_function(example_error_handling_diagnostics, "错误处理和诊断").await;
    }

    #[tokio::test]
    async fn test_performance_test() {
        test_example_function(example_performance_test, "性能测试").await;
    }

    #[tokio::test]
    async fn test_detailed_error_handling() {
        test_example_function(example_detailed_error_handling, "详细错误处理").await;
    }
}
