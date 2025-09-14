//! 领域事件定义
//!
//! 包含聊天系统的各种领域事件，用于实现事件驱动架构

pub mod chat_event;

// 重新导出事件类型
pub use chat_event::*;
