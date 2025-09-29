//! 应用层实现。
//!
//! 这里提供围绕领域模型的用例服务，处理输入校验、事务边界、
//! 以及对外部适配器（例如密码哈希、消息广播）的抽象。

pub mod broadcaster;
pub mod clock;
pub mod delivery;
pub mod error;
pub mod password;
pub mod presence;
pub mod rate_limiter;
pub mod repository;
pub mod sequencer;
pub mod services;

pub use broadcaster::{MessageBroadcast, MessageBroadcaster, MessageStream, WebSocketMessage};
pub use clock::{Clock, SystemClock};
pub use delivery::DeliveryTracker;
pub use error::ApplicationError;
pub use password::{PasswordHasher, PasswordHasherError};
pub use presence::{
    OnlineStats, PresenceEventType, PresenceManager, RedisPresenceManager, UserPresenceEvent,
};
pub use rate_limiter::{MessageRateLimiter, RateLimitError};
pub use repository::{ChatRoomRepository, MessageRepository, RoomMemberRepository, UserRepository};
pub use sequencer::{MessageSequencer, SequencedMessage};
pub use services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies};
