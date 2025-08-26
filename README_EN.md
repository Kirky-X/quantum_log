# QuantumLog

[![Crates.io](https://img.shields.io/crates/v/quantum_log.svg)](https://crates.io/crates/quantum_log)
[![Documentation](https://docs.rs/quantum_log/badge.svg)](https://docs.rs/quantum_log)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://github.com/Kirky-X/quantum_log/actions/workflows/rust.yml/badge.svg)](https://github.com/Kirky-X/quantum_log/actions/workflows/rust.yml)

**[中文](README.md)** | **[Docs](https://docs.rs/quantum_log)**

**QuantumLog** is an asynchronous logging library designed for high-performance computing environments, supporting multiple output formats and targets including files, databases, and standard output. It provides powerful configuration options, graceful shutdown mechanisms, and detailed diagnostic information.

## 🚀 Features

- **Asynchronous High Performance**: Tokio-based asynchronous architecture supporting high-concurrency logging
- **Multiple Output Targets**: Support for stdout, files, databases, and other output methods
- **Flexible Configuration**: Support for TOML configuration files and programmatic configuration
- **Graceful Shutdown**: Complete shutdown mechanism ensuring no log loss
- **Diagnostic Information**: Built-in diagnostic system for monitoring logging system performance
- **MPI Support**: Optimized for high-performance computing environments with MPI support
- **Backpressure Handling**: Intelligent handling of log backpressure under high load
- **Structured Logging**: Support for structured logging and multiple output formats

## 📦 Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
quantum_log = "0.3.1"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"

# Optional features
[dependencies.quantum_log]
version = "0.3.1"
features = ["database", "mpi_support", "tls"]  # Enable database, MPI and TLS support
```

## 🎯 Quick Start

### Basic Usage

```rust
use quantum_log::{init, shutdown};
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize QuantumLog
    init().await?;
    
    // Use standard tracing macros
    info!("Application started");
    warn!("This is a warning");
    error!("This is an error");
    
    // Graceful shutdown
    shutdown().await?;
    Ok(())
}
```

### Using Design Document Recommended API

```rust
use quantum_log::init_quantum_logger;
use tracing::{info, warn, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use the main API from design document
    let shutdown_handle = init_quantum_logger().await?;
    
    info!("Logging with QuantumLog");
    warn!("Warning message");
    error!("Error message");
    
    // Use returned handle for graceful shutdown
    shutdown_handle.shutdown().await?;
    Ok(())
}
```

## 🆕 0.3.1 Change Log

QuantumLog 0.3.1 brings stronger features and better stability. Key changes:

**🔒 Security Hardening**
- Database connection string sanitization: prevent sensitive information leakage
- **TLS Network Encryption**: Comprehensive TLS/SSL encryption support
  - TLS certificate verification (server certificate and hostname verification)
  - Custom CA certificate files and client certificate authentication
  - Configurable TLS verification policies for enhanced network security
- Network connection resilience: configurable reconnection attempts and delays
- File permission security checks: ensure proper log file access permissions
- Buffer overflow protection: enhance memory safety
- Network transmission encryption support: improve data transmission security

**⚡ Performance Optimization**
- Reduce string allocation and cloning operations
- Optimize HashMap conversion performance
- Improve database operation efficiency
- Optimize file path processing

**🛠️ Code Quality Improvements**
- Fix QuantumLoggerConfig missing fields issue
- Fix PipelineBuilder import issue
- Remove unused imports and variables
- All test cases pass verification

> Migration Note (MPI dynamic loading): Starting from 0.3.1, the runtime loader does not read a custom `MPI_LIB_PATH`. Use platform-standard environment variables (`LD_LIBRARY_PATH`/`PATH`/`DYLD_LIBRARY_PATH`) to override or specify library paths. `MPI_LIB_PATH` is only surfaced during build for informational purposes and is not used at runtime.

### 🔧 Enable MPI Dynamic Loading

QuantumLog supports loading MPI libraries at runtime without linking them at compile time. Enable this feature:

```toml
[dependencies.quantum_log]
version = "0.3.0"
features = ["mpi_support", "dynamic_mpi"]
```

Highlights:
- Runtime detection: automatically detects available MPI libraries at startup
- Cross-platform support: adapts to OS-specific library names
  - Linux: `libmpi.so`, `libmpi.so.12`, `libmpi.so.40`
  - Windows: `mpi.dll`
  - macOS: `libmpi.dylib`
- Flexible deployment: no need to install MPI dev packages in build environments
- Graceful degradation: if MPI is unavailable, MPI features are disabled and the program continues to run

Example:
```rust
use quantum_log::init_quantum_logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handle = init_quantum_logger().await?;
    tracing::info!("Application started; MPI support is determined at runtime");
    handle.shutdown().await?;
    Ok(())
}
```

**Library Search Configuration**:
The system searches for MPI dynamic libraries in this order:
1. **Standard system paths**: default system library search paths
2. **Environment variable paths**: `LD_LIBRARY_PATH` (Linux), `PATH` (Windows), `DYLD_LIBRARY_PATH` (macOS)
3. **Build-time detected paths**: common MPI installation paths detected during build
   - `/usr/lib/x86_64-linux-gnu/openmpi/lib`
   - `/usr/lib64/openmpi/lib`
   - `/opt/intel/oneapi/mpi/latest/lib`
   - `/usr/local/lib`

To specify a custom MPI library path, set the appropriate system environment variables.

**Note**: Ensure a compatible MPI runtime is installed on the target system when using dynamic loading.

**Environment Variables and Path Overrides**:
- Runtime dynamic loading follows system library search paths and known library names; the current implementation does not directly read a custom `MPI_LIB_PATH` at runtime.
- To override or force a specific library location, configure before starting your program:
  - Linux: `export LD_LIBRARY_PATH=/path/to/mpi/lib:$LD_LIBRARY_PATH`
  - macOS: `export DYLD_LIBRARY_PATH=/path/to/mpi/lib:$DYLD_LIBRARY_PATH`
  - Windows (PowerShell): `$env:PATH = "C:\\Path\\to\\MPI\\bin;" + $env:PATH`
- During build, common installation directories may be detected and surfaced via `MPI_LIB_PATH` for informational purposes, but runtime loading does not rely on this variable.

**Platform-specific Guidance**:
- Linux (OpenMPI or MPICH recommended):
  - Ubuntu/Debian: `sudo apt-get install libopenmpi-dev openmpi-bin` or `sudo apt-get install mpich`
  - CentOS/RHEL: `sudo yum install openmpi openmpi-devel` or `sudo yum install mpich`
- Windows (MS-MPI):
  - Install Microsoft MPI (MS-MPI) Runtime and SDK, and ensure the directory containing `mpi.dll` is in `PATH`.
  - Common path example: `C:\\Program Files\\Microsoft MPI\\Bin`.
- macOS (Homebrew OpenMPI):
  - `brew install open-mpi`, and add `$(brew --prefix)/lib` to `DYLD_LIBRARY_PATH` if necessary.

## 🧪 Testing

Run tests:

```bash
# Run all tests
cargo test

# Run tests for specific features
cargo test --features database
cargo test --features mpi_support

# Run examples
cargo run --example basic_usage
cargo run --example complete_examples
cargo run --example config_file_example
```

## 📝 License

This project is licensed under the Apache--2.0 License. See the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) to learn how to participate in project development.

## 📞 Support

If you encounter issues or have suggestions, please:

1. Check the [documentation](https://docs.rs/quantum_log)
2. Search or create an [Issue](https://github.com/Kirky-X/quantum_log/issues)
3. Join the [discussion](https://github.com/Kirky-X/quantum_log/discussions)
