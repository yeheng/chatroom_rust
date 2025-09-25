//! 应用层实现。
//!
//! 这里提供围绕领域模型的用例服务，处理输入校验、事务边界、
//! 以及对外部适配器（例如密码哈希、消息广播）的抽象。

pub mod broadcaster;
pub mod clock;
pub mod error;
pub mod password;
pub mod services;

pub use broadcaster::{MessageBroadcast, MessageBroadcaster};
pub use clock::{Clock, SystemClock};
pub use error::ApplicationError;
pub use password::{PasswordHasher, PasswordHasherError};
pub use services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies};
