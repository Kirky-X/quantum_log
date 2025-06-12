//! MPI FFI 接口和安全包装器
//!
//! 此模块提供了与 MPI 库的 FFI 接口，支持编译时链接和运行时动态加载两种方式。
//! 主要功能包括获取 MPI Rank 号和检查 MPI 初始化状态。

use crate::error::{QuantumLogError, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

/// MPI 初始化状态的全局标记
static MPI_CHECKED: Once = Once::new();
static MPI_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// MPI 常量定义
#[allow(dead_code)]
const MPI_SUCCESS: i32 = 0;
#[allow(dead_code)]
const MPI_ERR_OTHER: i32 = 15;

/// MPI 函数指针类型定义
#[allow(dead_code)]
type MpiInitializedFn = unsafe extern "C" fn(*mut i32) -> i32;
#[allow(dead_code)]
type MpiCommRankFn = unsafe extern "C" fn(i32, *mut i32) -> i32;

/// MPI 动态库句柄（仅在 dynamic_mpi feature 启用时使用）
#[cfg(feature = "dynamic_mpi")]
static mut MPI_LIB: Option<libloading::Library> = None;

/// MPI 函数指针（仅在 dynamic_mpi feature 启用时使用）
#[cfg(feature = "dynamic_mpi")]
static mut MPI_INITIALIZED_FN: Option<MpiInitializedFn> = None;
#[cfg(feature = "dynamic_mpi")]
static mut MPI_COMM_RANK_FN: Option<MpiCommRankFn> = None;

/// 编译时链接的 MPI 函数声明
#[cfg(all(feature = "mpi_support", not(feature = "dynamic_mpi")))]
extern "C" {
    fn MPI_Initialized(flag: *mut i32) -> i32;
    fn MPI_Comm_rank(comm: i32, rank: *mut i32) -> i32;
}

/// MPI_COMM_WORLD 常量
#[allow(dead_code)]
const MPI_COMM_WORLD: i32 = 0x44000000;

/// 检查 MPI 是否可用
///
/// 此函数会检查 MPI 库是否可用，并缓存结果。
/// 对于动态加载模式，会尝试加载 MPI 库。
/// 对于编译时链接模式，会检查 MPI 是否已初始化。
pub fn is_mpi_available() -> bool {
    MPI_CHECKED.call_once(|| {
        let available = check_mpi_availability();
        MPI_AVAILABLE.store(available, Ordering::Relaxed);
    });
    MPI_AVAILABLE.load(Ordering::Relaxed)
}

/// 内部函数：检查 MPI 可用性
fn check_mpi_availability() -> bool {
    #[cfg(feature = "dynamic_mpi")]
    {
        load_mpi_library().is_ok()
    }
    #[cfg(all(feature = "mpi_support", not(feature = "dynamic_mpi")))]
    {
        check_mpi_initialized().unwrap_or(false)
    }
    #[cfg(all(not(feature = "mpi_support"), not(feature = "dynamic_mpi")))]
    {
        false
    }
}

/// 动态加载 MPI 库（仅在 dynamic_mpi feature 启用时）
#[cfg(feature = "dynamic_mpi")]
fn load_mpi_library() -> Result<()> {
    unsafe {
        if MPI_LIB.is_some() {
            return Ok(());
        }

        // 尝试加载不同的 MPI 库
        let lib_names = [
            "libmpi.so",      // Linux - OpenMPI/MPICH
            "libmpi.so.12",   // Linux - OpenMPI specific version
            "libmpi.so.40",   // Linux - MPICH specific version
            "mpi.dll",        // Windows - Microsoft MPI
            "libmpi.dylib",   // macOS
        ];

        let mut last_error = None;
        for lib_name in &lib_names {
            match libloading::Library::new(lib_name) {
                Ok(lib) => {
                    // 尝试获取函数指针
                    match (
                        lib.get::<MpiInitializedFn>(b"MPI_Initialized"),
                        lib.get::<MpiCommRankFn>(b"MPI_Comm_rank"),
                    ) {
                        (Ok(init_fn), Ok(rank_fn)) => {
                            MPI_INITIALIZED_FN = Some(*init_fn);
                            MPI_COMM_RANK_FN = Some(*rank_fn);
                            MPI_LIB = Some(lib);
                            return Ok(());
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            last_error = Some(QuantumLogError::LibLoadingError { source: e });
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(QuantumLogError::LibLoadingError { source: e });
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            QuantumLogError::mpi("无法加载任何 MPI 库")
        }))
    }
}

/// 检查 MPI 是否已初始化
fn check_mpi_initialized() -> Result<bool> {
    #[cfg(feature = "dynamic_mpi")]
    {
        unsafe {
            if let Some(mpi_initialized_fn) = MPI_INITIALIZED_FN {
                let mut flag: i32 = 0;
                let result = mpi_initialized_fn(&mut flag as *mut i32);
                if result == MPI_SUCCESS {
                    Ok(flag != 0)
                } else {
                    Err(QuantumLogError::mpi(format!(
                        "MPI_Initialized 调用失败，错误码: {}",
                        result
                    )))
                }
            } else {
                Err(QuantumLogError::mpi("MPI_Initialized 函数未加载"))
            }
        }
    }
    #[cfg(all(feature = "mpi_support", not(feature = "dynamic_mpi")))]
    {
        unsafe {
            let mut flag: i32 = 0;
            let result = MPI_Initialized(&mut flag as *mut i32);
            if result == MPI_SUCCESS {
                Ok(flag != 0)
            } else {
                Err(QuantumLogError::mpi(format!(
                    "MPI_Initialized 调用失败，错误码: {}",
                    result
                )))
            }
        }
    }
    #[cfg(all(not(feature = "mpi_support"), not(feature = "dynamic_mpi")))]
    {
        Ok(false)
    }
}

/// 获取当前进程的 MPI Rank 号
///
/// 返回当前进程在 MPI_COMM_WORLD 中的 Rank 号。
/// 如果 MPI 不可用或未初始化，返回 None。
pub fn get_mpi_rank() -> Option<i32> {
    if !is_mpi_available() {
        return None;
    }

    match check_mpi_initialized() {
        Ok(true) => get_mpi_rank_internal().ok(),
        Ok(false) => None, // MPI 未初始化
        Err(_) => None,    // 检查失败
    }
}

/// 内部函数：获取 MPI Rank
fn get_mpi_rank_internal() -> Result<i32> {
    #[cfg(feature = "dynamic_mpi")]
    {
        unsafe {
            if let Some(mpi_comm_rank_fn) = MPI_COMM_RANK_FN {
                let mut rank: i32 = -1;
                let result = mpi_comm_rank_fn(MPI_COMM_WORLD, &mut rank as *mut i32);
                if result == MPI_SUCCESS {
                    Ok(rank)
                } else {
                    Err(QuantumLogError::mpi(format!(
                        "MPI_Comm_rank 调用失败，错误码: {}",
                        result
                    )))
                }
            } else {
                Err(QuantumLogError::mpi("MPI_Comm_rank 函数未加载"))
            }
        }
    }
    #[cfg(all(feature = "mpi_support", not(feature = "dynamic_mpi")))]
    {
        unsafe {
            let mut rank: i32 = -1;
            let result = MPI_Comm_rank(MPI_COMM_WORLD, &mut rank as *mut i32);
            if result == MPI_SUCCESS {
                Ok(rank)
            } else {
                Err(QuantumLogError::mpi(format!(
                    "MPI_Comm_rank 调用失败，错误码: {}",
                    result
                )))
            }
        }
    }
    #[cfg(all(not(feature = "mpi_support"), not(feature = "dynamic_mpi")))]
    {
        Err(QuantumLogError::mpi("MPI 支持未启用"))
    }
}

/// 获取 MPI Rank 号的字符串表示
///
/// 如果 MPI 可用且已初始化，返回 Rank 号的字符串形式。
/// 否则返回 "N/A"。
pub fn get_mpi_rank_string() -> String {
    match get_mpi_rank() {
        Some(rank) => rank.to_string(),
        None => "N/A".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mpi_availability_check() {
        // 测试 MPI 可用性检查不会 panic
        let _available = is_mpi_available();
        // 多次调用应该返回相同结果（缓存测试）
        let available1 = is_mpi_available();
        let available2 = is_mpi_available();
        assert_eq!(available1, available2);
    }

    #[test]
    fn test_mpi_rank_when_unavailable() {
        // 当 MPI 不可用时，应该返回 None
        if !is_mpi_available() {
            assert_eq!(get_mpi_rank(), None);
            assert_eq!(get_mpi_rank_string(), "N/A");
        }
    }

    #[test]
    fn test_mpi_rank_string_format() {
        let rank_str = get_mpi_rank_string();
        // 应该是数字或 "N/A"
        assert!(rank_str == "N/A" || rank_str.parse::<i32>().is_ok());
    }

    #[test]
    fn test_multiple_rank_calls() {
        // 多次调用应该返回一致的结果
        let rank1 = get_mpi_rank();
        let rank2 = get_mpi_rank();
        assert_eq!(rank1, rank2);
    }

    #[test]
    fn test_rank_string_consistency() {
        // 字符串版本应该与数值版本一致
        let rank = get_mpi_rank();
        let rank_str = get_mpi_rank_string();
        
        match rank {
            Some(r) => assert_eq!(rank_str, r.to_string()),
            None => assert_eq!(rank_str, "N/A"),
        }
    }
}