# QuantumLog - 量子级高性能日志系统

QuantumLog 是一个专为 Rust 生态系统设计的高性能异步日志库，支持多种输出目标，提供灵活的配置选项和优秀的性能表现。

## ✨ 核心特性

- **🚀 异步高性能**: 基于 tokio 的完全异步架构，支持高并发日志写入
- **🎯 多输出目标**: 支持控制台、文件、数据库、自定义 Sink 等多种输出方式
- **⚡ 零拷贝优化**: 智能缓冲和批量写入，最小化性能开销
- **🔄 文件分离策略**: 支持按日期、大小、小时等多种文件分离方式
- **💾 数据库集成**: 内置 SQLite、PostgreSQL、MySQL 支持
- **🌐 MPI 并行支持**: 原生支持 MPI 环境，适用于高性能计算场景
- **📊 结构化日志**: 完整支持 `tracing` 生态系统的结构化日志记录
- **🛡️ 错误恢复**: 内置错误处理和自动重试机制
- **⚙️ 灵活配置**: 支持 TOML、YAML、JSON 等多种配置格式
- **🔧 热重载**: 支持运行时配置更新，无需重启应用

## 📦 安装

在您的 `Cargo.toml` 中添加：

```toml
[dependencies]
quantum_log = "0.3.0"

# 可选特性
quantum_log = { version = "0.3.0", features = ["database", "mpi_support"] }
```

## 0.3.0 变更说明

- **zero dead_code 警告**: 清理所有编译告警，提供最干净的构建体验
- **MPI 动态加载**: 支持运行时动态加载 MPI 库，提高部署灵活性
- **完善的文档更新**: 统一 feature 名称，修正所有示例代码
- **增强的错误处理**: 更精确的错误分类和处理机制

> 迁移提示（MPI 动态加载）：自 0.3.0 起，运行时代码不再读取自定义 `MPI_LIB_PATH` 变量；请使用平台标准环境变量（`LD_LIBRARY_PATH`/`PATH`/`DYLD_LIBRARY_PATH`）覆盖或指定库路径。`MPI_LIB_PATH` 仅作为构建阶段的提示性输出，运行时不依赖该变量。

### 🔧 启用 MPI 动态加载

QuantumLog 支持运行时动态加载 MPI 库，无需编译时链接 MPI 库文件。启用此功能：

```toml
[dependencies.quantum_log]
version = "0.3.0"
features = ["mpi_support", "dynamic_mpi"]
```

**动态加载特性**：
- **运行时检测**: 程序启动时自动检测系统中可用的 MPI 库
- **跨平台支持**: 自动适配不同操作系统的库文件命名规范
  - Linux: `libmpi.so`, `libmpi.so.12`, `libmpi.so.40`
  - Windows: `mpi.dll`
  - macOS: `libmpi.dylib`
- **部署灵活**: 无需在构建环境安装 MPI 开发包，简化部署流程
- **降级兼容**: 当 MPI 库不可用时自动禁用 MPI 功能，程序正常运行

**使用示例**：
```rust
use quantum_log::init_quantum_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handle = init_quantum_logger().await?;
    
    // MPI 功能会根据运行时环境自动启用或禁用
    tracing::info!("程序启动，MPI 支持状态由运行时环境决定");
    
    handle.shutdown().await?;
    Ok(())
}
```

**库查找配置**：
系统会按以下顺序查找 MPI 动态库：
1. **标准系统路径**: 系统默认的库搜索路径
2. **环境变量路径**: `LD_LIBRARY_PATH` (Linux), `PATH` (Windows), `DYLD_LIBRARY_PATH` (macOS)
3. **构建时检测路径**: 构建期间检测到的常见 MPI 安装路径
   - `/usr/lib/x86_64-linux-gnu/openmpi/lib`
   - `/usr/lib64/openmpi/lib`
   - `/opt/intel/oneapi/mpi/latest/lib`
   - `/usr/local/lib`

如需指定特定的 MPI 库路径，可设置相应的系统环境变量。

**注意**: 使用动态加载时，确保目标系统已安装兼容的 MPI 运行时环境。

**环境变量与路径覆盖**：
- 运行时动态加载优先遵循系统库搜索路径与上述库名，当前实现不直接读取自定义 `MPI_LIB_PATH` 环境变量；
- 若需覆盖或指定特定库路径，建议在启动前配置：
  - Linux: `export LD_LIBRARY_PATH=/path/to/mpi/lib:$LD_LIBRARY_PATH`
  - macOS: `export DYLD_LIBRARY_PATH=/path/to/mpi/lib:$DYLD_LIBRARY_PATH`
  - Windows (PowerShell): `$env:PATH = "C:\\Path\\to\\MPI\\bin;" + $env:PATH`
- 构建阶段如检测到常见安装目录，会通过 `MPI_LIB_PATH` 输出构建环境信息，但运行时不依赖该变量。

**平台特定指引**：
- Linux（推荐 OpenMPI 或 MPICH）：
  - Ubuntu/Debian: `sudo apt-get install libopenmpi-dev openmpi-bin` 或 `sudo apt-get install mpich`
  - CentOS/RHEL: `sudo yum install openmpi openmpi-devel` 或 `sudo yum install mpich`
- Windows（MS-MPI）：
  - 安装 Microsoft MPI（MS-MPI）Runtime 与 SDK，并确保 `mpi.dll` 所在目录在 `PATH` 中；
  - 常见路径示例：`C:\\Program Files\\Microsoft MPI\\Bin`。
- macOS（Homebrew OpenMPI）：
  - `brew install open-mpi`，并将 `$(brew --prefix)/lib` 加入 `DYLD_LIBRARY_PATH`（如需）。
