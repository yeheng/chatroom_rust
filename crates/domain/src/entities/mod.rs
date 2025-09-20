//! 领域实体定义
//!
//! 包含系统的核心实体：用户、聊天室、消息等。

pub mod auth;
pub mod bot;
pub mod chatroom;
pub mod department;
pub mod file_upload;
pub mod message;
pub mod notification;
pub mod online_statistics;
pub mod organization;
pub mod position;
pub mod proxy;
pub mod roles_permissions;
pub mod room_member;
pub mod user;
pub mod user_position;
pub mod websocket;

// 重新导出核心实体
pub use file_upload::FileUpload;
pub use notification::{notification_types, Notification, NotificationPriority};
