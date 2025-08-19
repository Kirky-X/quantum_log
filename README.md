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
quantum_log = "0.3.1"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"

# 可选特性（示例）
[dependencies.quantum_log]
version = "0.3.1"
features = ["database", "mpi_support"]  # 启用数据库与 MPI 支持
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

### 使用设计文档推荐的主 API

```rust
use quantum_log::init_quantum_logger;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 使用设计文档推荐的主 API
    let shutdown_handle = init_quantum_logger().await?;

    info!("正在使用 QuantumLog 记录日志");
    warn!("警告信息");
    error!("错误信息");

    // 使用返回的句柄优雅关闭
    shutdown_handle.shutdown().await?;
    Ok(())
}
```

## 🆕 0.3.1 变更日志

QuantumLog 0.3.1 带来更强大的功能与更好的稳定性，主要变更：

**🔒 安全加固**
- 数据库连接字符串脱敏：防止敏感信息泄露
- 文件权限安全检查：确保日志文件访问权限正确
- 缓冲区溢出保护：增强内存安全性
- 网络传输加密支持：提升数据传输安全性

**⚡ 性能优化**
- 减少字符串分配和克隆操作
- 优化HashMap转换性能
- 改进数据库操作效率
- 优化文件路径处理

**🛠️ 代码质量改进**
- 修复QuantumLoggerConfig字段缺失问题
- 修复PipelineBuilder导入问题
- 移除未使用的导入和变量
- 所有测试用例通过验证

> 迁移提示（MPI 动态加载）：自 0.3.1 起，运行时代码不再读取自定义 `MPI_LIB_PATH`。请使用平台标准环境变量（`LD_LIBRARY_PATH`/`PATH`/`DYLD_LIBRARY_PATH`）覆盖或指定库路径。`MPI_LIB_PATH` 仅在构建阶段作为信息展示，运行时不依赖该变量。

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

# 运行示例
cargo run --example basic_usage
cargo run --example complete_examples
cargo run --example config_file_example
```

## 📝 许可证

本项目基于 Apache-2.0 许可证发布。详见 [LICENSE](LICENSE)。

## 🤝 贡献

欢迎贡献！请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解如何参与项目开发。

## 📞 支持

如果你遇到问题或有建议，请：

1. 查看 [在线文档](https://docs.rs/quantum_log)
2. 在 GitHub 上搜索或创建 [Issue](https://github.com/Kirky-X/quantum_log/issues)
3. 参与 [Discussions](https://github.com/Kirky-X/quantum_log/discussions)
