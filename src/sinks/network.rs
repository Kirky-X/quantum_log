//! 网络 Sink
//!
//! 此模块实现通过网络发送日志的 sink，支持 TCP 和 UDP 协议。

use crate::config::{NetworkConfig, NetworkProtocol};
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};
use crate::sinks::traits::{ExclusiveSink, QuantumSink, SinkError};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use tracing::Level;

// 预定义错误消息常量以避免运行时字符串分配
const TCP_WRITE_ERROR: &str = "TCP write failed";
const TCP_FLUSH_ERROR: &str = "TCP flush failed";
const TLS_WRITE_ERROR: &str = "TLS write failed";
const TLS_FLUSH_ERROR: &str = "TLS flush failed";
const UDP_SEND_ERROR: &str = "UDP send failed";
const NO_CONNECTION_ERROR: &str = "No active connection";
const SINK_ALREADY_STARTED: &str = "Sink already started";
const CONNECTION_TIMEOUT_ERROR: &str = "Connection timeout";
const MAX_RECONNECT_ERROR: &str = "Max reconnection attempts (5) exceeded";
const NETWORK_SINK_NOT_STARTED: &str = "NetworkSink not started";
const SHUTDOWN_SIGNAL_FAILED: &str = "Failed to send shutdown signal";
const SHUTDOWN_SIGNAL_LOST: &str = "Shutdown signal lost";
const NEWLINE_BYTES: &[u8] = b"\n";

#[cfg(feature = "tls")]
use tokio_rustls::{TlsConnector, client::TlsStream};
#[cfg(feature = "tls")]
use rustls::{ClientConfig, RootCertStore};
#[cfg(feature = "tls")]
use std::sync::Arc as StdArc;

/// 网络 Sink
#[derive(Debug)]
pub struct NetworkSink {
    /// 配置
    config: NetworkConfig,
    /// 事件发送器
    sender: Option<mpsc::Sender<SinkMessage>>,
    /// 处理器句柄
    processor_handle: Option<JoinHandle<()>>,
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
}

impl NetworkSink {
    /// 创建新的网络 sink
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config,
            sender: None,
            processor_handle: None,
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
        if self.sender.is_some() {
            return Err(QuantumLogError::ConfigError(
                SINK_ALREADY_STARTED.into(),
            ));
        }

        let buffer_size = 1000; // 固定缓冲区大小

        let (sender, receiver) = mpsc::channel(buffer_size);

        let processor = NetworkSinkProcessor::new(self.config.clone(), receiver).await?;
        let handle = tokio::spawn(async move {
            if let Err(e) = processor.run().await {
                tracing::error!("NetworkSink processor error: {}", e);
            }
        });

        self.sender = Some(sender);
        self.processor_handle = Some(handle);

        Ok(())
    }

    /// 发送事件
    pub async fn send_event_internal(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(sender) = &self.sender {
            let message = SinkMessage::Event(Box::new(event));

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

    /// 关闭 sink
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(sender) = self.sender.take() {
            let (tx, rx) = oneshot::channel();

            // 发送关闭信号
            if sender.send(SinkMessage::Shutdown(tx)).await.is_err() {
                return Err(QuantumLogError::SinkError(
                    SHUTDOWN_SIGNAL_FAILED.into(),
                ));
            }

            // 等待关闭完成
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
        if let Some(handle) = self.processor_handle.take() {
            if let Err(e) = handle.await {
                tracing::error!("Error waiting for NetworkSink processor: {}", e);
            }
        }

        Ok(())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.sender.is_some()
    }

    /// 获取配置
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
}

impl NetworkSinkProcessor {
    /// 创建新的处理器
    async fn new(config: NetworkConfig, receiver: mpsc::Receiver<SinkMessage>) -> Result<Self> {
        let mut processor = Self {
            config,
            receiver,
            connection: None,
            reconnect_count: 0,
            level_filter: None,
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

        match self.config.protocol {
            NetworkProtocol::Tcp => {
                let stream = timeout(
                    Duration::from_millis(self.config.timeout_ms.unwrap_or(30000)),
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
                    let tls_stream = self.establish_tls_connection(stream).await?;
                    let writer = BufWriter::new(tls_stream);
                    self.connection = Some(NetworkConnection::Tls(Arc::new(Mutex::new(writer))));
                    tracing::info!("TLS connection established to {}", address);
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
    async fn establish_tls_connection(&self, stream: TcpStream) -> Result<TlsStream<TcpStream>> {
        // 创建TLS配置
        let mut root_store = RootCertStore::empty();
        root_store.add_server_trust_anchors(
            webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
                rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                    ta.subject,
                    ta.spki,
                    ta.name_constraints,
                )
            })
        );
        
        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();
            
        let connector = TlsConnector::from(StdArc::new(config));
        
        // 获取服务器名称
        let server_name = rustls::ServerName::try_from(self.config.host.as_str())
            .map_err(|e| QuantumLogError::NetworkError(format!("Invalid server name: {}", e)))?;
            
        // 建立TLS连接
        let tls_stream = connector.connect(server_name, stream).await
            .map_err(|e| QuantumLogError::NetworkError(format!("TLS handshake failed: {}", e)))?;
            
        Ok(tls_stream)
    }

    /// 重连
    async fn reconnect(&mut self) -> Result<()> {
        // 检查最大重试次数（固定为5次）
        if self.reconnect_count >= 5 {
            return Err(QuantumLogError::NetworkError(
                MAX_RECONNECT_ERROR.into(),
            ));
        }

        self.reconnect_count += 1;

        // 等待重连间隔
        tokio::time::sleep(Duration::from_secs(5)).await;

        tracing::info!("Attempting reconnection #{}", self.reconnect_count);
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
            Some(NetworkConnection::Tls(writer_arc)) => {
                let mut writer = writer_arc.lock().await;
                writer.write_all(data).await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TLS_WRITE_ERROR, e))
                })?;
                writer.write_all(NEWLINE_BYTES).await.map_err(|e| {
                    QuantumLogError::NetworkError(format!("{}: {}", TLS_WRITE_ERROR, e))
                })?;

                writer.flush().await.map_err(|e| {
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
            Some(NetworkConnection::Tls(writer_arc)) => {
                // 尝试获取独占访问权限来正确关闭TLS连接
                match Arc::try_unwrap(writer_arc) {
                    Ok(writer_mutex) => {
                        let mut writer = writer_mutex.into_inner();
                        if let Err(e) = writer.flush().await {
                            tracing::error!("Error flushing TLS connection: {}", e);
                        }
                        // 获取底层的 TlsStream 并关闭
                        let tls_stream = writer.into_inner();
                        let (_, tcp_stream) = tls_stream.into_inner();
                        if let Err(e) = tcp_stream.shutdown().await {
                            tracing::error!("Error shutting down TLS stream: {}", e);
                        } else {
                            tracing::debug!("TLS connection closed successfully");
                        }
                    }
                    Err(writer_arc) => {
                        // 如果还有其他引用，只能刷新
                        let mut writer = writer_arc.lock().await;
                        if let Err(e) = writer.flush().await {
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
            timeout_ms: Some(30000),
        };

        let sink = NetworkSink::new(config);
        assert!(!sink.is_running());
    }

    #[tokio::test]
    async fn test_tcp_network_sink() {
        // 启动测试 TCP 服务器
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // 在后台接受连接
        let server_handle = tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                let mut buffer = [0; 1024];
                let _ = stream.readable().await;
                let _ = stream.try_read(&mut buffer);
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
            timeout_ms: Some(5000),
        };

        let mut sink = NetworkSink::new(config);

        // 启动 sink
        let result = sink.start().await;
        assert!(result.is_ok());
        assert!(sink.is_running());

        // 发送事件
        let event = create_test_event(Level::INFO, "Test TCP message");
        let result = sink.send_event(event).await;
        assert!(result.is_ok());

        // 等待一下
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 关闭
        let result = sink.shutdown().await;
        assert!(result.is_ok());

        // 等待服务器完成
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_udp_network_sink() {
        // 启动测试 UDP 服务器
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = server_socket.local_addr().unwrap();

        // 在后台接收数据
        let server_handle = tokio::spawn(async move {
            let mut buffer = [0; 1024];
            let _ = server_socket.recv(&mut buffer).await;
        });

        let config = NetworkConfig {
            enabled: true,
            level: None,
            host: addr.ip().to_string(),
            port: addr.port(),
            protocol: NetworkProtocol::Udp,
            format: OutputFormat::Text,
            buffer_size: 8192,
            timeout_ms: Some(5000),
        };

        let mut sink = NetworkSink::new(config);

        // 启动 sink
        let result = sink.start().await;
        assert!(result.is_ok());
        assert!(sink.is_running());

        // 发送事件
        let event = create_test_event(Level::INFO, "Test UDP message");
        let result = sink.send_event(event).await;
        assert!(result.is_ok());

        // 等待一下
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // 关闭
        let result = sink.shutdown().await;
        assert!(result.is_ok());

        // 等待服务器完成
        let _ = server_handle.await;
    }

    #[tokio::test]
    async fn test_network_sink_backpressure_drop() {
        let config = NetworkConfig {
            enabled: true,
            level: None,
            host: "127.0.0.1".to_string(),
            port: 9999, // 不存在的端口
            protocol: NetworkProtocol::Tcp,
            format: OutputFormat::Text,
            buffer_size: 8192,
            timeout_ms: Some(1000),
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
            timeout_ms: Some(30000),
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
        // 注意：这里需要可变引用，但trait要求不可变引用
        // 在实际使用中，可能需要使用内部可变性或重新设计
        Err(SinkError::Generic(
            "NetworkSink shutdown requires mutable reference".to_string(),
        ))
    }

    async fn is_healthy(&self) -> bool {
        self.is_running()
    }

    fn name(&self) -> &'static str {
        "network"
    }

    fn stats(&self) -> String {
        format!(
            "NetworkSink: running={}, protocol={:?}, target={}:{}",
            self.is_running(),
            self.config.protocol,
            self.config.host,
            self.config.port
        )
    }

    fn metadata(&self) -> crate::sinks::traits::SinkMetadata {
        crate::sinks::traits::SinkMetadata {
            name: "network".to_string(),
            sink_type: crate::sinks::traits::SinkType::Exclusive,
            enabled: self.is_running(),
            description: Some(format!(
                "Network sink using {:?} protocol to {}:{}",
                self.config.protocol, self.config.host, self.config.port
            )),
        }
    }
}

// 标记为独占型 sink
impl ExclusiveSink for NetworkSink {}
