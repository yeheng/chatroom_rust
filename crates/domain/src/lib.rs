//! 聊天室系统核心领域模型。
//!
//! 这里只保留纯业务概念：实体、值对象、领域错误、仓储契约。
//! 所有外部技术细节（数据库、加密、异步运行时等）都被隔离在其他层。

mod chat_room;
mod errors;
mod message;
mod repository;
mod room_member;
mod user;
mod value_objects;

pub use chat_room::{ChatRoom, ChatRoomVisibility};
pub use errors::{DomainError, RepositoryError};
pub use message::{Message, MessageRevision, MessageType};
pub use repository::{
    ChatRoomRepository, MessageRepository, RepositoryFuture, RepositoryResult,
    RoomMemberRepository, UserRepository,
};
pub use room_member::{RoomMember, RoomRole};
pub use user::{User, UserStatus};
pub use value_objects::{
    MessageContent, MessageId, PasswordHash, RoomId, Timestamp, UserEmail, UserId, Username,
};
