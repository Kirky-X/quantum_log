//! QuantumLog 基本使用示例
//!
//! 此示例展示了如何使用 QuantumLog 进行基本的日志记录。

use quantum_log::{init, shutdown};
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 QuantumLog
    println!("正在初始化 QuantumLog...");
    init().await?;
    println!("QuantumLog 初始化完成!");

    // 记录不同级别的日志
    info!("这是一条信息日志");
    warn!("这是一条警告日志");
    error!("这是一条错误日志");
    debug!("这是一条调试日志");

    // 记录带有字段的结构化日志
    info!(user_id = 12345, action = "login", "用户登录成功");
    warn!(error_code = 404, path = "/api/users", "API 路径未找到");

    // 记录带有复杂数据的日志
    let user_data = serde_json::json!({
        "id": 12345,
        "name": "张三",
        "email": "zhangsan@example.com"
    });
    info!(user = %user_data, "用户数据已更新");

    // 模拟一些工作
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 优雅关闭
    println!("正在关闭 QuantumLog...");
    shutdown().await?;
    println!("QuantumLog 已关闭!");

    Ok(())
}
