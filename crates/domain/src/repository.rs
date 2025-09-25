use futures::future::BoxFuture;

use crate::errors::RepositoryError;
use crate::message::Message;
use crate::room_member::RoomMember;
use crate::user::User;
use crate::value_objects::{MessageId, RoomId, UserEmail, UserId};
use crate::ChatRoom;

pub type RepositoryResult<T> = Result<T, RepositoryError>;
pub type RepositoryFuture<T> = BoxFuture<'static, RepositoryResult<T>>;

pub trait UserRepository: Send + Sync {
    fn create(&self, user: User) -> RepositoryFuture<User>;
    fn update(&self, user: User) -> RepositoryFuture<User>;
    fn find_by_id(&self, id: UserId) -> RepositoryFuture<Option<User>>;
    fn find_by_email(&self, email: UserEmail) -> RepositoryFuture<Option<User>>;
}

pub trait ChatRoomRepository: Send + Sync {
    fn create(&self, room: ChatRoom) -> RepositoryFuture<ChatRoom>;
    fn update(&self, room: ChatRoom) -> RepositoryFuture<ChatRoom>;
    fn delete(&self, id: RoomId) -> RepositoryFuture<()>;
    fn find_by_id(&self, id: RoomId) -> RepositoryFuture<Option<ChatRoom>>;
    fn list_by_owner(&self, owner: UserId) -> RepositoryFuture<Vec<ChatRoom>>;
}

pub trait RoomMemberRepository: Send + Sync {
    fn upsert(&self, member: RoomMember) -> RepositoryFuture<RoomMember>;
    fn remove(&self, room_id: RoomId, user_id: UserId) -> RepositoryFuture<()>;
    fn find(&self, room_id: RoomId, user_id: UserId) -> RepositoryFuture<Option<RoomMember>>;
    fn list_members(&self, room_id: RoomId) -> RepositoryFuture<Vec<RoomMember>>;
}

pub trait MessageRepository: Send + Sync {
    fn create(&self, message: Message) -> RepositoryFuture<Message>;
    fn find_by_id(&self, id: MessageId) -> RepositoryFuture<Option<Message>>;
    fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> RepositoryFuture<Vec<Message>>;
}
