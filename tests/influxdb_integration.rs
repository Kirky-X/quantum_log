//! InfluxDB 集成测试
//!
//! 此测试需要本地运行 InfluxDB 实例
//! 运行前请确保:
//! 1. InfluxDB 服务在 localhost:8086 运行
//! 2. 设置了正确的环境变量 INFLUXDB_TOKEN, INFLUXDB_ORG, INFLUXDB_BUCKET

use quantum_log::{
    config::{InfluxDBConfig, QuantumLogConfig},
    env_config::EnvConfig,
    init_with_config, shutdown,
};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_influxdb_integration() {
    // 检查环境变量配置
    let token = EnvConfig::get_influxdb_token()
        .expect("Failed to get InfluxDB token")
        .expect("INFLUXDB_TOKEN environment variable is required");
    
    let org = EnvConfig::get_influxdb_org()
        .unwrap_or_else(|| "kirky".to_string());
    
    let bucket = EnvConfig::get_influxdb_bucket()
        .unwrap_or_else(|| "test".to_string());
    
    let url = EnvConfig::get_influxdb_url()
        .unwrap_or_else(|| "http://localhost:8086".to_string());
    
    println!("Testing InfluxDB integration with:");
    println!("  URL: {}", url);
    println!("  Org: {}", org);
    println!("  Bucket: {}", bucket);
    println!("  Token: {}...", &token[..std::cmp::min(10, token.len())]);
    
    // 配置 QuantumLog 使用 InfluxDB
    let config = QuantumLogConfig {
        global_level: "INFO".to_string(),
        influxdb: Some(InfluxDBConfig {
            enabled: true,
            level: Some("INFO".to_string()),
            url,
            database: bucket.clone(), // 对于 InfluxDB 2.x，database 字段可以用作 bucket
            token: Some(token),
            username: None,
            password: None,
            batch_size: 100,
            flush_interval_seconds: 5,
            use_https: false,
            verify_ssl: true,
        }),
        ..Default::default()
    };
    
    // 初始化 QuantumLog
    init_with_config(config).await.expect("Failed to initialize QuantumLog");
    
    // 发送测试日志
    info!("InfluxDB integration test started");
    warn!("This is a warning message for InfluxDB");
    error!("This is an error message for InfluxDB");
    
    // 等待日志被刷新到 InfluxDB
    sleep(Duration::from_secs(3)).await;
    
    // 关闭 QuantumLog
    shutdown().await.expect("Failed to shutdown QuantumLog");
    
    println!("InfluxDB integration test completed successfully!");
}

#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_influxdb_connection_validation() {
    use quantum_log::env_config::EnvConfig;
    
    // 验证环境变量配置
    let validation_result = EnvConfig::validate_influxdb_config();
    
    match validation_result {
        Ok(_) => {
            println!("InfluxDB configuration validation passed");
            
            // 尝试获取配置
            let (token, username, password) = quantum_log::env_config::get_secure_influxdb_config()
                .expect("Failed to get secure InfluxDB config");
            
            if let Some(token) = token {
                println!("Using token authentication: {}...", &token[..std::cmp::min(10, token.len())]);
            } else if let (Some(username), Some(password)) = (username, password) {
                println!("Using username/password authentication: {}/***", username);
            } else {
                panic!("No valid authentication method found");
            }
        }
        Err(e) => {
            panic!("InfluxDB configuration validation failed: {}", e);
        }
    }
}