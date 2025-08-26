# InfluxDB Sink Examples

这个目录包含InfluxDB数据库相关的示例。

## 示例文件

- `influxdb_example.rs` - InfluxDB基本使用示例
- `influxdb_integration.rs` - InfluxDB集成示例
- `influxdb_example.toml` - InfluxDB配置文件示例

## 运行示例

```bash
# 运行InfluxDB基本示例
cargo run --bin influxdb_example --features database

# 运行InfluxDB集成示例
cargo run --bin influxdb_integration --features database
```

## 功能说明

InfluxDB Sink 用于将日志存储到InfluxDB时序数据库，支持：
- 时序数据存储
- 标签和字段映射
- 批量写入
- 数据压缩
- 查询和分析