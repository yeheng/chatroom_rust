//! 基础设施层实现。
//!
//! 提供数据库仓储、密码哈希、消息广播等适配器，实现应用/领域层定义的接口。

pub mod broadcast;
pub mod builder;
pub mod migrations;
pub mod password;

pub use broadcast::{
    BroadcasterType, LocalMessageBroadcaster, MessageStream, RedisMessageBroadcaster,
    RedisMessageStream, WebSocketError,
};
pub use builder::{Infrastructure, InfrastructureConfig, InfrastructureError};
pub use migrations::MIGRATOR;
pub use password::BcryptPasswordHasher;
