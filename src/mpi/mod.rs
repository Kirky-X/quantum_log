//! MPI FFI 相关模块
//!
//! 此模块提供了与 MPI (Message Passing Interface) 库的 FFI 接口，
//! 用于获取 MPI Rank 号和检查 MPI 初始化状态。

pub mod ffi;

pub use ffi::*;
