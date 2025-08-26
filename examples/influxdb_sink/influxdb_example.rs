//! QuantumLog InfluxDB 示例
//!
//! 此示例演示如何使用 QuantumLog 将日志写入 InfluxDB。

use quantum_log::{init_with_config, load_config_from_file, shutdown};
use std::path::Path;
use tracing::{debug, error, info, trace, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 加载配置文件
    let config_path = Path::new("examples/influxdb_example.toml");
    let config = load_config_from_file(config_path)?;
    
    // 初始化 QuantumLog
    init_with_config(config).await?;
    
    // 记录一些示例日志
    trace!("这是 trace 级别的日志");
    debug!("这是 debug 级别的日志");
    info!("应用程序已启动");
    warn!("这是一个警告");
    error!("这是一个错误");
    
    // 记录一些带有额外字段的日志
    info!(user_id = "12345", action = "login", "用户登录");
    warn!(error_code = 500, "服务内部错误");
    error!(database = "mysql", table = "users", "数据库连接失败");
    
    // 模拟一些工作
    for i in 1..=10 {
        info!(iteration = i, "处理第 {} 项", i);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    info!("应用程序即将关闭");
    
    // 优雅关闭
    shutdown().await?;
    
    Ok(())
}