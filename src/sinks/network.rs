//! 网络 Sink
//!
//! 此模块实现通过网络发送日志的 sink，支持 TCP 和 UDP 协议。

use crate::config::{NetworkConfig, NetworkProtocol};
use crate::core::event::QuantumLogEvent;
use crate::error::{QuantumLogError, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};
use tracing::Level;

/// 网络 Sink
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
            return Err(QuantumLogError::ConfigError("Sink already started".to_string()));
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
    pub async fn send_event(&self, event: QuantumLogEvent) -> Result<()> {
        if let Some(sender) = &self.sender {
            let message = SinkMessage::Event(Box::new(event));
            
            // 使用阻塞策略发送事件
            sender.send(message).await
                .map_err(|_| QuantumLogError::SinkError("Failed to send event to NetworkSink".to_string()))?;
        } else {
            return Err(QuantumLogError::SinkError("NetworkSink not started".to_string()));
        }
        Ok(())
    }

    /// 关闭 sink
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(sender) = self.sender.take() {
            let (tx, rx) = oneshot::channel();
            
            // 发送关闭信号
            if sender.send(SinkMessage::Shutdown(tx)).await.is_err() {
                return Err(QuantumLogError::SinkError("Failed to send shutdown signal".to_string()));
            }
            
            // 等待关闭完成
            match rx.await {
                Ok(result) => result?,
                Err(_) => return Err(QuantumLogError::SinkError("Shutdown signal lost".to_string())),
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
        let socket_addr: SocketAddr = address.parse()
            .map_err(|e| QuantumLogError::NetworkError(format!("Invalid address {}: {}", address, e)))?;
        
        match self.config.protocol {
            NetworkProtocol::Tcp => {
                let stream = timeout(
                    Duration::from_millis(self.config.timeout_ms.unwrap_or(30000)),
                    TcpStream::connect(socket_addr)
                ).await
                .map_err(|_| QuantumLogError::NetworkError("Connection timeout".to_string()))?
                .map_err(|e| QuantumLogError::NetworkError(format!("TCP connection failed: {}", e)))?;
                
                let writer = BufWriter::new(stream);
                self.connection = Some(NetworkConnection::Tcp(Arc::new(Mutex::new(writer))));
                
                tracing::info!("TCP connection established to {}", address);
            },
            NetworkProtocol::Udp => {
                let socket = UdpSocket::bind("0.0.0.0:0").await
                    .map_err(|e| QuantumLogError::NetworkError(format!("UDP socket bind failed: {}", e)))?;
                
                self.connection = Some(NetworkConnection::Udp(Arc::new(socket), socket_addr));
                
                tracing::info!("UDP socket created for {}", address);
            },
            NetworkProtocol::Http => {
                // HTTP协议支持待实现
                todo!("HTTP protocol support not yet implemented")
            },
        }
        
        self.reconnect_count = 0;
        Ok(())
    }

    /// 重连
    async fn reconnect(&mut self) -> Result<()> {
        // 检查最大重试次数（固定为5次）
        if self.reconnect_count >= 5 {
            return Err(QuantumLogError::NetworkError(
                "Max reconnection attempts (5) exceeded".to_string()
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
                },
                SinkMessage::Shutdown(response) => {
                    let result = self.shutdown().await;
                    let _ = response.send(result);
                    break;
                },
            }
        }
        Ok(())
    }

    /// 处理事件
    async fn handle_event(&mut self, event: QuantumLogEvent) -> Result<()> {
        // 检查级别过滤
        if let Some(ref filter_level) = self.level_filter {
            let event_level = event.level.parse::<Level>()
                .map_err(|_| QuantumLogError::ConfigError(format!("Invalid log level: {}", event.level)))?;
            
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
                writer.write_all(data).await
                    .map_err(|e| QuantumLogError::NetworkError(format!("TCP write failed: {}", e)))?;
                writer.write_all(b"\n").await
                    .map_err(|e| QuantumLogError::NetworkError(format!("TCP write failed: {}", e)))?;
                
                writer.flush().await
                    .map_err(|e| QuantumLogError::NetworkError(format!("TCP flush failed: {}", e)))?;
            },
            Some(NetworkConnection::Udp(socket, addr)) => {
                socket.send_to(data, addr).await
                    .map_err(|e| QuantumLogError::NetworkError(format!("UDP send failed: {}", e)))?;
            },
            None => {
                return Err(QuantumLogError::NetworkError("No active connection".to_string()));
            },
        }
        
        Ok(())
    }

    /// 格式化事件
    fn format_event(&self, event: &QuantumLogEvent) -> Result<String> {
        match self.config.format {
            crate::config::OutputFormat::Text => Ok(event.to_formatted_string("full")),
            crate::config::OutputFormat::Json => {
                event.to_json()
                    .map_err(|e| QuantumLogError::SerializationError { source: e })
            },
            crate::config::OutputFormat::Csv => {
                let csv_row = event.to_csv_row();
                Ok(csv_row.join(","))
            },
        }
    }

    /// 关闭处理器
    async fn shutdown(&mut self) -> Result<()> {
        // 刷新连接
        if let Some(NetworkConnection::Tcp(writer_arc)) = &self.connection {
            let mut writer = writer_arc.lock().await;
            if let Err(e) = writer.flush().await {
                tracing::error!("Error flushing TCP connection: {}", e);
            }
        }
        
        self.connection = None;
        tracing::info!("NetworkSink shutdown completed");
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LogLevel, OutputFormat};
    use crate::core::event::ContextInfo;
    use std::net::SocketAddr;
    use tokio::net::{TcpListener, UdpSocket};
    use tracing::Level;

    fn create_test_event(level: Level, message: &str) -> QuantumLogEvent {
        static CALLSITE: tracing::callsite::DefaultCallsite = tracing::callsite::DefaultCallsite::new(&tracing::Metadata::new(
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
            if let Ok((mut stream, _)) = listener.accept().await {
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