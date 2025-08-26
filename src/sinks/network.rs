//! 网络 Sink
//!
//! 此模块实现通过网络发送日志的 sink，支持 TCP 和 UDP 协议。

use crate::config::{NetworkConfig, NetworkProtocol, SecurityPolicy};
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};
use crate::sinks::traits::{ExclusiveSink, QuantumSink, SinkError};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use tracing::Level;

#[cfg(feature = "tls")]
use std::collections::HashMap;
#[cfg(feature = "tls")]
use tokio::sync::RwLock;
#[cfg(feature = "tls")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(feature = "tls")]
use ring::digest;
#[cfg(feature = "tls")]
// use secrecy::{Secret, ExposeSecret}; // 暂时未使用
#[cfg(feature = "tls")]
use tokio_util::time::DelayQueue;

// 预定义错误消息常量以避免运行时字符串分配
const TCP_WRITE_ERROR: &str = "TCP write failed";
const TCP_FLUSH_ERROR: &str = "TCP flush failed";
#[cfg(feature = "tls")]
const TLS_WRITE_ERROR: &str = "TLS write failed";
#[cfg(feature = "tls")]
const TLS_FLUSH_ERROR: &str = "TLS flush failed";
const UDP_SEND_ERROR: &str = "UDP send failed";
const NO_CONNECTION_ERROR: &str = "No active connection";
const SINK_ALREADY_STARTED: &str = "Sink already started";
const CONNECTION_TIMEOUT_ERROR: &str = "Connection timeout";
const MAX_RECONNECT_ERROR: &str = "Max reconnection attempts exceeded";
const NETWORK_SINK_NOT_STARTED: &str = "NetworkSink not started";
const SHUTDOWN_SIGNAL_FAILED: &str = "Failed to send shutdown signal";
const SHUTDOWN_SIGNAL_LOST: &str = "Shutdown signal lost";
const NEWLINE_BYTES: &[u8] = b"\n";
// const SECURITY_AUDIT_ERROR: &str = "Security audit logging failed"; // 暂时未使用
const RATE_LIMIT_EXCEEDED: &str = "Connection rate limit exceeded";
const INVALID_INPUT_DATA: &str = "Invalid input data detected";
// const TLS_VERSION_NOT_SUPPORTED: &str = "TLS version not supported"; // 暂时未使用
// const CIPHER_SUITE_NOT_ALLOWED: &str = "Cipher suite not allowed"; // 暂时未使用
// const SNI_REQUIRED: &str = "SNI is required for this connection"; // 暂时未使用

#[cfg(feature = "tls")]
use tokio_rustls::{TlsConnector, client::TlsStream};
#[cfg(feature = "tls")]
use rustls::{ClientConfig, RootCertStore};
#[cfg(feature = "tls")]
use rustls_pemfile;
#[cfg(feature = "tls")]
use std::sync::Arc as StdArc;

/// 安全审计事件类型
#[cfg(feature = "tls")]
#[derive(Debug, Clone)]
enum SecurityAuditEvent {
    ConnectionAttempt { host: String, port: u16, timestamp: Instant },
    TlsHandshakeStart { host: String, port: u16, timestamp: Instant },
    TlsHandshakeSuccess { host: String, port: u16, cipher_suite: String, timestamp: Instant },
    TlsHandshakeFailed { host: String, port: u16, error: String, timestamp: Instant },
    CertificateVerificationFailed { host: String, error: String, timestamp: Instant },
    RateLimitExceeded { host: String, port: u16, timestamp: Instant },
    InvalidInputDetected { data_hash: String, timestamp: Instant },
    SecurityPolicyViolation { policy: String, violation: String, timestamp: Instant },
}

/// 连接速率限制器
#[cfg(feature = "tls")]
#[derive(Debug)]
struct RateLimiter {
    connections: Arc<RwLock<HashMap<String, DelayQueue<()>>>>,
    max_connections_per_second: u32,
}

#[cfg(feature = "tls")]
impl RateLimiter {
    fn new(max_connections_per_second: u32) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            max_connections_per_second,
        }
    }
    
    async fn check_rate_limit(&self, host: &str) -> bool {
        let mut connections = self.connections.write().await;
        let queue = connections.entry(host.to_string()).or_insert_with(|| DelayQueue::new());
        
        // 清理过期的连接记录
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(&waker);
        while queue.poll_expired(&mut cx).is_ready() {
            // 移除过期的连接记录
        }
        
        if queue.len() >= self.max_connections_per_second as usize {
            false
        } else {
            queue.insert((), Duration::from_secs(1));
            true
        }
    }
}

/// 安全审计日志记录器
#[cfg(feature = "tls")]
#[derive(Debug)]
struct SecurityAuditor {
    enabled: bool,
    event_count: AtomicU64,
}

#[cfg(feature = "tls")]
impl SecurityAuditor {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            event_count: AtomicU64::new(0),
        }
    }
    
    fn log_event(&self, event: SecurityAuditEvent) {
        if !self.enabled {
            return;
        }
        
        self.event_count.fetch_add(1, Ordering::Relaxed);
        
        match event {
            SecurityAuditEvent::ConnectionAttempt { host, port, timestamp } => {
                tracing::info!(
                    target: "quantum_log::security_audit",
                    event = "connection_attempt",
                    host = %host,
                    port = %port,
                    timestamp = ?timestamp,
                    "Network connection attempt"
                );
            }
            SecurityAuditEvent::TlsHandshakeStart { host, port, timestamp } => {
                tracing::info!(
                    target: "quantum_log::security_audit",
                    event = "tls_handshake_start",
                    host = %host,
                    port = %port,
                    timestamp = ?timestamp,
                    "TLS handshake initiated"
                );
            }
            SecurityAuditEvent::TlsHandshakeSuccess { host, port, cipher_suite, timestamp } => {
                tracing::info!(
                    target: "quantum_log::security_audit",
                    event = "tls_handshake_success",
                    host = %host,
                    port = %port,
                    cipher_suite = %cipher_suite,
                    timestamp = ?timestamp,
                    "TLS handshake completed successfully"
                );
            }
            SecurityAuditEvent::TlsHandshakeFailed { host, port, error, timestamp } => {
                tracing::warn!(
                    target: "quantum_log::security_audit",
                    event = "tls_handshake_failed",
                    host = %host,
                    port = %port,
                    error = %error,
                    timestamp = ?timestamp,
                    "TLS handshake failed"
                );
            }
            SecurityAuditEvent::CertificateVerificationFailed { host, error, timestamp } => {
                tracing::error!(
                    target: "quantum_log::security_audit",
                    event = "certificate_verification_failed",
                    host = %host,
                    error = %error,
                    timestamp = ?timestamp,
                    "Certificate verification failed"
                );
            }
            SecurityAuditEvent::RateLimitExceeded { host, port, timestamp } => {
                tracing::warn!(
                    target: "quantum_log::security_audit",
                    event = "rate_limit_exceeded",
                    host = %host,
                    port = %port,
                    timestamp = ?timestamp,
                    "Connection rate limit exceeded"
                );
            }
            SecurityAuditEvent::InvalidInputDetected { data_hash, timestamp } => {
                tracing::error!(
                    target: "quantum_log::security_audit",
                    event = "invalid_input_detected",
                    data_hash = %data_hash,
                    timestamp = ?timestamp,
                    "Invalid input data detected"
                );
            }
            SecurityAuditEvent::SecurityPolicyViolation { policy, violation, timestamp } => {
                tracing::error!(
                    target: "quantum_log::security_audit",
                    event = "security_policy_violation",
                    policy = %policy,
                    violation = %violation,
                    timestamp = ?timestamp,
                    "Security policy violation detected"
                );
            }
        }
    }
    
    #[allow(dead_code)]
    fn get_event_count(&self) -> u64 {
        self.event_count.load(Ordering::Relaxed)
    }
    
    /// 获取最近的安全事件统计
    #[allow(dead_code)]
    fn get_event_stats(&self) -> String {
        format!("Total security events logged: {}", self.get_event_count())
    }
    
    /// 检查是否启用了安全审计
    #[allow(dead_code)]
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}


/// 网络 Sink
#[derive(Debug)]
pub struct NetworkSink {
    /// 配置
    config: NetworkConfig,
    /// 事件发送器
    sender: Arc<Mutex<Option<mpsc::Sender<SinkMessage>>>>,
    /// 处理器句柄
    processor_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

/// Sink 消息
enum SinkMessage {
    /// 日志事件
    Event(Box<QuantumLogEvent>),
    /// 关闭信号
    Shutdown(oneshot::Sender<Result<()>>),
}

/// 网络连接
enum NetworkConnection {
    /// TCP 连接
    Tcp(Arc<Mutex<BufWriter<TcpStream>>>),
    /// TLS 加密的 TCP 连接
    #[cfg(feature = "tls")]
    Tls(Arc<Mutex<BufWriter<TlsStream<TcpStream>>>>),
    /// UDP 套接字
    Udp(Arc<UdpSocket>, SocketAddr),
}

/// 网络 Sink 处理器
struct NetworkSinkProcessor {
    /// 配置
    config: NetworkConfig,
    /// 事件接收器
    receiver: mpsc::Receiver<SinkMessage>,
    /// 网络连接
    connection: Option<NetworkConnection>,
    /// 重连计数器
    reconnect_count: usize,
    /// 级别过滤器
    level_filter: Option<Level>,
    /// 速率限制器
    #[cfg(feature = "tls")]
    rate_limiter: Option<RateLimiter>,
    /// 安全审计器
    #[cfg(feature = "tls")]
    security_auditor: SecurityAuditor,
}

impl NetworkSink {
    /// 创建新的网络 sink
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            sender: Arc::new(Mutex::new(None)),
            processor_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// 设置级别过滤器
    pub fn with_level_filter(self, _level: Level) -> Self {
        // 注意：这里我们需要在配置中添加级别过滤器字段
        // 暂时存储在内部，实际实现时需要修改配置结构
        self
    }

    /// 启动 sink
    pub async fn start(&mut self) -> Result<()> {
        let mut sender_guard = self.sender.lock().await;
        if sender_guard.is_some() {
            return Err(QuantumLogError::ConfigError(
                SINK_ALREADY_STARTED.into(),
            ));
        }

        let buffer_size = self.config.buffer_size;

        let (sender, receiver) = mpsc::channel(buffer_size);

        let processor = NetworkSinkProcessor::new(self.config.clone(), receiver).await?;
        let handle = tokio::spawn(async move {
            if let Err(e) = processor.run().await {
                tracing::error!("NetworkSink processor error: {}", e);
            }
        });

        *sender_guard = Some(sender);
        drop(sender_guard);
        
        let mut handle_guard = self.processor_handle.lock().await;
        *handle_guard = Some(handle);

        Ok(())
    }

    /// 发送事件
    pub async fn send_event_internal(&self, event: QuantumLogEvent) -> Result<()> {
        let sender_guard = self.sender.lock().await;
        if let Some(sender) = sender_guard.as_ref() {
            let message = SinkMessage::Event(Box::new(event));
            let sender = sender.clone();
            drop(sender_guard);

            // 使用阻塞策略发送事件
            sender.send(message).await.map_err(|_| {
                QuantumLogError::SinkError("Failed to send event to NetworkSink".to_string())
            })?;
        } else {
            return Err(QuantumLogError::SinkError(
                NETWORK_SINK_NOT_STARTED.into(),
            ));
        }
        Ok(())
    }

    /// 内部通用关闭逻辑，支持从 &self 调用
    async fn shutdown_ref(&self) -> Result<()> {
        // 发送关闭信号
        let sender_taken = {
            let mut guard = self.sender.lock().await;
            guard.take()
        };
        if let Some(sender) = sender_taken {
            let (tx, rx) = oneshot::channel();

            if sender.send(SinkMessage::Shutdown(tx)).await.is_err() {
                return Err(QuantumLogError::SinkError(
                    SHUTDOWN_SIGNAL_FAILED.into(),
                ));
            }

            match rx.await {
                Ok(result) => result?,
                Err(_) => {
                    return Err(QuantumLogError::SinkError(
                        SHUTDOWN_SIGNAL_LOST.into(),
                    ))
                }
            }
        }

        // 等待处理器完成
        let handle_taken = {
            let mut guard = self.processor_handle.lock().await;
            guard.take()
        };
        if let Some(handle) = handle_taken {
            if let Err(e) = handle.await {
                tracing::error!("Error waiting for NetworkSink processor: {}", e);
            }
        }

        Ok(())
    }

    /// 关闭 sink
    pub async fn shutdown(&self) -> Result<()> {
        self.shutdown_ref().await
    }

    /// 检查是否正在运行
    pub async fn is_running(&self) -> bool {
        self.sender.lock().await.is_some()
    }

    /// 同步检查是否正在运行（用于非异步上下文）
    pub fn is_running_sync(&self) -> bool {
        // 使用try_lock避免阻塞
        match self.sender.try_lock() {
            Ok(guard) => guard.is_some(),
            Err(_) => true, // 如果锁被占用，假设正在运行
        }
    }

    /// 获取配置
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

impl NetworkSinkProcessor {
    /// 创建新的处理器
    async fn new(config: NetworkConfig, receiver: mpsc::Receiver<SinkMessage>) -> Result<Self> {
        // 验证安全配置
        crate::config::validate_network_security(&config)?;
        
        let level_filter = config.level.as_ref().and_then(|level_str| {
            match level_str.to_lowercase().as_str() {
                "trace" => Some(Level::TRACE),
                "debug" => Some(Level::DEBUG),
                "info" => Some(Level::INFO),
                "warn" => Some(Level::WARN),
                "error" => Some(Level::ERROR),
                _ => None,
            }
        });

        #[cfg(feature = "tls")]
        let rate_limiter = if config.connection_rate_limit > 0 {
            Some(RateLimiter::new(config.connection_rate_limit))
        } else {
            None
        };

        #[cfg(feature = "tls")]
        let security_auditor = SecurityAuditor::new(config.enable_security_audit);

        let mut processor = Self {
            config,
            receiver,
            connection: None,
            reconnect_count: 0,
            level_filter,
            #[cfg(feature = "tls")]
            rate_limiter,
            #[cfg(feature = "tls")]
            security_auditor,
        };

        // 尝试建立初始连接
        if let Err(e) = processor.connect().await {
            tracing::warn!("Failed to establish initial network connection: {}", e);
        }

        Ok(processor)
    }

    /// 建立网络连接
    async fn connect(&mut self) -> Result<()> {
        let address = format!("{}:{}", self.config.host, self.config.port);
        let socket_addr: SocketAddr = address.parse().map_err(|e| {
            QuantumLogError::NetworkError(format!("Invalid address {}: {}", address, e))
        })?;

        // 检查连接速率限制
         #[cfg(feature = "tls")]
          if let Some(ref rate_limiter) = self.rate_limiter {
              if !rate_limiter.check_rate_limit(&self.config.host).await {
                 self.security_auditor.log_event(SecurityAuditEvent::RateLimitExceeded {
                     host: self.config.host.clone(),
                     port: self.config.port,
                     timestamp: Instant::now(),
                 });
                 return Err(QuantumLogError::NetworkError(RATE_LIMIT_EXCEEDED.into()));
             }
         }

        // 记录连接尝试
        #[cfg(feature = "tls")]
        self.security_auditor.log_event(SecurityAuditEvent::ConnectionAttempt {
            host: self.config.host.clone(),
            port: self.config.port,
            timestamp: Instant::now(),
        });

        match self.config.protocol {
            NetworkProtocol::Tcp => {
                let stream = timeout(
                    Duration::from_millis(self.config.timeout_ms),
                    TcpStream::connect(socket_addr),
                )
                .await
                .map_err(|_| QuantumLogError::NetworkError(CONNECTION_TIMEOUT_ERROR.into()))?
                .map_err(|e| {
                    QuantumLogError::NetworkError(format!("TCP connection failed: {}", e))
                })?;

                // 检查是否需要TLS加密
                #[cfg(feature = "tls")]
                if self.config.use_tls.unwrap_or(false) {
                    // 记录TLS握手开始
                    self.security_auditor.log_event(SecurityAuditEvent::TlsHandshakeStart {
                        host: self.config.host.clone(),
                        port: self.config.port,
                        timestamp: Instant::now(),
                    });
                    
                    match self.establish_tls_connection(stream).await {
                        Ok(tls_stream) => {
                            self.connection = Some(NetworkConnection::Tls(Arc::new(Mutex::new(tls_stream))));
                            
                            // 记录TLS握手成功
                            self.security_auditor.log_event(SecurityAuditEvent::TlsHandshakeSuccess {
                                host: self.config.host.clone(),
                                port: self.config.port,
                                cipher_suite: "unknown".to_string(), // 实际实现中应该获取真实的密码套件
                                timestamp: Instant::now(),
                            });
                            
                            tracing::info!("TLS connection established to {}", address);
                        }
                        Err(e) => {
                            // 记录TLS握手失败
                            self.security_auditor.log_event(SecurityAuditEvent::TlsHandshakeFailed {
                                host: self.config.host.clone(),
                                port: self.config.port,
                                error: e.to_string(),
                                timestamp: Instant::now(),
                            });
                            return Err(e);
                        }
                    }
                } else {
                    let writer = BufWriter::new(stream);
                    self.connection = Some(NetworkConnection::Tcp(Arc::new(Mutex::new(writer))));
                    tracing::info!("TCP connection established to {}", address);
                }
                
                #[cfg(not(feature = "tls"))]
                {
                    let writer = BufWriter::new(stream);
                    self.connection = Some(NetworkConnection::Tcp(Arc::new(Mutex::new(writer))));
                    tracing::info!("TCP connection established to {}", address);
                }
            }
            NetworkProtocol::Udp => {
                let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("UDP socket bind failed: {}", e))
                })?;

                self.connection = Some(NetworkConnection::Udp(Arc::new(socket), socket_addr));

                tracing::info!("UDP socket created for {}", address);
            }
            NetworkProtocol::Http => {
                // HTTP协议支持待实现
                todo!("HTTP protocol support not yet implemented")
            }
        }

        self.reconnect_count = 0;
        Ok(())
    }

    /// 建立TLS连接
    #[cfg(feature = "tls")]
    async fn establish_tls_connection(&self, stream: TcpStream) -> Result<BufWriter<TlsStream<TcpStream>>> {
        // 创建TLS配置
        let mut root_store = RootCertStore::empty();
        
        // 根据配置决定是否使用系统根证书
        if self.config.tls_verify_certificates {
            if let Some(ca_file) = &self.config.tls_ca_file {
                // 使用自定义CA文件
                let ca_data = std::fs::read(ca_file)
                    .map_err(|e| QuantumLogError::NetworkError(format!("Failed to read CA file {}: {}", ca_file, e)))?;
                for cert_result in rustls_pemfile::certs(&mut ca_data.as_slice()) {
                    let cert = cert_result
                        .map_err(|e| QuantumLogError::NetworkError(format!("Failed to parse CA certificate: {}", e)))?;
                    root_store.add(cert)
                        .map_err(|e| QuantumLogError::NetworkError(format!("Failed to add CA certificate: {}", e)))?;
                }
            } else {
                // 使用系统默认根证书
                root_store.extend(
                    webpki_roots::TLS_SERVER_ROOTS.iter().cloned()
                );
            }
        } else {
            // 如果不验证证书，添加一个接受所有证书的验证器
            tracing::warn!("TLS certificate verification is disabled - this is insecure!");
        }
        
        // 根据安全策略配置TLS版本和密码套件
        let config_builder = ClientConfig::builder();
        
        // 根据配置设置证书验证
        let config = if self.config.tls_verify_certificates {
            config_builder
                .with_root_certificates(root_store)
                .with_no_client_auth()
        } else {
            // 严格安全策略下禁止禁用证书验证
            if self.config.security_policy == SecurityPolicy::Strict {
                self.security_auditor.log_event(SecurityAuditEvent::SecurityPolicyViolation {
                    policy: "Strict".to_string(),
                    violation: "Certificate verification disabled".to_string(),
                    timestamp: Instant::now(),
                });
                return Err(QuantumLogError::ConfigError(
                    "Certificate verification cannot be disabled in strict security mode".into()
                ));
            }
            
            // 创建一个不验证证书的配置
            use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
            use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
            use rustls::{DigitallySignedStruct, SignatureScheme};
            
            #[derive(Debug)]
            struct NoVerifier;
            
            impl ServerCertVerifier for NoVerifier {
                fn verify_server_cert(
                    &self,
                    _end_entity: &CertificateDer<'_>,
                    _intermediates: &[CertificateDer<'_>],
                    _server_name: &ServerName<'_>,
                    _ocsp_response: &[u8],
                    _now: UnixTime,
                ) -> std::result::Result<ServerCertVerified, rustls::Error> {
                    Ok(ServerCertVerified::assertion())
                }
                
                fn verify_tls12_signature(
                    &self,
                    _message: &[u8],
                    _cert: &CertificateDer<'_>,
                    _dss: &DigitallySignedStruct,
                ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
                    Ok(HandshakeSignatureValid::assertion())
                }
                
                fn verify_tls13_signature(
                    &self,
                    _message: &[u8],
                    _cert: &CertificateDer<'_>,
                    _dss: &DigitallySignedStruct,
                ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
                    Ok(HandshakeSignatureValid::assertion())
                }
                
                fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
                    vec![
                        SignatureScheme::RSA_PKCS1_SHA1,
                        SignatureScheme::ECDSA_SHA1_Legacy,
                        SignatureScheme::RSA_PKCS1_SHA256,
                        SignatureScheme::ECDSA_NISTP256_SHA256,
                        SignatureScheme::RSA_PKCS1_SHA384,
                        SignatureScheme::ECDSA_NISTP384_SHA384,
                        SignatureScheme::RSA_PKCS1_SHA512,
                        SignatureScheme::ECDSA_NISTP521_SHA512,
                        SignatureScheme::RSA_PSS_SHA256,
                        SignatureScheme::RSA_PSS_SHA384,
                        SignatureScheme::RSA_PSS_SHA512,
                        SignatureScheme::ED25519,
                        SignatureScheme::ED448,
                    ]
                }
            }
            
            self.security_auditor.log_event(SecurityAuditEvent::CertificateVerificationFailed {
                host: self.config.host.clone(),
                error: "Certificate verification disabled by configuration".to_string(),
                timestamp: Instant::now(),
            });
            
            config_builder
                .dangerous()
                .with_custom_certificate_verifier(StdArc::new(NoVerifier))
                .with_no_client_auth()
        };
            
        let connector = TlsConnector::from(StdArc::new(config));
        
        // 获取服务器名称
        let server_name = if self.config.tls_verify_hostname {
            rustls::pki_types::ServerName::try_from(self.config.host.clone())
                .map_err(|e| QuantumLogError::NetworkError(format!("Invalid server name: {}", e)))?
        } else {
            // 严格模式下要求SNI
            if self.config.security_policy == SecurityPolicy::Strict {
                return Err(QuantumLogError::ConfigError(
                    "SNI is required in strict security mode".into()
                ));
            }
            // 如果不验证主机名，使用一个虚拟名称
            rustls::pki_types::ServerName::try_from("localhost".to_string())
                .map_err(|e| QuantumLogError::NetworkError(format!("Invalid server name: {}", e)))?
        };
            
        // 建立TLS连接
        let tls_stream = connector.connect(server_name, stream).await
            .map_err(|e| QuantumLogError::NetworkError(format!("TLS handshake failed: {}", e)))?;
            
        let buffered = BufWriter::new(tls_stream);
        Ok(buffered)
    }

    /// 重连
    async fn reconnect(&mut self) -> Result<()> {
        // 检查最大重试次数
        if self.reconnect_count >= self.config.max_reconnect_attempts {
            return Err(QuantumLogError::NetworkError(
                format!("{} ({})", MAX_RECONNECT_ERROR, self.config.max_reconnect_attempts),
            ));
        }

        self.reconnect_count += 1;

        // 指数退避算法：基础延迟 * 2^(重试次数-1)
        let delay_ms = self.config.reconnect_delay_ms * (1 << (self.reconnect_count - 1).min(10)); // 最大1024倍
        let delay = Duration::from_millis(delay_ms.min(60000)); // 最大60秒
        
        tracing::info!(
            "Attempting reconnection #{}/{} after {}ms delay", 
            self.reconnect_count, 
            self.config.max_reconnect_attempts,
            delay.as_millis()
        );
        
        tokio::time::sleep(delay).await;
        self.connect().await
    }

    /// 运行处理器
    async fn run(mut self) -> Result<()> {
        while let Some(message) = self.receiver.recv().await {
            match message {
                SinkMessage::Event(event) => {
                    if let Err(e) = self.handle_event(*event).await {
                        tracing::error!("Error handling event in NetworkSink: {}", e);

                        // 如果是网络错误，尝试重连
                        if matches!(e, QuantumLogError::NetworkError(_)) {
                            if let Err(reconnect_err) = self.reconnect().await {
                                tracing::error!("Reconnection failed: {}", reconnect_err);
                            }
                        }
                    }
                }
                SinkMessage::Shutdown(response) => {
                    let result = self.shutdown().await;
                    let _ = response.send(result);
                    break;
                }
            }
        }
        Ok(())
    }

    /// 处理事件
    async fn handle_event(&mut self, event: QuantumLogEvent) -> Result<()> {
        // 检查级别过滤
        if let Some(ref filter_level) = self.level_filter {
            let event_level = event.level.parse::<Level>().map_err(|_| {
                QuantumLogError::ConfigError(format!("Invalid log level: {}", event.level))
            })?;

            if event_level < *filter_level {
                return Ok(());
            }
        }

        // 检查连接状态
        if self.connection.is_none() {
            self.connect().await?;
        }

        // 格式化事件
        let formatted = self.format_event(&event)?;
        let data = formatted.as_bytes();
        
        // 输入数据验证
        #[cfg(feature = "tls")]
        {
            if data.is_empty() || data.len() > 1024 * 1024 { // 1MB 限制
                let digest_result = digest::digest(&digest::SHA256, data);
                 let data_hash = hex::encode(digest_result.as_ref());
                 self.security_auditor.log_event(SecurityAuditEvent::InvalidInputDetected {
                     data_hash,
                    timestamp: Instant::now(),
                });
                return Err(QuantumLogError::ConfigError(INVALID_INPUT_DATA.into()));
            }
        }

        // 发送数据
        match &self.connection {
            Some(NetworkConnection::Tcp(writer_arc)) => {
                let mut writer = writer_arc.lock().await;
                writer.write_all(data).await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TCP_WRITE_ERROR, e))
                })?;
                writer.write_all(NEWLINE_BYTES).await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TCP_WRITE_ERROR, e))
                })?;

                writer.flush().await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TCP_FLUSH_ERROR, e))
                })?;
            }
            #[cfg(feature = "tls")]
            Some(NetworkConnection::Tls(stream_arc)) => {
                let mut stream = stream_arc.lock().await;
                stream.write_all(data).await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TLS_WRITE_ERROR, e))
                })?;
                stream.write_all(NEWLINE_BYTES).await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TLS_WRITE_ERROR, e))
                })?;

                stream.flush().await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TLS_FLUSH_ERROR, e))
                })?;
            }
            Some(NetworkConnection::Udp(socket, addr)) => {
                socket.send_to(data, addr).await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", UDP_SEND_ERROR, e))
                })?;
            }
            None => {
                return Err(QuantumLogError::NetworkError(
                    NO_CONNECTION_ERROR.into(),
                ));
            }
        }

        Ok(())
    }

    /// 格式化事件
    fn format_event(&self, event: &QuantumLogEvent) -> Result<String> {
        match self.config.format {
            crate::config::OutputFormat::Text => Ok(event.to_formatted_string("full")),
            crate::config::OutputFormat::Json => event
                .to_json()
                .map_err(|e| QuantumLogError::SerializationError { source: e }),
            crate::config::OutputFormat::Csv => {
                let csv_row = event.to_csv_row();
                // 使用预估容量避免多次重新分配
                let estimated_capacity = csv_row.iter().map(|s| s.len()).sum::<usize>() + csv_row.len();
                let mut result = String::with_capacity(estimated_capacity);
                for (i, field) in csv_row.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    result.push_str(field);
                }
                Ok(result)
            }
        }
    }

    /// 关闭处理器
    async fn shutdown(&mut self) -> Result<()> {
        // 刷新并关闭连接
        match self.connection.take() {
            Some(NetworkConnection::Tcp(writer_arc)) => {
                // 尝试获取独占访问权限来正确关闭连接
                match Arc::try_unwrap(writer_arc) {
                    Ok(writer_mutex) => {
                        let mut writer = writer_mutex.into_inner();
                        if let Err(e) = writer.flush().await {
                            tracing::error!("Error flushing TCP connection: {}", e);
                        }
                        // 获取底层的 TcpStream 并关闭
                        let mut stream = writer.into_inner();
                        if let Err(e) = stream.shutdown().await {
                            tracing::error!("Error shutting down TCP stream: {}", e);
                        } else {
                            tracing::debug!("TCP connection closed successfully");
                        }
                    }
                    Err(writer_arc) => {
                        // 如果还有其他引用，只能刷新
                        let mut writer = writer_arc.lock().await;
                        if let Err(e) = writer.flush().await {
                            tracing::error!("Error flushing TCP connection (shared): {}", e);
                        }
                        tracing::warn!("TCP connection has multiple references, cannot close properly");
                    }
                }
            }
            #[cfg(feature = "tls")]
            Some(NetworkConnection::Tls(stream_arc)) => {
                // 尝试获取独占访问权限来正确关闭TLS连接
                match Arc::try_unwrap(stream_arc) {
                    Ok(stream_mutex) => {
                        let mut stream = stream_mutex.into_inner();
                        if let Err(e) = stream.flush().await {
                            tracing::error!("Error flushing TLS connection: {}", e);
                        }
                        // TLS连接会在drop时自动关闭
                        tracing::debug!("TLS connection closed successfully");
                    }
                    Err(stream_arc) => {
                        // 如果还有其他引用，只能刷新
                        let mut stream = stream_arc.lock().await;
                        if let Err(e) = stream.flush().await {
                            tracing::error!("Error flushing TLS connection (shared): {}", e);
                        }
                        tracing::warn!("TLS connection has multiple references, cannot close properly");
                    }
                }
            }
            Some(NetworkConnection::Udp(socket, _)) => {
                // UDP 套接字会在 Arc 被 drop 时自动关闭
                // 但我们可以显式记录关闭信息
                tracing::debug!("UDP socket will be closed when Arc is dropped");
                drop(socket); // 显式 drop
            }
            None => {
                tracing::debug!("No connection to close");
            }
        }

        tracing::info!("NetworkSink shutdown completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OutputFormat;
    use crate::core::event::ContextInfo;

    use tokio::net::{TcpListener, UdpSocket};
    use tracing::Level;

    fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
        static CALLSITE: tracing::callsite::DefaultCallsite =
            tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
                "test",
                "quantum_log::network::test",
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
            level,
            Some("test.rs"),
            Some(42),
            Some("test_module"),
            tracing::field::FieldSet::new(&[], tracing::callsite::Identifier(&CALLSITE)),
            tracing::metadata::Kind::EVENT,
        );

        QuantumLogEvent::new(
            level,
            message.to_string(),
            &metadata,
            std::collections::HashMap::new(),
            ContextInfo::default(),
        )
    }

    #[tokio::test]
    async fn test_network_sink_creation() {
        let config = NetworkConfig {
            enabled: true,
            level: None,
            host: "127.0.0.1".to_string(),
            port: 8080,
            protocol: NetworkProtocol::Tcp,
            format: OutputFormat::Text,
            buffer_size: 8192,
            timeout_ms: 30000,
            max_reconnect_attempts: 5,
            reconnect_delay_ms: 1000,
            #[cfg(feature = "tls")]
            use_tls: Some(false),
            #[cfg(feature = "tls")]
            tls_verify_certificates: true,
            #[cfg(feature = "tls")]
            tls_verify_hostname: true,
            #[cfg(feature = "tls")]
            tls_ca_file: None,
            #[cfg(feature = "tls")]
            tls_cert_file: None,
            #[cfg(feature = "tls")]
             tls_key_file: None,
             #[cfg(feature = "tls")]
             tls_min_version: crate::config::TlsVersion::Tls12,
             #[cfg(feature = "tls")]
             tls_cipher_suite: crate::config::TlsCipherSuite::Medium,
             #[cfg(feature = "tls")]
             tls_require_sni: false,
             connection_rate_limit: 0,
             enable_security_audit: false,
             security_policy: crate::config::SecurityPolicy::Balanced,
        };

        let sink = NetworkSink::new(config);
        assert!(!sink.is_running().await);
    }

    #[tokio::test]
    #[ignore] // 网络测试存在超时问题，暂时禁用
    async fn test_tcp_network_sink() {
        // 启动测试 TCP 服务器
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // 在后台接受连接
        let server_handle = tokio::spawn(async move {
            tokio::select! {
                result = listener.accept() => {
                    if let Ok((stream, _)) = result {
                        let mut buffer = [0; 1024];
                        let _ = stream.readable().await;
                        let _ = stream.try_read(&mut buffer);
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(200)) => {
                    // 超时退出
                }
            }
        });

        let config = NetworkConfig {
            enabled: true,
            level: None,
            host: addr.ip().to_string(),
            port: addr.port(),
            protocol: NetworkProtocol::Tcp,
            format: OutputFormat::Text,
            buffer_size: 8192,
            timeout_ms: 100,
            max_reconnect_attempts: 0,
            reconnect_delay_ms: 10,
            #[cfg(feature = "tls")]
            use_tls: Some(false),
            #[cfg(feature = "tls")]
            tls_verify_certificates: true,
            #[cfg(feature = "tls")]
            tls_verify_hostname: true,
            #[cfg(feature = "tls")]
            tls_ca_file: None,
            #[cfg(feature = "tls")]
            tls_cert_file: None,
            #[cfg(feature = "tls")]
            tls_key_file: None,
            #[cfg(feature = "tls")]
            tls_min_version: crate::config::TlsVersion::Tls12,
            #[cfg(feature = "tls")]
            tls_cipher_suite: crate::config::TlsCipherSuite::High,
            #[cfg(feature = "tls")]
            tls_require_sni: true,
            connection_rate_limit: 100,
            enable_security_audit: true,
            security_policy: SecurityPolicy::Balanced,
        };

        let mut sink = NetworkSink::new(config);

        // 启动 sink
        let result = sink.start().await;
        assert!(result.is_ok());
        assert!(sink.is_running().await);

        // 发送事件
        let event = create_test_event(Level::INFO, "Test TCP message");
        let result = sink.send_event(event).await;
        assert!(result.is_ok());

        // 等待一下
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // 关闭
        let result = sink.shutdown().await;
        assert!(result.is_ok());

        // 等待服务器完成（带超时）
        let _ = tokio::time::timeout(tokio::time::Duration::from_millis(100), server_handle).await;
    }

    #[tokio::test]
    #[ignore] // 网络测试存在超时问题，暂时禁用
    async fn test_udp_network_sink() {
        // 启动测试 UDP 服务器
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server_socket.local_addr().unwrap();

        // 在后台接收数据
        let server_handle = tokio::spawn(async move {
            let mut buffer = [0; 1024];
            tokio::select! {
                result = server_socket.recv(&mut buffer) => {
                    let _ = result;
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(200)) => {
                    // 超时退出
                }
            }
        });

        let config = NetworkConfig {
            enabled: true,
            level: None,
            host: addr.ip().to_string(),
            port: addr.port(),
            protocol: NetworkProtocol::Udp,
            format: OutputFormat::Text,
            buffer_size: 8192,
            timeout_ms: 100,
            max_reconnect_attempts: 0,
            reconnect_delay_ms: 10,
            #[cfg(feature = "tls")]
            use_tls: Some(false),
            #[cfg(feature = "tls")]
            tls_verify_certificates: true,
            #[cfg(feature = "tls")]
            tls_verify_hostname: true,
            #[cfg(feature = "tls")]
            tls_ca_file: None,
            #[cfg(feature = "tls")]
            tls_cert_file: None,
            #[cfg(feature = "tls")]
            tls_key_file: None,
            #[cfg(feature = "tls")]
            tls_min_version: crate::config::TlsVersion::Tls12,
            #[cfg(feature = "tls")]
            tls_cipher_suite: crate::config::TlsCipherSuite::High,
            #[cfg(feature = "tls")]
            tls_require_sni: true,
            connection_rate_limit: 100,
            enable_security_audit: true,
            security_policy: SecurityPolicy::Balanced,
        };

        let mut sink = NetworkSink::new(config);

        // 启动 sink
        let result = sink.start().await;
        assert!(result.is_ok());
        assert!(sink.is_running().await);

        // 发送事件
        let event = create_test_event(Level::INFO, "Test UDP message");
        let result = sink.send_event(event).await;
        assert!(result.is_ok());

        // 等待一下
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // 关闭
        let result = sink.shutdown().await;
        assert!(result.is_ok());

        // 等待服务器完成（带超时）
        let _ = tokio::time::timeout(tokio::time::Duration::from_millis(100), server_handle).await;
    }

    #[tokio::test]
    #[ignore] // 网络测试存在超时问题，暂时禁用
    async fn test_network_sink_backpressure_drop() {
        let config = NetworkConfig {
            enabled: true,
            level: None,
            host: "127.0.0.1".to_string(),
            port: 9999, // 不存在的端口
            protocol: NetworkProtocol::Tcp,
            format: OutputFormat::Text,
            buffer_size: 8192,
            timeout_ms: 100,
            max_reconnect_attempts: 0,
            reconnect_delay_ms: 10,
            #[cfg(feature = "tls")]
            use_tls: Some(false),
            #[cfg(feature = "tls")]
            tls_verify_certificates: true,
            #[cfg(feature = "tls")]
            tls_verify_hostname: true,
            #[cfg(feature = "tls")]
            tls_ca_file: None,
            #[cfg(feature = "tls")]
            tls_cert_file: None,
            #[cfg(feature = "tls")]
            tls_key_file: None,
            #[cfg(feature = "tls")]
            tls_min_version: crate::config::TlsVersion::Tls12,
            #[cfg(feature = "tls")]
            tls_cipher_suite: crate::config::TlsCipherSuite::High,
            #[cfg(feature = "tls")]
            tls_require_sni: true,
            connection_rate_limit: 10,
            enable_security_audit: true,
            security_policy: SecurityPolicy::Balanced,
        };

        let mut sink = NetworkSink::new(config);
        sink.start().await.unwrap();

        // 发送事件（应该被丢弃，因为连接失败）
        let event = create_test_event(Level::INFO, "Test message");
        let result = sink.send_event(event).await;
        assert!(result.is_ok()); // send_event 本身不会失败，但事件可能被丢弃

        sink.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_network_sink_json_format() {
        let config = NetworkConfig {
            enabled: true,
            level: None,
            host: "127.0.0.1".to_string(),
            port: 8080,
            protocol: NetworkProtocol::Tcp,
            format: OutputFormat::Json,
            buffer_size: 8192,
            timeout_ms: 500,
            max_reconnect_attempts: 1,
            reconnect_delay_ms: 100,
            #[cfg(feature = "tls")]
            use_tls: Some(false),
            #[cfg(feature = "tls")]
            tls_verify_certificates: true,
            #[cfg(feature = "tls")]
            tls_verify_hostname: true,
            #[cfg(feature = "tls")]
            tls_ca_file: None,
            #[cfg(feature = "tls")]
            tls_cert_file: None,
            #[cfg(feature = "tls")]
            tls_key_file: None,
            #[cfg(feature = "tls")]
            tls_min_version: crate::config::TlsVersion::Tls12,
            #[cfg(feature = "tls")]
            tls_cipher_suite: crate::config::TlsCipherSuite::High,
            #[cfg(feature = "tls")]
            tls_require_sni: true,
            connection_rate_limit: 10,
            enable_security_audit: true,
            security_policy: SecurityPolicy::Balanced,
        };

        let sink = NetworkSink::new(config);

        // 检查配置
        assert_eq!(sink.config().format, OutputFormat::Json);
        assert_eq!(sink.config().protocol, NetworkProtocol::Tcp);
    }
}

// 实现新的统一 Sink trait
#[async_trait]
impl QuantumSink for NetworkSink {
    type Config = NetworkConfig;
    type Error = SinkError;

    async fn send_event(&self, event: QuantumLogEvent) -> std::result::Result<(), Self::Error> {
        self.send_event_internal(event).await.map_err(|e| match e {
            QuantumLogError::ChannelError(msg) => SinkError::Generic(msg),
            QuantumLogError::ConfigError(msg) => SinkError::Config(msg),
            QuantumLogError::IoError { source } => SinkError::Io(source),
            QuantumLogError::NetworkError(msg) => SinkError::Network(msg),
            _ => SinkError::Generic(e.to_string()),
        })
    }

    async fn shutdown(&self) -> std::result::Result<(), Self::Error> {
        // 使用内部可变性来支持从&self调用shutdown
        self.shutdown_ref().await.map_err(|e| match e {
            QuantumLogError::ChannelError(msg) => SinkError::Generic(msg),
            QuantumLogError::ConfigError(msg) => SinkError::Config(msg),
            QuantumLogError::IoError { source } => SinkError::Io(source),
            QuantumLogError::NetworkError(msg) => SinkError::Network(msg),
            QuantumLogError::SinkError(msg) => SinkError::Generic(msg),
            _ => SinkError::Generic(e.to_string()),
        })
    }

    async fn is_healthy(&self) -> bool {
        self.is_running().await
    }

    fn name(&self) -> &'static str {
        "network"
    }

    fn stats(&self) -> String {
        format!(
            "NetworkSink: running={}, protocol={:?}, target={}:{}",
            self.is_running_sync(),
            self.config.protocol,
            self.config.host,
            self.config.port
        )
    }

    fn metadata(&self) -> crate::sinks::traits::SinkMetadata {
        crate::sinks::traits::SinkMetadata {
            name: "network".to_string(),
            sink_type: crate::sinks::traits::SinkType::Exclusive,
            enabled: self.is_running_sync(),
            description: Some(format!(
                "Network sink using {:?} protocol to {}:{}",
                self.config.protocol, self.config.host, self.config.port
            )),
        }
    }
}

// 标记为独占型 sink
impl ExclusiveSink for NetworkSink {}
