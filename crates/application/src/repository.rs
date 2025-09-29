use async_trait::async_trait;
use domain::{
    ChatRoom, Message, MessageId, MessageDelivery, RepositoryError, RoomId, RoomMember, User, UserEmail, UserId,
};

/// 简单直接的事务管理：使用一个单独的service层来管理事务
pub struct TransactionScope;

impl TransactionScope {
    pub fn new() -> Self {
        Self
    }
}

/// 通用分页参数
#[derive(Debug, Clone, Copy)]
pub struct PaginationParams {
    pub limit: i64,
    pub offset: Option<i64>,
}

impl PaginationParams {
    pub fn new(limit: i64) -> Self {
        Self {
            limit,
            offset: None,
        }
    }

    pub fn with_offset(limit: i64, offset: i64) -> Self {
        Self {
            limit,
            offset: Some(offset),
        }
    }
}

/// 时间范围查询参数
#[derive(Debug, Clone, Default)]
pub struct TimeRangeParams {
    pub start: Option<chrono::DateTime<chrono::Utc>>,
    pub end: Option<chrono::DateTime<chrono::Utc>>,
    pub include_deleted: bool,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    /// 创建新用户（使用连接池）
    async fn create(&self, user: User) -> Result<User, RepositoryError>;

    /// 更新用户信息
    async fn update(&self, user: User) -> Result<User, RepositoryError>;

    /// 根据ID查找用户
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError>;

    /// 根据邮箱查找用户
    async fn find_by_email(&self, email: UserEmail) -> Result<Option<User>, RepositoryError>;

    /// 删除用户（软删除或硬删除）
    async fn delete(&self, _id: UserId) -> Result<(), RepositoryError> {
        // 默认实现：不支持删除用户（出于数据完整性考虑）
        Err(RepositoryError::storage(
            "User deletion not supported".to_string(),
        ))
    }
}

#[async_trait]
pub trait ChatRoomRepository: Send + Sync {
    /// 创建新房间（使用连接池）
    async fn create(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError>;

    /// 更新房间信息
    async fn update(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError>;

    /// 根据ID查找房间
    async fn find_by_id(&self, id: RoomId) -> Result<Option<ChatRoom>, RepositoryError>;

    /// 删除房间
    async fn delete(&self, id: RoomId) -> Result<(), RepositoryError>;

    /// 根据所有者查询房间列表
    async fn find_by_owner(&self, owner_id: UserId) -> Result<Vec<ChatRoom>, RepositoryError>;

    /// 查询公开房间列表（支持分页）
    async fn find_public_rooms(
        &self,
        pagination: PaginationParams,
    ) -> Result<Vec<ChatRoom>, RepositoryError> {
        // 默认实现：返回空列表（需要具体实现来支持）
        let _ = pagination;
        Ok(Vec::new())
    }

    /// 原子性创建房间和owner成员 - Linus式简化版本
    /// 这个方法解决了create_room的核心事务问题，无需额外抽象
    async fn create_with_owner(
        &self,
        room: ChatRoom,
        owner: RoomMember,
    ) -> Result<ChatRoom, RepositoryError>;
}

#[async_trait]
pub trait RoomMemberRepository: Send + Sync {
    /// 创建或更新房间成员（使用连接池）
    async fn upsert(&self, member: RoomMember) -> Result<RoomMember, RepositoryError>;

    /// 查找特定用户在房间中的成员信息
    async fn find_member(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, RepositoryError>;

    /// 移除房间成员
    async fn delete_member(&self, room_id: RoomId, user_id: UserId) -> Result<(), RepositoryError>;

    /// 查询房间所有成员
    async fn find_by_room(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError>;

    /// 查询用户参与的所有房间
    async fn find_by_user(&self, user_id: UserId) -> Result<Vec<RoomMember>, RepositoryError> {
        // 默认实现：返回空列表（需要具体实现来支持）
        let _ = user_id;
        Ok(Vec::new())
    }

    // === 向后兼容的方法别名 ===

    /// @deprecated 使用 find_member 替代
    async fn find(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, RepositoryError> {
        self.find_member(room_id, user_id).await
    }

    /// @deprecated 使用 delete_member 替代
    async fn remove(&self, room_id: RoomId, user_id: UserId) -> Result<(), RepositoryError> {
        self.delete_member(room_id, user_id).await
    }

    /// @deprecated 使用 find_by_room 替代
    async fn list_members(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError> {
        self.find_by_room(room_id).await
    }
}

#[async_trait]
pub trait MessageRepository: Send + Sync {
    /// 保存消息到数据库，返回消息ID（使用连接池）
    async fn create(&self, message: Message) -> Result<MessageId, RepositoryError>;

    /// 根据ID查找消息
    async fn find_by_id(&self, id: MessageId) -> Result<Option<Message>, RepositoryError>;

    /// 获取房间的最近消息（支持分页）
    async fn find_recent_by_room(
        &self,
        room_id: RoomId,
        pagination: PaginationParams,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError>;

    /// 获取指定时间之后的消息（用于用户重连）
    async fn find_since_timestamp(
        &self,
        room_id: RoomId,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Message>, RepositoryError>;

    /// 根据时间范围查询消息（管理员功能）
    async fn find_by_time_range(
        &self,
        room_id: RoomId,
        time_range: TimeRangeParams,
        pagination: PaginationParams,
    ) -> Result<Vec<Message>, RepositoryError>;

    /// 更新消息内容（编辑功能）
    async fn update(&self, message: Message) -> Result<(), RepositoryError>;

    /// 软删除消息
    async fn delete(&self, id: MessageId) -> Result<(), RepositoryError> {
        // 默认实现：不支持删除消息
        let _ = id;
        Err(RepositoryError::storage(
            "Message deletion not supported".to_string(),
        ))
    }

    // === 向后兼容的方法别名 ===

    /// @deprecated 使用 create 替代，保持一致的命名
    async fn save_message(&self, message: Message) -> Result<MessageId, RepositoryError> {
        self.create(message).await
    }

    /// @deprecated 使用 find_recent_by_room 替代
    async fn get_recent_messages(
        &self,
        room_id: RoomId,
        limit: i64,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError> {
        self.find_recent_by_room(room_id, PaginationParams::new(limit), before)
            .await
    }

    /// @deprecated 使用 find_since_timestamp 替代
    async fn get_messages_since(
        &self,
        room_id: RoomId,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Message>, RepositoryError> {
        self.find_since_timestamp(room_id, timestamp).await
    }

    /// @deprecated 使用 find_by_time_range 替代
    async fn get_admin_message_history(
        &self,
        room_id: RoomId,
        before: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<i64>,
        include_deleted: bool,
    ) -> Result<Vec<Message>, RepositoryError> {
        let time_range = TimeRangeParams {
            start: None,
            end: before,
            include_deleted,
        };
        let pagination = PaginationParams::new(limit.unwrap_or(50));
        self.find_by_time_range(room_id, time_range, pagination)
            .await
    }

    /// @deprecated 使用 find_recent_by_room 替代，注意参数类型变化
    async fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError> {
        self.find_recent_by_room(room_id, PaginationParams::new(limit as i64), before)
            .await
    }
}

/// 消息传递状态追踪Repository
/// 对应数据库表：message_deliveries
#[async_trait]
pub trait MessageDeliveryRepository: Send + Sync {
    /// 记录消息发送状态
    async fn record_sent(&self, delivery: MessageDelivery) -> Result<(), RepositoryError>;

    /// 标记消息已送达
    async fn mark_delivered(
        &self,
        message_id: MessageId,
        user_id: UserId,
        delivered_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError>;

    /// 查找用户的未送达消息
    async fn find_undelivered_for_user(&self, user_id: UserId) -> Result<Vec<MessageDelivery>, RepositoryError>;

    /// 查找特定消息的传递状态
    async fn find_by_message(&self, message_id: MessageId) -> Result<Vec<MessageDelivery>, RepositoryError>;

    /// 清理已送达的旧记录（用于数据维护）
    async fn cleanup_delivered_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, RepositoryError>;
}
