//! 高级安全措施演示示例
//! 
//! 本示例展示了QuantumLog网络sink的高级安全功能，包括：
//! - 安全审计
//! - 连接速率限制
//! - TLS安全配置
//! - 输入数据验证

use quantum_log::config::{
    NetworkConfig, NetworkProtocol, OutputFormat, SecurityPolicy
};
#[cfg(feature = "tls")]
use quantum_log::config::{TlsVersion, TlsCipherSuite};
use quantum_log::sinks::NetworkSink;
use quantum_log::core::event::{QuantumLogEvent, ContextInfo};
use tracing::Level;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== QuantumLog 高级安全措施演示 ===");
    
    // 创建高安全性网络配置
    let config = NetworkConfig {
        enabled: true,
        level: None,
        host: "127.0.0.1".to_string(),
        port: 8080,
        protocol: NetworkProtocol::Tcp,
        format: OutputFormat::Json,
        buffer_size: 8192,
        timeout_ms: 5000,
        max_reconnect_attempts: 3,
        reconnect_delay_ms: 1000,
        
        // 安全配置
        security_policy: SecurityPolicy::Strict,
        connection_rate_limit: 10, // 每秒最多10个连接
        enable_security_audit: true,
        
        // TLS配置
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: true,
        #[cfg(feature = "tls")]
        tls_verify_hostname: true,
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls13,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::High,
        #[cfg(feature = "tls")]
        tls_require_sni: true,
        #[cfg(feature = "tls")]
        tls_ca_file: None,
        #[cfg(feature = "tls")]
        tls_cert_file: None,
        #[cfg(feature = "tls")]
        tls_key_file: None,
    };
    
    println!("✓ 创建高安全性网络配置");
    println!("  - 安全策略: {:?}", config.security_policy);
    println!("  - 连接速率限制: {} 连接/秒", config.connection_rate_limit);
    println!("  - 安全审计: {}", if config.enable_security_audit { "启用" } else { "禁用" });
    
    #[cfg(feature = "tls")]
    {
        println!("  - TLS最小版本: {:?}", config.tls_min_version);
        println!("  - TLS密码套件: {:?}", config.tls_cipher_suite);
        println!("  - 证书验证: {}", if config.tls_verify_certificates { "启用" } else { "禁用" });
    }
    
    // 创建网络sink
    let mut sink = NetworkSink::new(config);
    println!("✓ 创建网络sink实例");
    
    // 创建测试事件
    let timestamp = Utc::now();
    let event = QuantumLogEvent {
        timestamp,
        level: "INFO".to_string(),
        target: "security_demo".to_string(),
        message: "这是一个安全演示消息".to_string(),
        module_path: Some("security_demo".to_string()),
        file: Some("security_demo.rs".to_string()),
        line: Some(42),
        thread_name: Some("main".to_string()),
        thread_id: "1".to_string(),
        fields: HashMap::new(),
        context: ContextInfo::new(),
    };
    
    println!("✓ 创建测试事件");
    
    // 注意：由于这是演示，我们不会实际启动sink（需要真实的服务器）
    // 在实际使用中，您需要：
    // 1. 启动一个接收日志的服务器
    // 2. 调用 sink.start().await
    // 3. 发送事件 sink.send_event(event).await
    // 4. 关闭 sink.shutdown().await
    
    println!("\n=== 安全功能说明 ===");
    println!("1. 安全审计系统会记录以下事件：");
    println!("   - 连接尝试");
    println!("   - TLS握手开始/成功/失败");
    println!("   - 证书验证失败");
    println!("   - 速率限制超出");
    println!("   - 无效输入数据检测");
    println!("   - 安全策略违规");
    
    println!("\n2. 连接速率限制：");
    println!("   - 使用令牌桶算法控制连接频率");
    println!("   - 防止连接洪水攻击");
    
    println!("\n3. TLS安全配置：");
    println!("   - 强制使用最新TLS版本");
    println!("   - 选择高强度密码套件");
    println!("   - 严格证书验证");
    
    println!("\n4. 输入数据验证：");
    println!("   - 计算数据哈希值");
    println!("   - 检测数据完整性");
    println!("   - 记录可疑数据");
    
    println!("\n✓ 高级安全措施演示完成！");
    
    Ok(())
}