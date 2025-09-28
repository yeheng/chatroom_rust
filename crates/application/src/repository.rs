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
    // 保存消息到数据库，确保每条消息立即持久化
    async fn save_message(&self, message: Message) -> Result<MessageId, RepositoryError>;

    // 根据ID查找消息
    async fn find_by_id(&self, id: MessageId) -> Result<Option<Message>, RepositoryError>;

    // 获取房间最近的消息（支持分页）
    async fn get_recent_messages(
        &self,
        room_id: RoomId,
        limit: i64,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError>;

    // 获取指定时间之后的所有消息（用于用户重连后获取错过的消息）
    async fn get_messages_since(
        &self,
        room_id: RoomId,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Message>, RepositoryError>;

    // 管理员专用：获取历史消息（包含已删除消息）
    async fn get_admin_message_history(
        &self,
        room_id: RoomId,
        before: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<i64>,
        include_deleted: bool,
    ) -> Result<Vec<Message>, RepositoryError>;

    // 为了向后兼容，保留原有的 create 方法
    async fn create(&self, message: Message) -> Result<Message, RepositoryError> {
        let message_id = self.save_message(message.clone()).await?;
        self.find_by_id(message_id).await?.ok_or(RepositoryError::NotFound)
    }

    // 为了向后兼容，保留原有的 list_recent 方法
    async fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError> {
        self.get_recent_messages(room_id, limit as i64, before).await
    }
}
