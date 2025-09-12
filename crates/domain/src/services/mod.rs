//! 领域服务定义
//!
//! 包含与业务逻辑相关的服务，如密码处理等。

pub mod auth_service;
pub mod password_service;
pub mod websocket_service;

#[cfg(test)]
mod auth_service_test;

// 重新导出服务
pub use auth_service::*;
pub use password_service::*;
pub use websocket_service::*;
