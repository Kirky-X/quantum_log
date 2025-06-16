//! 上下文注入层
//!
//! 此层负责向日志事件注入背景信息，如进程ID、线程ID、用户名、主机名和MPI Rank等。

use crate::core::event::ContextInfo;
use crate::mpi::ffi::{get_mpi_rank, is_mpi_available};
use crate::utils::background_info::{get_hostname, get_pid, get_tid, get_username};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// 上下文注入层
///
/// 此层会在每个日志事件中注入背景信息，包括：
/// - 进程ID (PID)
/// - 线程ID (TID)
/// - 用户名
/// - 主机名
/// - MPI Rank (如果可用)
/// - 自定义上下文字段
#[derive(Clone)]
pub struct ContextInjectorLayer {
    /// 缓存的上下文信息
    cached_context: Arc<RwLock<CachedContext>>,
    /// 自定义上下文字段
    custom_fields: HashMap<String, serde_json::Value>,
}

/// 缓存的上下文信息
#[derive(Debug, Clone, Default)]
struct CachedContext {
    /// 用户名 (进程级别，不会改变)
    username: Option<String>,
    /// 主机名 (进程级别，不会改变)
    hostname: Option<String>,
    /// MPI Rank (进程级别，不会改变)
    mpi_rank: Option<i32>,
    /// 是否已初始化
    initialized: bool,
}

impl ContextInjectorLayer {
    /// 创建新的上下文注入层
    pub fn new() -> Self {
        Self {
            cached_context: Arc::new(RwLock::new(CachedContext::default())),
            custom_fields: HashMap::new(),
        }
    }

    /// 添加自定义上下文字段
    pub fn with_custom_field(mut self, key: String, value: serde_json::Value) -> Self {
        self.custom_fields.insert(key, value);
        self
    }

    /// 获取当前上下文信息
    pub async fn get_context_info(&self) -> ContextInfo {
        // 获取缓存的上下文信息
        let cached = {
            let cache = self.cached_context.read().await;
            if cache.initialized {
                cache.clone()
            } else {
                drop(cache);
                // 初始化缓存
                let mut cache = self.cached_context.write().await;
                if !cache.initialized {
                    cache.username = Some(get_username());
                    cache.hostname = Some(get_hostname());
                    cache.mpi_rank = if is_mpi_available() {
                        get_mpi_rank()
                    } else {
                        None
                    };
                    cache.initialized = true;
                }
                cache.clone()
            }
        };

        // 获取动态信息 (每次调用都可能不同)
        let pid = get_pid();
        let tid = get_tid().parse::<u64>().unwrap_or(0);

        ContextInfo {
            pid,
            tid,
            username: cached.username,
            hostname: cached.hostname,
            mpi_rank: cached.mpi_rank,
            custom_fields: self.custom_fields.clone(),
        }
    }
}

impl Default for ContextInjectorLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for ContextInjectorLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        // 在实际实现中，我们需要将上下文信息附加到事件上
        // 这里我们使用 event.record() 来记录上下文信息
        // 但由于 tracing 的限制，我们需要在更高层次处理这个逻辑

        // 注意：实际的上下文注入会在 FormatLayer 或 DispatchLayer 中进行
        // 这里主要是为了展示架构设计

        let _ = event; // 避免未使用警告
    }
}

// 为了实际使用，我们提供一个辅助函数来获取上下文信息
impl ContextInjectorLayer {
    /// 为给定的事件创建上下文信息
    ///
    /// 这个方法会被其他层调用来获取上下文信息
    pub async fn create_context_for_event(&self) -> ContextInfo {
        self.get_context_info().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_injector_creation() {
        let layer = ContextInjectorLayer::new();
        assert!(layer.custom_fields.is_empty());
    }

    #[test]
    fn test_context_injector_with_custom_fields() {
        let layer = ContextInjectorLayer::new()
            .with_custom_field(
                "app_name".to_string(),
                serde_json::Value::String("test_app".to_string()),
            )
            .with_custom_field(
                "version".to_string(),
                serde_json::Value::String("1.0.0".to_string()),
            );

        assert_eq!(layer.custom_fields.len(), 2);
        assert!(layer.custom_fields.contains_key("app_name"));
        assert!(layer.custom_fields.contains_key("version"));
    }

    #[tokio::test]
    async fn test_context_info_generation() {
        let layer = ContextInjectorLayer::new();
        let context = layer.get_context_info().await;

        // PID 和 TID 应该总是有值
        assert!(context.pid > 0);
        assert!(context.tid > 0);

        // 用户名和主机名在大多数系统上应该可用
        // 但在某些环境下可能为 None，所以我们不强制要求
    }

    #[tokio::test]
    async fn test_context_caching() {
        let layer = ContextInjectorLayer::new();

        // 第一次调用
        let context1 = layer.get_context_info().await;

        // 第二次调用
        let context2 = layer.get_context_info().await;

        // 缓存的字段应该相同
        assert_eq!(context1.username, context2.username);
        assert_eq!(context1.hostname, context2.hostname);
        assert_eq!(context1.mpi_rank, context2.mpi_rank);

        // PID 应该相同 (同一进程)
        assert_eq!(context1.pid, context2.pid);

        // TID 可能相同也可能不同 (取决于调用的线程)
    }
}
