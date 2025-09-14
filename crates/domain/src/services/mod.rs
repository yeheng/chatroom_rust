//! 领域服务定义
//!
//! 包含与业务逻辑相关的服务，如密码处理等。

pub mod auth_service;
pub mod chatroom_service;
pub mod online_time_service;
pub mod organization_service;
pub mod password_service;
pub mod proxy_service;
pub mod user_management_service;
pub mod websocket_service;

#[cfg(test)]
mod auth_service_test;

// 重新导出服务
pub use auth_service::*;
pub use chatroom_service::*;
pub use online_time_service::*;
pub use organization_service::*;
pub use password_service::*;
pub use proxy_service::*;
pub use user_management_service::*;
pub use websocket_service::*;
