//! QuantumLog 构建脚本
//!
//! 此脚本负责：
//! 1. 检测 MPI 环境并生成相应的绑定
//! 2. 设置条件编译标志
//! 3. 链接必要的系统库

use chrono::Utc;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // 检查是否启用了 MPI 支持
    let mpi_support = cfg!(feature = "mpi");
    let dynamic_mpi = cfg!(feature = "dynamic_mpi");

    if mpi_support {
        setup_mpi_support(dynamic_mpi);
    }

    // 设置平台特定的配置
    setup_platform_specific();

    // 输出构建信息
    println!(
        "cargo:rustc-env=QUANTUM_LOG_BUILD_TIME={}",
        Utc::now().to_rfc3339()
    );
    println!(
        "cargo:rustc-env=QUANTUM_LOG_VERSION={}",
        env!("CARGO_PKG_VERSION")
    );
}

/// 设置 MPI 支持
fn setup_mpi_support(dynamic_mpi: bool) {
    println!("cargo:rustc-cfg=mpi_enabled");

    if dynamic_mpi {
        // 动态加载 MPI
        println!("cargo:rustc-cfg=dynamic_mpi");
        setup_dynamic_mpi();
    } else {
        // 静态链接 MPI
        setup_static_mpi();
    }
}

/// 设置动态 MPI 加载
fn setup_dynamic_mpi() {
    // 对于动态加载，我们不需要在构建时链接 MPI
    // 只需要确保 libloading 依赖可用
    println!("cargo:rustc-cfg=mpi_dynamic");

    // 检查常见的 MPI 库路径
    let mpi_paths = [
        "/usr/lib/x86_64-linux-gnu/openmpi/lib",
        "/usr/lib64/openmpi/lib",
        "/opt/intel/oneapi/mpi/latest/lib",
        "/usr/local/lib",
    ];

    for path in &mpi_paths {
        if std::path::Path::new(path).exists() {
            println!("cargo:rustc-env=MPI_LIB_PATH={}", path);
            break;
        }
    }
}

/// 设置静态 MPI 链接
fn setup_static_mpi() {
    // 使用 pkg-config 查找 MPI
    if let Ok(mpi) = pkg_config::Config::new()
        .atleast_version("3.0")
        .probe("ompi")
    {
        // OpenMPI 找到了
        println!("cargo:rustc-cfg=mpi_openmpi");
        for lib_path in &mpi.link_paths {
            println!("cargo:rustc-link-search=native={}", lib_path.display());
        }
        for lib in &mpi.libs {
            println!("cargo:rustc-link-lib={}", lib);
        }
    } else if let Ok(mpi) = pkg_config::Config::new()
        .atleast_version("3.0")
        .probe("mpich")
    {
        // MPICH 找到了
        println!("cargo:rustc-cfg=mpi_mpich");
        for lib_path in &mpi.link_paths {
            println!("cargo:rustc-link-search=native={}", lib_path.display());
        }
        for lib in &mpi.libs {
            println!("cargo:rustc-link-lib={}", lib);
        }
    } else {
        // 尝试手动查找 MPI
        if let Some(mpi_home) = env::var_os("MPI_HOME") {
            let mpi_path = PathBuf::from(mpi_home);
            let lib_path = mpi_path.join("lib");
            let include_path = mpi_path.join("include");

            if lib_path.exists() && include_path.exists() {
                println!("cargo:rustc-link-search=native={}", lib_path.display());
                println!("cargo:rustc-link-lib=mpi");
                println!("cargo:rustc-cfg=mpi_manual");
            } else {
                println!("cargo:warning=MPI_HOME set but libraries not found");
            }
        } else {
            println!("cargo:warning=MPI not found. MPI features will be disabled at runtime.");
        }
    }
}

/// 设置平台特定的配置
fn setup_platform_specific() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    match target_os.as_str() {
        "linux" => {
            setup_linux_specific();
        }
        "windows" => {
            setup_windows_specific();
        }
        "macos" => {
            setup_macos_specific();
        }
        _ => {
            println!("cargo:warning=Unsupported target OS: {}", target_os);
        }
    }
}

/// Linux 特定设置
fn setup_linux_specific() {
    println!("cargo:rustc-cfg=target_linux");

    // 检查 gettid 系统调用支持
    if let Ok(output) = std::process::Command::new("getconf")
        .arg("_POSIX_VERSION")
        .output()
    {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            if let Ok(version_num) = version.trim().parse::<i32>() {
                if version_num >= 200809 {
                    println!("cargo:rustc-cfg=has_gettid");
                }
            }
        }
    }

    // 链接 pthread
    println!("cargo:rustc-link-lib=pthread");
}

/// Windows 特定设置
fn setup_windows_specific() {
    println!("cargo:rustc-cfg=target_windows");

    // Windows API 库
    println!("cargo:rustc-link-lib=kernel32");
    println!("cargo:rustc-link-lib=user32");
    println!("cargo:rustc-link-lib=advapi32");
}

/// macOS 特定设置
fn setup_macos_specific() {
    println!("cargo:rustc-cfg=target_macos");

    // macOS 系统框架
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        // 测试版本信息生成
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
    }

    #[test]
    fn test_git_hash() {
        // 测试 Git 哈希获取（可能失败，这是正常的）
        let hash = get_git_hash();
        if let Some(h) = hash {
            assert!(!h.is_empty());
        }
    }
}
