//! 应用层服务
//!
//! 包含聊天室、消息和用户管理服务。

pub mod chat_room_service;
pub mod feature_flag_service;
pub mod message_history_service;
pub mod message_service;
pub mod organization_service;
pub mod user_service;

pub use chat_room_service::*;
pub use feature_flag_service::*;
pub use message_history_service::*;
pub use message_service::*;
pub use organization_service::*;
pub use user_service::*;

#[cfg(test)]
pub mod chat_room_service_tests;
#[cfg(test)]
pub mod message_history_service_tests;
#[cfg(test)]
pub mod message_service_tests;
#[cfg(test)]
pub mod user_service_tests;
