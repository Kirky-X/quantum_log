# File Sink Examples

这个目录包含文件输出相关的示例。

## 示例文件

- `complete_examples.rs` - 完整的文件日志示例，展示各种文件输出功能

## 运行示例

```bash
# 运行完整示例
cargo run --bin complete_examples
```

## 功能说明

File Sink 用于将日志输出到文件，支持：
- 文件滚动
- 按级别分文件
- 文件压缩
- 自动清理旧文件