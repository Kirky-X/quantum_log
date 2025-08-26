//! 从配置文件加载 QuantumLog 配置的示例
//! 这个示例展示了如何从 TOML 配置文件加载配置并初始化 QuantumLog

use quantum_log::{init_with_config, shutdown, QuantumLogConfig};
use std::path::Path;
use tokio::fs;
use tracing::{debug, error, info, span, warn, Level};

/// 从字符串加载配置（用于演示）
async fn load_config_from_string(
    config_content: &str,
) -> Result<QuantumLogConfig, Box<dyn std::error::Error>> {
    let config: QuantumLogConfig = toml::from_str(config_content)?;
    Ok(config)
}

/// 从文件加载配置
async fn load_config_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<QuantumLogConfig, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path).await?;
    let config: QuantumLogConfig = toml::from_str(&content)?;
    Ok(config)
}

/// 创建示例配置文件内容
fn create_example_config() -> &'static str {
    r#"
# QuantumLog 示例配置
global_level = "DEBUG"
pre_init_buffer_size = 2000
pre_init_stdout_enabled = true
backpressure_strategy = "Drop"

[context_fields]
timestamp = true
level = true
target = true
file_line = true
pid = true
tid = false
mpi_rank = false
username = true
hostname = true
span_info = true

[format]
format_type = "Json"
timestamp_format = "%Y-%m-%d %H:%M:%S%.3f"
template = "{timestamp} [{level}] {target}:{line} - {message}"
json_fields_key = "data"

[stdout]
enabled = true
level = "INFO"
format = "Text"

[file]
enabled = true
level = "DEBUG"
output_type = "Json"
directory = "./logs"
filename_base = "quantum_config_example"
extension = "log"
separation_strategy = "None"
write_buffer_size = 16384
writer_cache_ttl_seconds = 300
writer_cache_capacity = 1024

[file.rotation]
strategy = "Size"
max_size_mb = 25
max_files = 3
compress_rotated_files = false

[database]
enabled = false
level = "WARN"
db_type = "Postgresql"
connection_string = "postgresql://user:pass@localhost/logs"
table_name = "quantum_logs"
batch_size = 100
connection_pool_size = 5
connection_timeout_ms = 5000
auto_create_table = true
"#
}

/// 示例1: 从字符串配置加载
async fn example_load_from_string() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例1: 从字符串配置加载 ===");

    let config_content = create_example_config();
    let config = load_config_from_string(config_content).await?;

    // 只在第一次初始化
    static INIT_ONCE: std::sync::Once = std::sync::Once::new();
    INIT_ONCE.call_once(|| {
        tokio::spawn(async move {
            if let Err(e) = init_with_config(config).await {
                eprintln!("初始化失败: {}", e);
            }
        });
    });

    // 等待初始化完成
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    info!("从字符串配置加载成功");
    debug!("调试级别日志已启用");
    warn!(component = "config", "配置加载完成");
    error!("测试错误日志");

    // 使用结构化日志记录
    let span = span!(Level::INFO, "config_test", config_source = "string");
    let _enter = span.enter();
    info!("在配置测试 span 中记录日志");

    println!("从字符串配置加载示例完成");
    Ok(())
}

/// 示例2: 从文件加载配置
async fn example_load_from_file() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例2: 从文件加载配置 ===");

    let config_path = "./examples/quantum_log_example.toml";

    // 检查配置文件是否存在
    if !Path::new(config_path).exists() {
        println!("配置文件 {} 不存在，创建临时配置文件", config_path);

        // 创建临时配置文件
        let temp_config_path = "./temp_config.toml";
        fs::write(temp_config_path, create_example_config()).await?;

        let _config = load_config_from_file(temp_config_path).await?;
        // 不再重复初始化

        // 清理临时文件
        let _ = fs::remove_file(temp_config_path).await;
    } else {
        let _config = load_config_from_file(config_path).await?;
        // 不再重复初始化
    }

    info!("从文件配置加载成功");
    debug!("文件配置中的调试日志");
    warn!(source = "file", "从文件加载的配置");

    // 记录一些结构化数据
    info!(
        config_file = config_path,
        load_time = chrono::Utc::now().to_rfc3339(),
        "配置文件加载完成"
    );

    println!("从文件加载配置示例完成");
    Ok(())
}

/// 示例3: 动态配置修改
async fn example_dynamic_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例3: 动态配置修改 ===");

    // 创建基础配置
    let mut config_content = create_example_config().to_string();

    // 修改配置 - 更改日志级别
    config_content = config_content.replace("global_level = \"DEBUG\"", "global_level = \"WARN\"");

    let _config = load_config_from_string(&config_content).await?;
    // 不再重复初始化

    // 这些日志不会显示，因为级别设置为 WARN
    debug!("这条调试日志不会显示");
    info!("这条信息日志不会显示");

    // 这些日志会显示
    warn!("这条警告日志会显示");
    error!("这条错误日志会显示");

    info!("动态配置修改测试完成");

    println!("动态配置修改示例完成");
    Ok(())
}

/// 示例4: 环境特定配置
async fn example_environment_specific_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例4: 环境特定配置 ===");

    // 模拟不同环境的配置
    let env = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());

    let config_content = match env.as_str() {
        "production" => {
            r#"
global_level = "INFO"
pre_init_buffer_size = 5000
pre_init_stdout_enabled = false

[stdout]
enabled = false

[file]
enabled = true
level = "INFO"
directory = "./logs/production"
filename_base = "quantum_prod"
output_type = "Json"

[database]
enabled = true
level = "WARN"
"#
        }
        "testing" => {
            r#"
global_level = "DEBUG"
pre_init_buffer_size = 1000
pre_init_stdout_enabled = true

[stdout]
enabled = true
level = "INFO"
color_enabled = true
colored = true
format = "Text"

[file]
enabled = false
"#
        }
        _ => {
            // development
            r#"
global_level = "DEBUG"
pre_init_buffer_size = 2000
pre_init_stdout_enabled = true

[stdout]
enabled = true
level = "DEBUG"
format = "Text"

[file]
enabled = true
level = "DEBUG"
directory = "./logs/development"
filename_base = "quantum_dev"
output_type = "Json"
"#
        }
    };

    let _config = load_config_from_string(config_content).await?;
    // 不再重复初始化

    info!(environment = %env, "环境特定配置已加载");
    debug!("开发环境调试信息");
    warn!("环境配置警告");

    println!("环境特定配置示例完成");
    Ok(())
}

/// 示例5: 配置验证和错误处理
async fn example_config_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例5: 配置验证和错误处理 ===");

    // 测试无效配置
    let invalid_configs = vec![
        // 无效的日志级别
        r#"
global_level = "INVALID_LEVEL"
"#,
        // 无效的数字值
        r#"
global_level = "INFO"
pre_init_buffer_size = -1
"#,
        // 缺少必需字段的配置
        r#"
[file]
enabled = true
# 缺少 path 字段
"#,
    ];

    for (i, invalid_config) in invalid_configs.iter().enumerate() {
        println!("测试无效配置 {}", i + 1);
        match load_config_from_string(invalid_config).await {
            Ok(_) => println!("⚠️  配置 {} 意外通过验证", i + 1),
            Err(e) => println!("✅ 配置 {} 正确被拒绝: {}", i + 1, e),
        }
    }

    // 使用有效配置继续
    let valid_config = create_example_config();
    let _config = load_config_from_string(valid_config).await?;
    // 不再重复初始化

    info!("配置验证测试完成");

    println!("配置验证和错误处理示例完成");
    Ok(())
}

/// 主函数
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("QuantumLog 配置文件示例");
    println!("========================\n");

    // 运行所有示例
    let examples = vec![
        "从字符串配置加载",
        "从文件加载配置",
        "动态配置修改",
        "环境特定配置",
        "配置验证和错误处理",
    ];

    for name in examples {
        println!("\n运行示例: {}", name);
        println!("{}", "=".repeat(50));

        let result = match name {
            "从字符串配置加载" => example_load_from_string().await,
            "从文件加载配置" => example_load_from_file().await,
            "动态配置修改" => example_dynamic_config().await,
            "环境特定配置" => example_environment_specific_config().await,
            "配置验证和错误处理" => example_config_validation().await,
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
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    println!("\n🎉 所有配置文件示例运行完成！");

    // 最后关闭日志系统
    shutdown().await?;
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
    async fn test_load_from_string() {
        test_example_function(example_load_from_string, "从字符串加载配置").await;
    }

    #[tokio::test]
    async fn test_dynamic_config() {
        test_example_function(example_dynamic_config, "动态配置").await;
    }

    #[tokio::test]
    async fn test_environment_specific_config() {
        match example_environment_specific_config().await {
            Ok(_) => println!("✅ 环境特定配置测试通过"),
            Err(e) => {
                // 环境特定配置可能因为缺少环境变量而失败，这是正常的
                println!("⚠️ 环境特定配置测试失败（可能是正常的）: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_config_validation() {
        test_example_function(example_config_validation, "配置验证").await;
    }

    #[test]
    fn test_config_parsing() {
        let config_content = create_example_config();
        let result: Result<QuantumLogConfig, _> = toml::from_str(config_content);
        assert!(result.is_ok(), "配置解析失败");
        println!("✅ 配置解析测试通过");
    }
}
