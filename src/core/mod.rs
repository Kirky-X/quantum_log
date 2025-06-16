//! QuantumLog 核心模块
//!
//! 本模块包含 QuantumLog 的核心功能组件，包括事件定义、订阅器和各种层。

pub mod event;
pub mod layers;
pub mod subscriber;

// 重新导出核心类型
pub use event::QuantumLogEvent;
pub use subscriber::{QuantumLogSubscriber, QuantumLogSubscriberBuilder};

// 重新导出层类型
pub use layers::{
    context_injector::ContextInjectorLayer, dispatcher::DispatcherLayer, formatter::FormatterLayer,
    pre_init_buffer::PreInitBufferLayer,
};
