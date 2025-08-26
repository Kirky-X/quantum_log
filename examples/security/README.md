# Security Examples

这个目录包含安全相关的示例。

## 示例文件

- `security_demo.rs` - 安全功能演示，展示数据脱敏、TLS加密等安全特性

## 运行示例

```bash
# 运行安全演示
cargo run --bin security_demo --features tls
```

## 功能说明

安全示例展示了QuantumLog的安全特性：
- 敏感数据脱敏
- TLS加密传输
- 文件权限检查
- 安全配置验证