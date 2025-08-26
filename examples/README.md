# QuantumLog Examples

这是QuantumLog的示例和测试集合，展示了各种Sink的使用方法和最佳实践。

## 目录结构

```
examples/
├── console_sink/     # 控制台输出示例
├── file_sink/        # 文件输出示例
├── network_sink/     # 网络传输示例
├── csv_sink/         # CSV格式输出示例
├── influxdb_sink/    # InfluxDB数据库示例
├── security/         # 安全功能示例
├── config/           # 配置文件示例
├── trait_usage/      # Sink Trait使用示例
├── test_examples.rs  # 测试运行器
├── run_examples.sh   # Linux/macOS运行脚本
└── run_examples.ps1  # Windows运行脚本
```

## 快速开始

### 运行所有示例

```bash
# Linux/macOS
./run_examples.sh

# Windows
.\run_examples.ps1

# 或者使用Rust测试运行器
cargo run --bin test_examples
```

### 运行单个示例

```bash
# 基本使用示例
cargo run --bin basic_usage

# 文件输出示例
cargo run --bin complete_examples

# TLS网络传输示例
cargo run --bin tls_network_example --features tls

# InfluxDB示例（需要启用database特性）
cargo run --bin influxdb_example --features database
```

## 特性说明

- `default`: 基本功能
- `database`: 启用InfluxDB支持
- `mpi_support`: 启用MPI支持
- `tls`: 启用TLS加密传输

## 示例分类

### 按Sink类型

每个目录专门展示一种Sink的使用：

- **Console Sink**: 控制台输出，适合开发调试
- **File Sink**: 文件输出，支持滚动和压缩
- **Network Sink**: 网络传输，支持TLS加密
- **CSV Sink**: 结构化数据输出
- **InfluxDB Sink**: 时序数据库存储

### 按功能类型

- **Security**: 安全功能演示
- **Config**: 配置文件使用
- **Trait Usage**: 自定义Sink开发

## 测试和验证

所有示例都经过测试验证，确保：
- 代码可以正常编译
- 运行时无错误
- 功能符合预期
- 性能满足要求

## 贡献指南

添加新示例时请：
1. 选择合适的目录或创建新目录
2. 添加到Cargo.toml的[[bin]]配置中
3. 创建对应的README.md说明
4. 更新test_examples.rs测试列表