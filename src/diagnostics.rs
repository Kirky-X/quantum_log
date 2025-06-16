//! 定义 QuantumLog (灵迹) 日志框架的内部诊断与指标。
//!
//! 此模块提供了对日志系统健康状况和性能的可观测性。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 内部诊断与指标数据结构。
///
/// 使用原子操作确保线程安全，提供日志系统运行时的关键指标。
#[derive(Debug, Default)]
pub struct Diagnostics {
    /// 系统启动时间
    start_time: Option<Instant>,
    
    /// 已处理的日志事件总数
    events_processed: AtomicU64,
    
    /// 因背压策略而丢弃的日志事件数
    events_dropped_backpressure: AtomicU64,
    
    /// 因错误而丢弃的日志事件数
    events_dropped_error: AtomicU64,
    
    /// 各个 Sink 的错误计数
    sink_errors: AtomicU64,
    
    /// 预初始化缓冲区中的事件数
    pre_init_buffer_events: AtomicU64,
    
    /// 预初始化缓冲区溢出次数
    pre_init_buffer_overflows: AtomicU64,
    
    /// 数据库批量写入次数
    database_batch_writes: AtomicU64,
    
    /// 文件写入次数
    file_writes: AtomicU64,
    
    /// 标准输出写入次数
    stdout_writes: AtomicU64,
}

/// 诊断数据的快照，用于外部查询。
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticsSnapshot {
    /// 系统运行时间
    pub uptime: Option<Duration>,
    
    /// 已处理的日志事件总数
    pub events_processed: u64,
    
    /// 因背压策略而丢弃的日志事件数
    pub events_dropped_backpressure: u64,
    
    /// 因错误而丢弃的日志事件数
    pub events_dropped_error: u64,
    
    /// 各个 Sink 的错误计数
    pub sink_errors: u64,
    
    /// 预初始化缓冲区中的事件数
    pub pre_init_buffer_events: u64,
    
    /// 预初始化缓冲区溢出次数
    pub pre_init_buffer_overflows: u64,
    
    /// 数据库批量写入次数
    pub database_batch_writes: u64,
    
    /// 文件写入次数
    pub file_writes: u64,
    
    /// 标准输出写入次数
    pub stdout_writes: u64,
    
    /// 总丢弃事件数（背压 + 错误）
    pub total_events_dropped: u64,
    
    /// 事件处理成功率（百分比）
    pub success_rate_percent: f64,
}

impl Diagnostics {
    /// 创建新的诊断实例。
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }
    
    /// 增加已处理事件计数。
    pub fn increment_events_processed(&self) {
        self.events_processed.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加因背压而丢弃的事件计数。
    pub fn increment_events_dropped_backpressure(&self) {
        self.events_dropped_backpressure.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加因错误而丢弃的事件计数。
    pub fn increment_events_dropped_error(&self) {
        self.events_dropped_error.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加 Sink 错误计数。
    pub fn increment_sink_errors(&self) {
        self.sink_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加预初始化缓冲区事件计数。
    pub fn increment_pre_init_buffer_events(&self) {
        self.pre_init_buffer_events.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加预初始化缓冲区溢出计数。
    pub fn increment_pre_init_buffer_overflows(&self) {
        self.pre_init_buffer_overflows.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加数据库批量写入计数。
    pub fn increment_database_batch_writes(&self) {
        self.database_batch_writes.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加文件写入计数。
    pub fn increment_file_writes(&self) {
        self.file_writes.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 增加标准输出写入计数。
    pub fn increment_stdout_writes(&self) {
        self.stdout_writes.fetch_add(1, Ordering::Relaxed);
    }
    
    /// 批量增加已处理事件计数。
    pub fn add_events_processed(&self, count: u64) {
        self.events_processed.fetch_add(count, Ordering::Relaxed);
    }
    
    /// 获取诊断数据的快照。
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let events_processed = self.events_processed.load(Ordering::Relaxed);
        let events_dropped_backpressure = self.events_dropped_backpressure.load(Ordering::Relaxed);
        let events_dropped_error = self.events_dropped_error.load(Ordering::Relaxed);
        let total_events_dropped = events_dropped_backpressure + events_dropped_error;
        
        let success_rate_percent = if events_processed + total_events_dropped > 0 {
            (events_processed as f64 / (events_processed + total_events_dropped) as f64) * 100.0
        } else {
            100.0
        };
        
        DiagnosticsSnapshot {
            uptime: self.start_time.map(|start| start.elapsed()),
            events_processed,
            events_dropped_backpressure,
            events_dropped_error,
            sink_errors: self.sink_errors.load(Ordering::Relaxed),
            pre_init_buffer_events: self.pre_init_buffer_events.load(Ordering::Relaxed),
            pre_init_buffer_overflows: self.pre_init_buffer_overflows.load(Ordering::Relaxed),
            database_batch_writes: self.database_batch_writes.load(Ordering::Relaxed),
            file_writes: self.file_writes.load(Ordering::Relaxed),
            stdout_writes: self.stdout_writes.load(Ordering::Relaxed),
            total_events_dropped,
            success_rate_percent,
        }
    }
    
    /// 重置所有计数器（主要用于测试）。
    pub fn reset(&self) {
        self.events_processed.store(0, Ordering::Relaxed);
        self.events_dropped_backpressure.store(0, Ordering::Relaxed);
        self.events_dropped_error.store(0, Ordering::Relaxed);
        self.sink_errors.store(0, Ordering::Relaxed);
        self.pre_init_buffer_events.store(0, Ordering::Relaxed);
        self.pre_init_buffer_overflows.store(0, Ordering::Relaxed);
        self.database_batch_writes.store(0, Ordering::Relaxed);
        self.file_writes.store(0, Ordering::Relaxed);
        self.stdout_writes.store(0, Ordering::Relaxed);
    }
}

/// 全局诊断实例，使用 Arc 包装以支持多线程访问。
static GLOBAL_DIAGNOSTICS: std::sync::OnceLock<Arc<Diagnostics>> = std::sync::OnceLock::new();

/// 初始化全局诊断实例。
///
/// 此函数应在日志系统初始化时调用，只能调用一次。
pub fn init_diagnostics() -> Arc<Diagnostics> {
    GLOBAL_DIAGNOSTICS.get_or_init(|| Arc::new(Diagnostics::new())).clone()
}

/// 获取全局诊断实例的引用。
///
/// 如果诊断系统尚未初始化，返回 None。
pub fn get_diagnostics_instance() -> Option<Arc<Diagnostics>> {
    GLOBAL_DIAGNOSTICS.get().cloned()
}

/// 获取诊断数据快照
///
/// 这是设计文档中指定的主要诊断函数，直接返回当前的诊断状态快照。
/// 如果诊断系统尚未初始化，返回默认的快照。
///
/// # 返回值
///
/// 返回 `DiagnosticsSnapshot`，包含当前的诊断统计信息
///
/// # 示例
///
/// ```rust
/// use quantum_log::{init_quantum_logger, get_diagnostics};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let _handle = init_quantum_logger().await?;
///     
///     let diagnostics = get_diagnostics();
///     println!("Events processed: {}", diagnostics.events_processed);
///     
///     Ok(())
/// }
/// ```
pub fn get_diagnostics() -> DiagnosticsSnapshot {
    match GLOBAL_DIAGNOSTICS.get() {
        Some(diagnostics) => diagnostics.snapshot(),
        None => DiagnosticsSnapshot {
            uptime: None,
            events_processed: 0,
            events_dropped_backpressure: 0,
            events_dropped_error: 0,
            sink_errors: 0,
            pre_init_buffer_events: 0,
            pre_init_buffer_overflows: 0,
            database_batch_writes: 0,
            file_writes: 0,
            stdout_writes: 0,
            total_events_dropped: 0,
            success_rate_percent: 100.0,
        },
    }
}

/// 获取诊断数据的快照。
///
/// 这是一个便利函数，用于快速获取当前的诊断状态。
/// 如果诊断系统尚未初始化，返回默认的快照。
pub fn get_diagnostics_snapshot() -> DiagnosticsSnapshot {
    get_diagnostics()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_diagnostics_creation() {
        let diagnostics = Diagnostics::new();
        let snapshot = diagnostics.snapshot();
        
        assert!(snapshot.uptime.is_some());
        assert_eq!(snapshot.events_processed, 0);
        assert_eq!(snapshot.events_dropped_backpressure, 0);
        assert_eq!(snapshot.events_dropped_error, 0);
        assert_eq!(snapshot.sink_errors, 0);
        assert_eq!(snapshot.success_rate_percent, 100.0);
    }

    #[test]
    fn test_increment_operations() {
        let diagnostics = Diagnostics::new();
        
        diagnostics.increment_events_processed();
        diagnostics.increment_events_dropped_backpressure();
        diagnostics.increment_sink_errors();
        
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.events_processed, 1);
        assert_eq!(snapshot.events_dropped_backpressure, 1);
        assert_eq!(snapshot.sink_errors, 1);
        assert_eq!(snapshot.total_events_dropped, 1);
    }

    #[test]
    fn test_success_rate_calculation() {
        let diagnostics = Diagnostics::new();
        
        // 处理 8 个事件，丢弃 2 个
        diagnostics.add_events_processed(8);
        diagnostics.increment_events_dropped_backpressure();
        diagnostics.increment_events_dropped_error();
        
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.events_processed, 8);
        assert_eq!(snapshot.total_events_dropped, 2);
        assert_eq!(snapshot.success_rate_percent, 80.0);
    }

    #[test]
    fn test_reset_functionality() {
        let diagnostics = Diagnostics::new();
        
        diagnostics.increment_events_processed();
        diagnostics.increment_sink_errors();
        
        let snapshot_before = diagnostics.snapshot();
        assert_eq!(snapshot_before.events_processed, 1);
        assert_eq!(snapshot_before.sink_errors, 1);
        
        diagnostics.reset();
        
        let snapshot_after = diagnostics.snapshot();
        assert_eq!(snapshot_after.events_processed, 0);
        assert_eq!(snapshot_after.sink_errors, 0);
    }

    #[test]
    fn test_concurrent_access() {
        let diagnostics = Arc::new(Diagnostics::new());
        let mut handles = vec![];
        
        // 启动多个线程同时增加计数器
        for _ in 0..10 {
            let diagnostics_clone = diagnostics.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    diagnostics_clone.increment_events_processed();
                }
            });
            handles.push(handle);
        }
        
        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
        
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.events_processed, 1000);
    }

    #[test]
    fn test_uptime_measurement() {
        let diagnostics = Diagnostics::new();
        
        // 等待一小段时间
        thread::sleep(Duration::from_millis(10));
        
        let snapshot = diagnostics.snapshot();
        assert!(snapshot.uptime.is_some());
        assert!(snapshot.uptime.unwrap() >= Duration::from_millis(10));
    }

    #[test]
    fn test_global_diagnostics_initialization() {
        // 注意：这个测试可能会影响其他测试，因为全局状态是共享的
        let diagnostics1 = init_diagnostics();
        let diagnostics2 = init_diagnostics();
        
        // 应该返回同一个实例
        assert!(Arc::ptr_eq(&diagnostics1, &diagnostics2));
        
        // 测试获取函数
        let retrieved = get_diagnostics();
        // 由于get_diagnostics返回DiagnosticsSnapshot而不是Arc，我们只检查数据
        assert!(retrieved.uptime.is_some());
    }

    #[test]
    fn test_diagnostics_snapshot_convenience_function() {
        // 检查是否已经有全局实例
        if let Some(diagnostics) = get_diagnostics_instance() {
            // 如果已经初始化，重置计数器
            diagnostics.reset();
        }
        
        let snapshot = get_diagnostics_snapshot();
        
        // 应该返回默认值（计数器为0）
        assert_eq!(snapshot.events_processed, 0);
        assert_eq!(snapshot.success_rate_percent, 100.0);
        // 注意：如果全局实例已经初始化，uptime可能不为None
        // 这是正常的，因为OnceLock一旦初始化就无法重置
    }

    #[test]
    fn test_edge_cases() {
        let diagnostics = Diagnostics::new();
        
        // 测试零除法情况
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.success_rate_percent, 100.0);
        
        // 测试只有丢弃事件的情况
        diagnostics.increment_events_dropped_error();
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.success_rate_percent, 0.0);
    }

    #[test]
    fn test_all_counter_types() {
        let diagnostics = Diagnostics::new();
        
        diagnostics.increment_events_processed();
        diagnostics.increment_events_dropped_backpressure();
        diagnostics.increment_events_dropped_error();
        diagnostics.increment_sink_errors();
        diagnostics.increment_pre_init_buffer_events();
        diagnostics.increment_pre_init_buffer_overflows();
        diagnostics.increment_database_batch_writes();
        diagnostics.increment_file_writes();
        diagnostics.increment_stdout_writes();
        
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.events_processed, 1);
        assert_eq!(snapshot.events_dropped_backpressure, 1);
        assert_eq!(snapshot.events_dropped_error, 1);
        assert_eq!(snapshot.sink_errors, 1);
        assert_eq!(snapshot.pre_init_buffer_events, 1);
        assert_eq!(snapshot.pre_init_buffer_overflows, 1);
        assert_eq!(snapshot.database_batch_writes, 1);
        assert_eq!(snapshot.file_writes, 1);
        assert_eq!(snapshot.stdout_writes, 1);
    }
}