//! 优雅停机模块
//!
//! 此模块提供了 QuantumLog 的优雅停机机制，确保所有日志都能被正确处理和刷新。

use crate::error::{QuantumLogError, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Notify, RwLock};
use tokio::time::timeout;
use tracing::{error, info, warn};

/// 停机句柄
///
/// 用于控制 QuantumLog 的优雅停机过程
#[derive(Debug, Clone)]
pub struct ShutdownHandle {
    /// 停机信号发送器
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
    /// 停机状态
    state: Arc<RwLock<ShutdownState>>,
    /// 停机完成通知
    completion_notify: Arc<Notify>,
    /// 停机超时时间
    timeout_duration: Duration,
}

/// 停机信号
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// 优雅停机
    Graceful,
    /// 强制停机
    Force,
    /// 立即停机
    Immediate,
}

/// 停机状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownState {
    /// 运行中
    Running,
    /// 停机中
    Shutting,
    /// 已停机
    Shutdown,
    /// 停机失败
    Failed(String),
}

/// 停机监听器
///
/// 用于接收停机信号的组件
pub struct ShutdownListener {
    /// 停机信号接收器
    shutdown_rx: broadcast::Receiver<ShutdownSignal>,
    /// 组件名称
    component_name: String,
}

/// 停机统计信息
#[derive(Debug, Clone, Default)]
pub struct ShutdownStats {
    /// 停机开始时间
    pub start_time: Option<std::time::Instant>,
    /// 停机完成时间
    pub end_time: Option<std::time::Instant>,
    /// 处理的日志数量
    pub processed_logs: u64,
    /// 刷新的批次数量
    pub flushed_batches: u32,
    /// 停机的组件数量
    pub shutdown_components: u32,
    /// 失败的组件数量
    pub failed_components: u32,
}

impl ShutdownHandle {
    /// 创建新的停机句柄
    pub fn new(timeout_duration: Duration) -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);

        Self {
            shutdown_tx,
            state: Arc::new(RwLock::new(ShutdownState::Running)),
            completion_notify: Arc::new(Notify::new()),
            timeout_duration,
        }
    }

    /// 创建停机监听器
    pub fn create_listener(&self, component_name: impl Into<String>) -> ShutdownListener {
        ShutdownListener {
            shutdown_rx: self.shutdown_tx.subscribe(),
            component_name: component_name.into(),
        }
    }

    /// 发起优雅停机
    pub async fn shutdown_graceful(&self) -> Result<ShutdownStats> {
        self.shutdown_with_signal(ShutdownSignal::Graceful).await
    }

    /// 发起强制停机
    pub async fn shutdown_force(&self) -> Result<ShutdownStats> {
        self.shutdown_with_signal(ShutdownSignal::Force).await
    }

    /// 发起立即停机
    pub async fn shutdown_immediate(&self) -> Result<ShutdownStats> {
        self.shutdown_with_signal(ShutdownSignal::Immediate).await
    }

    /// 使用指定信号停机
    async fn shutdown_with_signal(&self, signal: ShutdownSignal) -> Result<ShutdownStats> {
        // 检查当前状态
        {
            let mut state = self.state.write().await;
            match *state {
                ShutdownState::Running => {
                    *state = ShutdownState::Shutting;
                    info!("Starting {} shutdown", signal_name(&signal));
                }
                ShutdownState::Shutting => {
                    warn!("Shutdown already in progress");
                    return Err(QuantumLogError::ShutdownInProgress);
                }
                ShutdownState::Shutdown => {
                    warn!("Already shutdown");
                    return Err(QuantumLogError::AlreadyShutdown);
                }
                ShutdownState::Failed(ref reason) => {
                    warn!("Previous shutdown failed: {}", reason);
                    return Err(QuantumLogError::ShutdownFailed(reason.clone()));
                }
            }
        }

        let stats = Arc::new(RwLock::new(ShutdownStats {
            start_time: Some(std::time::Instant::now()),
            ..Default::default()
        }));

        // 发送停机信号
        if let Err(e) = self.shutdown_tx.send(signal.clone()) {
            error!("Failed to send shutdown signal: {}", e);
            let mut state = self.state.write().await;
            *state = ShutdownState::Failed(format!("Failed to send signal: {}", e));
            return Err(QuantumLogError::ShutdownFailed(format!(
                "Signal send failed: {}",
                e
            )));
        }

        // 等待停机完成或超时
        let result = match signal {
            ShutdownSignal::Immediate => {
                // 立即停机不等待
                Ok(())
            }
            _ => {
                // 等待停机完成
                timeout(self.timeout_duration, self.completion_notify.notified())
                    .await
                    .map_err(|_| QuantumLogError::ShutdownTimeout)
            }
        };

        // 更新状态和统计信息
        let final_stats = {
            let mut stats_guard = stats.write().await;
            stats_guard.end_time = Some(std::time::Instant::now());
            stats_guard.clone()
        };

        match result {
            Ok(_) => {
                let mut state = self.state.write().await;
                *state = ShutdownState::Shutdown;
                info!("Shutdown completed successfully");
                Ok(final_stats)
            }
            Err(e) => {
                let mut state = self.state.write().await;
                *state = ShutdownState::Failed(e.to_string());
                error!("Shutdown failed: {}", e);
                Err(e)
            }
        }
    }

    /// 通知停机完成
    pub fn notify_completion(&self) {
        self.completion_notify.notify_waiters();
    }

    /// 获取当前停机状态
    pub async fn get_state(&self) -> ShutdownState {
        self.state.read().await.clone()
    }

    /// 检查是否正在停机
    pub async fn is_shutting_down(&self) -> bool {
        matches!(
            *self.state.read().await,
            ShutdownState::Shutting | ShutdownState::Shutdown
        )
    }

    /// 检查是否已停机
    pub async fn is_shutdown(&self) -> bool {
        matches!(*self.state.read().await, ShutdownState::Shutdown)
    }

    /// 设置超时时间
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout_duration = timeout;
    }
}

impl ShutdownListener {
    /// 等待停机信号
    pub async fn wait_for_shutdown(&mut self) -> Result<ShutdownSignal> {
        match self.shutdown_rx.recv().await {
            Ok(signal) => {
                info!(
                    "Component '{}' received shutdown signal: {:?}",
                    self.component_name, signal
                );
                Ok(signal)
            }
            Err(broadcast::error::RecvError::Closed) => {
                warn!(
                    "Shutdown channel closed for component '{}'",
                    self.component_name
                );
                Err(QuantumLogError::ShutdownChannelClosed)
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!(
                    "Component '{}' lagged behind, skipped {} signals",
                    self.component_name, skipped
                );
                // 尝试再次接收
                Box::pin(self.wait_for_shutdown()).await
            }
        }
    }

    /// 非阻塞检查停机信号
    pub fn try_recv_shutdown(&mut self) -> Option<ShutdownSignal> {
        match self.shutdown_rx.try_recv() {
            Ok(signal) => {
                info!(
                    "Component '{}' received shutdown signal: {:?}",
                    self.component_name, signal
                );
                Some(signal)
            }
            Err(broadcast::error::TryRecvError::Empty) => None,
            Err(broadcast::error::TryRecvError::Closed) => {
                warn!(
                    "Shutdown channel closed for component '{}'",
                    self.component_name
                );
                Some(ShutdownSignal::Immediate)
            }
            Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                warn!(
                    "Component '{}' lagged behind, skipped {} signals",
                    self.component_name, skipped
                );
                // 返回强制停机信号
                Some(ShutdownSignal::Force)
            }
        }
    }

    /// 获取组件名称
    pub fn component_name(&self) -> &str {
        &self.component_name
    }
}

/// 获取信号名称
fn signal_name(signal: &ShutdownSignal) -> &'static str {
    match signal {
        ShutdownSignal::Graceful => "graceful",
        ShutdownSignal::Force => "force",
        ShutdownSignal::Immediate => "immediate",
    }
}

/// 停机超时配置
#[derive(Debug, Clone)]
pub struct ShutdownTimeouts {
    /// 优雅停机超时
    pub graceful: Duration,
    /// 强制停机超时
    pub force: Duration,
    /// 组件停机超时
    pub component: Duration,
}

impl Default for ShutdownTimeouts {
    fn default() -> Self {
        Self {
            graceful: Duration::from_secs(30),
            force: Duration::from_secs(10),
            component: Duration::from_secs(5),
        }
    }
}

/// 停机协调器
///
/// 协调多个组件的停机过程
pub struct ShutdownCoordinator {
    /// 停机句柄
    handle: ShutdownHandle,
    /// 注册的组件
    components: Arc<RwLock<Vec<String>>>,
}

impl ShutdownCoordinator {
    /// 创建新的停机协调器
    pub fn new(timeouts: ShutdownTimeouts) -> Self {
        Self {
            handle: ShutdownHandle::new(timeouts.graceful),
            components: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 注册组件
    pub async fn register_component(&self, name: impl Into<String>) -> ShutdownListener {
        let name = name.into();
        self.components.write().await.push(name.clone());
        self.handle.create_listener(name)
    }

    /// 获取停机句柄
    pub fn handle(&self) -> &ShutdownHandle {
        &self.handle
    }

    /// 获取注册的组件列表
    pub async fn get_components(&self) -> Vec<String> {
        self.components.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_shutdown_handle_creation() {
        let handle = ShutdownHandle::new(Duration::from_secs(5));
        assert!(matches!(handle.get_state().await, ShutdownState::Running));
    }

    #[tokio::test]
    async fn test_shutdown_listener() {
        let handle = ShutdownHandle::new(Duration::from_secs(5));
        let mut listener = handle.create_listener("test_component");

        // 在后台发送停机信号
        let handle_clone = handle.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(100)).await;
            let _ = handle_clone.shutdown_tx.send(ShutdownSignal::Graceful);
        });

        // 等待停机信号
        let signal = listener.wait_for_shutdown().await.unwrap();
        assert_eq!(signal, ShutdownSignal::Graceful);
    }

    #[tokio::test]
    async fn test_shutdown_coordinator() {
        let coordinator = ShutdownCoordinator::new(ShutdownTimeouts::default());

        let _listener1 = coordinator.register_component("component1").await;
        let _listener2 = coordinator.register_component("component2").await;

        let components = coordinator.get_components().await;
        assert_eq!(components.len(), 2);
        assert!(components.contains(&"component1".to_string()));
        assert!(components.contains(&"component2".to_string()));
    }

    #[tokio::test]
    async fn test_shutdown_states() {
        let handle = ShutdownHandle::new(Duration::from_secs(1));

        assert!(!handle.is_shutting_down().await);
        assert!(!handle.is_shutdown().await);

        // 模拟停机完成
        handle.notify_completion();

        // 注意：这个测试可能需要调整，因为状态变化是在 shutdown_graceful 中进行的
    }
}
