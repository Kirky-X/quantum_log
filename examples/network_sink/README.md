# Network Sink Examples

这个目录包含网络传输相关的示例。

## 示例文件

- `tls_network_example.rs` - TLS网络传输示例，展示安全的网络日志传输

## 运行示例

```bash
# 运行TLS网络示例
cargo run --bin tls_network_example
```

## 功能说明

Network Sink 用于将日志通过网络传输，支持：
- TCP/UDP协议
- TLS加密传输
- 自动重连
- 网络缓冲