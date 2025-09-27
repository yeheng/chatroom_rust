use async_trait::async_trait;
use domain::{
    ChatRoom, Message, MessageId, RepositoryError, RoomId, RoomMember, User, UserEmail, UserId,
};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn create(&self, user: User) -> Result<User, RepositoryError>;
    async fn update(&self, user: User) -> Result<User, RepositoryError>;
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError>;
    async fn find_by_email(&self, email: UserEmail) -> Result<Option<User>, RepositoryError>;
}

#[async_trait]
pub trait ChatRoomRepository: Send + Sync {
    async fn create(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError>;
    async fn update(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError>;
    async fn find_by_id(&self, id: RoomId) -> Result<Option<ChatRoom>, RepositoryError>;
    async fn delete(&self, id: RoomId) -> Result<(), RepositoryError>;
    async fn list_by_owner(&self, owner_id: UserId) -> Result<Vec<ChatRoom>, RepositoryError>;
}

#[async_trait]
pub trait RoomMemberRepository: Send + Sync {
    async fn upsert(&self, member: RoomMember) -> Result<RoomMember, RepositoryError>;
    async fn find(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, RepositoryError>;
    async fn remove(&self, room_id: RoomId, user_id: UserId) -> Result<(), RepositoryError>;
    async fn list_members(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError>;
}

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn create(&self, message: Message) -> Result<Message, RepositoryError>;
    async fn find_by_id(&self, id: MessageId) -> Result<Option<Message>, RepositoryError>;
    async fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError>;
}
