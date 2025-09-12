//! 聊天室系统核心领域模型
//!
//! 包含用户、聊天室、消息等核心实体，以及相关的业务逻辑和规则。

pub mod entities;
pub mod errors;
pub mod feature_flags;
pub mod services;

// 重新导出常用类型
pub use entities::*;
pub use errors::*;
pub use feature_flags::*;
pub use services::*;
