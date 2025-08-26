//! QuantumLog TLS网络传输示例
//! 演示如何配置和使用TLS加密的网络日志传输

use quantum_log::config::{NetworkConfig, NetworkProtocol, OutputFormat, SecurityPolicy, TlsVersion, TlsCipherSuite};
// use quantum_log::sinks::NetworkSink; // 注释掉未使用的导入
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// 示例1: 基本TLS网络配置
async fn example_basic_tls_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例1: 基本TLS网络配置 ===");

    let network_config = NetworkConfig {
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
        connection_rate_limit: 100,
        enable_security_audit: false,
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: true,
        #[cfg(feature = "tls")]
        tls_verify_hostname: true,
        #[cfg(feature = "tls")]
        tls_ca_file: None, // 使用系统默认CA证书
        #[cfg(feature = "tls")]
        tls_cert_file: None,
        #[cfg(feature = "tls")]
        tls_key_file: None,
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls13,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::High,
        #[cfg(feature = "tls")]
        tls_require_sni: true,
    };

    println!("TLS网络配置: {:?}", network_config);
    println!("基本TLS配置示例完成");
    Ok(())
}

/// 示例2: 自定义CA证书配置
async fn example_custom_ca_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例2: 自定义CA证书配置 ===");

    let network_config = NetworkConfig {
        enabled: true,
        level: None,
        protocol: NetworkProtocol::Tcp,
        host: "internal-log-server.company.com".to_string(),
        port: 8443,
        format: OutputFormat::Json,
        buffer_size: 8192,
        timeout_ms: 10000,
        max_reconnect_attempts: 5,
        reconnect_delay_ms: 2000,
        security_policy: SecurityPolicy::Strict,
        connection_rate_limit: 100,
        enable_security_audit: false,
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: true,
        #[cfg(feature = "tls")]
        tls_verify_hostname: true,
        #[cfg(feature = "tls")]
        tls_ca_file: Some("/etc/ssl/certs/company-ca.pem".to_string()),
        #[cfg(feature = "tls")]
        tls_cert_file: None,
        #[cfg(feature = "tls")]
        tls_key_file: None,
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls12,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::High,
        #[cfg(feature = "tls")]
        tls_require_sni: false,
    };

    println!("自定义CA配置: {:?}", network_config);
    println!("自定义CA证书配置示例完成");
    Ok(())
}

/// 示例3: 客户端证书认证配置
async fn example_client_cert_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例3: 客户端证书认证配置 ===");

    let network_config = NetworkConfig {
        enabled: true,
        level: None,
        protocol: NetworkProtocol::Tcp,
        host: "secure-api.example.com".to_string(),
        port: 8443,
        format: OutputFormat::Json,
        buffer_size: 8192,
        timeout_ms: 5000,
        max_reconnect_attempts: 3,
        reconnect_delay_ms: 1000,
        security_policy: SecurityPolicy::Strict,
        connection_rate_limit: 50,
        enable_security_audit: false,
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: true,
        #[cfg(feature = "tls")]
        tls_verify_hostname: true,
        #[cfg(feature = "tls")]
        tls_ca_file: Some("/etc/ssl/certs/ca-bundle.pem".to_string()),
        #[cfg(feature = "tls")]
        tls_cert_file: Some("/etc/ssl/client/client.crt".to_string()),
        #[cfg(feature = "tls")]
        tls_key_file: Some("/etc/ssl/client/client.key".to_string()),
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls12,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::High,
        #[cfg(feature = "tls")]
        tls_require_sni: true,
    };

    println!("客户端证书认证配置: {:?}", network_config);
    println!("客户端证书认证配置示例完成");
    Ok(())
}

/// 示例4: 开发环境配置（跳过证书验证）
async fn example_dev_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例4: 开发环境配置 ===");
    println!("⚠️  警告: 此配置仅适用于开发环境，生产环境请启用证书验证");

    let network_config = NetworkConfig {
        enabled: true,
        level: None,
        protocol: NetworkProtocol::Tcp,
        host: "localhost".to_string(),
        port: 8080,
        format: OutputFormat::Json,
        buffer_size: 8192,
        timeout_ms: 3000,
        max_reconnect_attempts: 1,
        reconnect_delay_ms: 500,
        security_policy: SecurityPolicy::Permissive,
        connection_rate_limit: 10,
        enable_security_audit: false,
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: false,
        #[cfg(feature = "tls")]
        tls_verify_hostname: false,
        #[cfg(feature = "tls")]
        tls_ca_file: None,
        #[cfg(feature = "tls")]
        tls_cert_file: None,
        #[cfg(feature = "tls")]
        tls_key_file: None,
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls12,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::Medium,
        #[cfg(feature = "tls")]
        tls_require_sni: false,
    };

    println!("开发环境配置: {:?}", network_config);
    println!("开发环境配置示例完成");
    Ok(())
}

/// 示例5: 完整的TLS网络日志记录
#[cfg(feature = "tls")]
async fn example_full_tls_logging() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例5: 完整的TLS网络日志记录 ===");

    // 注意：这个示例需要实际的TLS服务器才能运行
    // 在实际使用中，请确保服务器地址和证书路径正确
    
    let network_config = NetworkConfig {
        enabled: true,
        level: None,
        protocol: NetworkProtocol::Tcp,
        host: "log-server.example.com".to_string(),
        port: 8443,
        format: OutputFormat::Json,
        buffer_size: 8192,
        timeout_ms: 5000,
        max_reconnect_attempts: 3,
        reconnect_delay_ms: 1000,
        security_policy: SecurityPolicy::Strict,
        connection_rate_limit: 100,
        enable_security_audit: true,
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: true,
        #[cfg(feature = "tls")]
        tls_verify_hostname: true,
        #[cfg(feature = "tls")]
        tls_ca_file: Some("/path/to/ca-cert.pem".to_string()),
        #[cfg(feature = "tls")]
        tls_cert_file: None,
        #[cfg(feature = "tls")]
        tls_key_file: None,
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls12,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::High,
        #[cfg(feature = "tls")]
        tls_require_sni: true,
    };

    println!("尝试创建TLS网络Sink...");
    
    // 在实际环境中，这里会尝试连接到TLS服务器
    // let network_sink = NetworkSink::new(network_config).await?;
    
    println!("模拟TLS网络日志记录:");
    info!("应用启动，使用TLS加密传输日志");
    warn!(component = "security", "检测到可疑活动");
    error!(error_code = "TLS001", "TLS握手失败");
    debug!("TLS连接状态检查");
    
    println!("完整的TLS网络日志记录示例完成");
    Ok(())
}

#[cfg(not(feature = "tls"))]
async fn example_full_tls_logging() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例5: 完整的TLS网络日志记录 ===");
    println!("TLS特性未启用，请使用 --features tls 编译");
    Ok(())
}

/// 示例6: TLS配置最佳实践
async fn example_tls_best_practices() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 示例6: TLS配置最佳实践 ===");
    
    println!("🔒 TLS安全配置最佳实践:");
    println!("1. 生产环境始终启用证书验证 (tls_verify_certificates: true)");
    println!("2. 启用主机名验证防止中间人攻击 (tls_verify_hostname: true)");
    println!("3. 使用受信任的CA证书或自定义CA证书文件");
    println!("4. 定期更新和轮换客户端证书");
    println!("5. 配置适当的超时和重连策略");
    println!("6. 监控TLS连接状态和错误");
    
    // 生产环境推荐配置
    let production_config = NetworkConfig {
        enabled: true,
        level: None,
        protocol: NetworkProtocol::Tcp,
        host: "prod-log-server.company.com".to_string(),
        port: 8443,
        format: OutputFormat::Json,
        buffer_size: 8192,
        timeout_ms: 10000,
        max_reconnect_attempts: 5,
        reconnect_delay_ms: 2000,
        security_policy: SecurityPolicy::Strict,
        connection_rate_limit: 100,
        enable_security_audit: true,
        #[cfg(feature = "tls")]
        use_tls: Some(true),
        #[cfg(feature = "tls")]
        tls_verify_certificates: true,
        #[cfg(feature = "tls")]
        tls_verify_hostname: true,
        #[cfg(feature = "tls")]
        tls_ca_file: Some("/etc/ssl/certs/company-ca.pem".to_string()),
        #[cfg(feature = "tls")]
        tls_cert_file: Some("/etc/ssl/certs/app-client.pem".to_string()),
        #[cfg(feature = "tls")]
        tls_key_file: Some("/etc/ssl/private/app-client-key.pem".to_string()),
        #[cfg(feature = "tls")]
        tls_min_version: TlsVersion::Tls13,
        #[cfg(feature = "tls")]
        tls_cipher_suite: TlsCipherSuite::High,
        #[cfg(feature = "tls")]
        tls_require_sni: true,
    };
    
    println!("\n📋 生产环境推荐配置:");
    println!("{:#?}", production_config);
    
    println!("TLS配置最佳实践示例完成");
    Ok(())
}

/// 主函数 - 运行所有TLS示例
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("QuantumLog TLS网络传输示例");
    println!("==============================\n");

    // 初始化基本日志系统
    quantum_log::init().await?;

    // 运行所有示例
    let examples = vec![
        "基本TLS网络配置",
        "自定义CA证书配置",
        "客户端证书认证配置",
        "开发环境配置",
        "完整的TLS网络日志记录",
        "TLS配置最佳实践",
    ];

    for name in examples {
        println!("\n运行示例: {}", name);
        println!("{}", "=".repeat(50));

        let result = match name {
            "基本TLS网络配置" => example_basic_tls_config().await,
            "自定义CA证书配置" => example_custom_ca_config().await,
            "客户端证书认证配置" => example_client_cert_config().await,
            "开发环境配置" => example_dev_config().await,
            "完整的TLS网络日志记录" => example_full_tls_logging().await,
            "TLS配置最佳实践" => example_tls_best_practices().await,
            _ => {
                println!("未知示例: {}", name);
                continue;
            }
        };

        match result {
            Ok(_) => println!("✅ 示例 '{}' 执行成功", name),
            Err(e) => println!("❌ 示例 '{}' 执行失败: {}", name, e),
        }

        // 示例间短暂延迟
        sleep(Duration::from_millis(500)).await;
    }

    // 优雅关闭
    quantum_log::shutdown().await?;
    
    println!("\n🎉 所有TLS示例执行完成！");
    println!("\n📚 更多信息:");
    println!("- 查看项目文档了解详细配置选项");
    println!("- 参考 examples/complete_examples.rs 了解集成用法");
    println!("- 使用 cargo test --features tls 运行TLS相关测试");
    
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
    async fn test_basic_tls_config() {
        test_example_function(example_basic_tls_config, "基本TLS配置").await;
    }

    #[tokio::test]
    async fn test_custom_ca_config() {
        test_example_function(example_custom_ca_config, "自定义CA配置").await;
    }

    #[tokio::test]
    async fn test_client_cert_config() {
        test_example_function(example_client_cert_config, "客户端证书配置").await;
    }

    #[tokio::test]
    async fn test_dev_config() {
        test_example_function(example_dev_config, "开发环境配置").await;
    }

    #[tokio::test]
    async fn test_tls_best_practices() {
        test_example_function(example_tls_best_practices, "TLS最佳实践").await;
    }
}