# QuantumLog

[![Crates.io](https://img.shields.io/crates/v/quantum_log.svg)](https://crates.io/crates/quantum_log)
[![Documentation](https://docs.rs/quantum_log/badge.svg)](https://docs.rs/quantum_log)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://github.com/Kirky-X/quantum_log/actions/workflows/rust.yml/badge.svg)](https://github.com/Kirky-X/quantum_log/actions/workflows/rust.yml)

**[English](README_EN.md)** | **[文档](https://docs.rs/quantum_log)**

**QuantumLog** 是一个为高性能计算场景打造的异步日志库，支持文件、标准输出、数据库等多种输出目标，提供灵活的配置选项、优雅的关闭机制以及详细的诊断能力。

## 🚀 核心特性

- **异步高性能**：基于 Tokio 的异步架构，支持高并发日志写入
- **多输出目标**：支持 stdout、文件、数据库等多种输出方式
- **灵活配置**：支持 TOML 配置与代码方式配置
- **优雅关闭**：完善的关闭机制，确保日志不丢失
- **诊断能力**：内置诊断信息，便于监控日志系统性能
- **MPI 支持**：面向 HPC 环境优化，支持 MPI
- **背压处理**：高负载下的智能背压处理
- **结构化日志**：支持结构化日志与多种输出格式

## 📦 安装

在你的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
quantum_log = "0.3.2"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"

# 可选特性（示例）
[dependencies.quantum_log]
version = "0.3.2"
features = ["database", "mpi_support", "tls"]  # 启用数据库、MPI 和 TLS 支持
```

## 🎯 快速开始

### 基础用法

```rust
use quantum_log::{init, shutdown};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 QuantumLog
    init().await?;

    // 使用标准 tracing 宏
    info!("应用启动");
    warn!("这是一个警告");
    error!("这是一个错误");

    // 优雅关闭
    shutdown().await?;
    Ok(())
}
```

### 使用带关闭句柄的初始化 API

```rust
use quantum_log::init_quantum_logger;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用带关闭句柄的初始化 API，适用于需要精确控制关闭时机的场景
    let shutdown_handle = init_quantum_logger().await?;

    info!("正在使用 QuantumLog 记录日志");
    warn!("警告信息");
    error!("错误信息");

    // 使用返回的句柄优雅关闭
    shutdown_handle.shutdown().await?;
    Ok(())
}
```

### API 选择指南

- **`init()`**: 简单初始化，适用于大多数应用场景，使用全局 `shutdown()` 函数关闭
- **`init_quantum_logger()`**: 返回关闭句柄，适用于需要精确控制关闭时机的场景
- **`init_with_config()`**: 使用自定义配置初始化，适用于需要特定配置的场景

## 🆕 0.3.2 变更日志

QuantumLog 0.3.2 带来更强大的功能与更好的稳定性，主要变更：

**📊 InfluxDB 时序数据库支持**
- **新增 InfluxDB Sink**：完整支持将日志写入 InfluxDB 时序数据库
  - 兼容 InfluxDB 1.x 和 2.x 版本
  - 支持 Token 认证和基本用户名/密码认证
  - 批量写入机制提高性能（可配置批次大小和刷新间隔）
  - 异步处理架构，支持自动重连和错误恢复
  - 结构化数据模型：measurement、tags、fields 完整映射
- **时序数据分析优化**：为 HPC 和监控场景提供专业的时序日志存储

**🏗️ 项目架构重构**
- **独立 Examples Crate**：将测试和示例代码重构为独立的 `examples` crate
  - 每个子目录专门负责单个 sink 的测试样例
  - 提供完整的编译和运行环境
  - 包含 console、file、database、influxdb、network 等完整示例
- **项目结构优化**：合并 `test/` 和 `examples/` 目录，提升代码组织性
- **文档系统完善**：更新 `.gitignore`，排除构建产物和设计文档

**🧪 测试框架改进**
- **完整测试覆盖**：所有 sink 类型的独立测试样例
- **集成测试增强**：InfluxDB、数据库、网络等模块的完整集成测试
- **示例代码验证**：确保所有示例代码可编译和运行
- **回归测试完善**：覆盖所有功能模块的回归测试套件

**🔒 安全加固**
- 数据库连接字符串脱敏：防止敏感信息泄露
- 文件权限安全检查：确保日志文件访问权限正确
- 缓冲区溢出保护：增强内存安全性
- **TLS网络传输加密**：全面支持TLS/SSL加密传输
  - 支持TLS证书验证（服务器证书和主机名验证）
  - 支持自定义CA证书文件和客户端证书认证
  - 可配置的TLS验证策略，提升网络传输安全性
- 网络连接重连机制优化：支持可配置的重连次数和延迟

**⚡ 性能优化**
- 减少字符串分配和克隆操作
- 优化HashMap转换性能
- 改进数据库操作效率
- 优化文件路径处理
- InfluxDB 批量写入性能优化

**🛠️ 代码质量改进**
- **网络模块TLS安全增强**：完整实现TLS加密传输功能
  - 新增TLS配置字段：`tls_verify_certificates`、`tls_verify_hostname`、`tls_ca_file`、`tls_cert_file`、`tls_key_file`
  - 实现自定义TLS验证器，支持灵活的证书验证策略
  - 优化网络重连机制：`max_reconnect_attempts`、`reconnect_delay_ms`
- **InfluxDB 模块实现**：完整的 InfluxDB sink 实现，支持企业级时序数据存储
- 修复QuantumLoggerConfig字段缺失问题
- 修复PipelineBuilder导入问题
- 移除未使用的导入和变量
- 所有测试用例通过验证

> 迁移提示（MPI 动态加载）：自 0.3.2 起，运行时代码不再读取自定义 `MPI_LIB_PATH`。请使用平台标准环境变量（`LD_LIBRARY_PATH`/`PATH`/`DYLD_LIBRARY_PATH`）覆盖或指定库路径。`MPI_LIB_PATH` 仅在构建阶段作为信息展示，运行时不依赖该变量。

### 🔧 启用 MPI 动态加载

QuantumLog 支持在运行时动态加载 MPI 库，无需在编译期进行静态/动态链接。启用方式：

```toml
[dependencies.quantum_log]
version = "0.3.0"
features = ["mpi_support", "dynamic_mpi"]
```

亮点：
- 运行时检测：启动时自动检测可用的 MPI 库
- 跨平台支持：适配不同系统的库文件命名
  - Linux: `libmpi.so`, `libmpi.so.12`, `libmpi.so.40`
  - Windows: `mpi.dll`
  - macOS: `libmpi.dylib`
- 灵活部署：构建环境无需安装 MPI 开发包
- 优雅降级：当 MPI 不可用时自动禁用相关功能，程序继续运行

示例：
```rust
use quantum_log::init_quantum_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handle = init_quantum_logger().await?;
    tracing::info!("程序已启动；MPI 支持由运行时环境决定");
    handle.shutdown().await?;
    Ok(())
}
```

**库查找配置**：
系统按以下顺序查找 MPI 动态库：
1. 标准系统路径：系统默认的库搜索路径
2. 环境变量路径：`LD_LIBRARY_PATH`（Linux）、`PATH`（Windows）、`DYLD_LIBRARY_PATH`（macOS）
3. 构建期检测到的常见安装路径：
   - `/usr/lib/x86_64-linux-gnu/openmpi/lib`
   - `/usr/lib64/openmpi/lib`
   - `/opt/intel/oneapi/mpi/latest/lib`
   - `/usr/local/lib`

如需指定自定义库路径，请配置相应的系统环境变量。

注意：使用动态加载时，请确保目标系统已安装兼容的 MPI 运行时。

**环境变量与路径覆盖**：
- 运行时动态加载遵循系统库搜索路径与常见库名；当前实现不直接读取自定义 `MPI_LIB_PATH` 运行时变量。
- 如需覆盖或指定特定库位置，请在启动前配置：
  - Linux: `export LD_LIBRARY_PATH=/path/to/mpi/lib:$LD_LIBRARY_PATH`
  - macOS: `export DYLD_LIBRARY_PATH=/path/to/mpi/lib:$DYLD_LIBRARY_PATH`
  - Windows（PowerShell）: `$env:PATH = "C:\\Path\\to\\MPI\\bin;" + $env:PATH`
- 构建阶段如检测到常见目录，会通过 `MPI_LIB_PATH` 输出构建环境信息，但运行时不依赖该变量。

**平台指引**：
- Linux（推荐 OpenMPI 或 MPICH）：
  - Ubuntu/Debian: `sudo apt-get install libopenmpi-dev openmpi-bin` 或 `sudo apt-get install mpich`
  - CentOS/RHEL: `sudo yum install openmpi openmpi-devel` 或 `sudo yum install mpich`
- Windows（MS-MPI）：
  - 安装 Microsoft MPI（MS-MPI）Runtime 与 SDK，并确保包含 `mpi.dll` 的目录在 `PATH` 中
  - 常见路径示例：`C:\\Program Files\\Microsoft MPI\\Bin`
- macOS（Homebrew OpenMPI）：
  - `brew install open-mpi`，并在必要时将 `$(brew --prefix)/lib` 加入 `DYLD_LIBRARY_PATH`

## 🧪 测试

运行测试：

```bash
# 运行全部测试
cargo test

# 按特性运行测试
cargo test --features database
cargo test --features mpi_support
cargo test --features tls

# 运行示例
cargo run --example basic_usage
cargo run --example complete_examples
cargo run --example config_file_example
cargo run --example influxdb_example
```

## 📊 InfluxDB 支持

QuantumLog 支持将日志写入 InfluxDB，适用于需要时序数据分析的场景。

### 配置示例

```toml
# InfluxDB 配置
[influxdb]
enabled = true
level = "INFO"
url = "http://localhost:8086"
database = "quantum_logs"
# 对于 InfluxDB 2.x，使用 token 认证
# token = "your-influxdb-token"
# 对于 InfluxDB 1.x，使用用户名/密码认证
username = "quantum_user"
password = "quantum_password"
batch_size = 100
flush_interval_seconds = 5
use_https = false
verify_ssl = true
```

### 特性

- **批量写入**：通过批量处理提高写入性能
- **双版本支持**：兼容 InfluxDB 1.x 和 2.x
- **认证支持**：支持 Token 和基本认证
- **异步处理**：基于 Tokio 的异步架构
- **自动刷新**：定时刷新机制确保数据及时写入

### 数据模型

日志数据在 InfluxDB 中的存储结构：

- **Measurement**: 日志目标模块名
- **Tags**: 
  - `level`: 日志级别
  - `hostname`: 主机名
  - `thread`: 线程名
  - `mpi_rank`: MPI 排名（如果启用）
- **Fields**:
  - `message`: 日志消息
  - `file`: 文件路径
  - `line`: 行号
  - `module`: 模块路径
  - `username`: 用户名
  - 自定义字段

## 📝 许可证

本项目基于 Apache-2.0 许可证发布。详见 [LICENSE](LICENSE)。

## 🤝 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解如何参与项目开发。

## 📞 支持

如果你遇到问题或有建议，请：

1. 查看 [在线文档](https://docs.rs/quantum_log)
2. 在 GitHub 上搜索或创建 [Issue](https://github.com/Kirky-X/quantum_log/issues)
3. 参与 [Discussions](https://github.com/Kirky-X/quantum_log/discussions)
