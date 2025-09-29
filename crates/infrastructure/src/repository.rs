use std::sync::Arc;

use application::repository::{
    ChatRoomRepository, MessageRepository, MessageDeliveryRepository, PaginationParams, RoomMemberRepository, TimeRangeParams,
    UserRepository,
};
use async_trait::async_trait;
use domain::{
    ChatRoom, ChatRoomVisibility, Message, MessageContent, MessageId, MessageType, MessageDelivery, RepositoryError,
    RoomId, RoomMember, RoomRole, User, UserEmail, UserId, UserStatus,
};
use sqlx::{postgres::PgPoolOptions, types::chrono, FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn map_sqlx_err(err: sqlx::Error) -> RepositoryError {
    match err {
        sqlx::Error::RowNotFound => RepositoryError::NotFound,
        sqlx::Error::Database(ref db_err) if db_err.code().is_some_and(|code| code == "23505") => {
            RepositoryError::Conflict
        }
        other => {
            let message = other.to_string();
            RepositoryError::storage_with_source(message, other)
        }
    }
}

fn invalid_data<E>(error: E) -> RepositoryError
where
    E: std::error::Error + Send + Sync + 'static,
{
    RepositoryError::storage_with_source(error.to_string(), error)
}

#[derive(Clone)]
pub struct PgStorage {
    pub user_repository: Arc<PgUserRepository>,
    pub room_repository: Arc<PgChatRoomRepository>,
    pub member_repository: Arc<PgRoomMemberRepository>,
    pub message_repository: Arc<PgMessageRepository>,
}

impl PgStorage {
    pub fn new(pool: PgPool) -> Self {
        let user_repository = Arc::new(PgUserRepository::new(pool.clone()));
        let room_repository = Arc::new(PgChatRoomRepository::new(pool.clone()));
        let member_repository = Arc::new(PgRoomMemberRepository::new(pool.clone()));
        let message_repository = Arc::new(PgMessageRepository::new(pool.clone()));

        Self {
            user_repository,
            room_repository,
            member_repository,
            message_repository,
        }
    }
}

#[derive(Debug, FromRow)]
struct UserRecord {
    id: Uuid,
    username: String,
    email: String,
    password_hash: String,
    status: UserStatus,
    is_superuser: bool,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<UserRecord> for User {
    type Error = RepositoryError;

    fn try_from(value: UserRecord) -> Result<Self, Self::Error> {
        let username = domain::Username::parse(value.username).map_err(invalid_data)?;
        let email = domain::UserEmail::parse(value.email).map_err(invalid_data)?;
        let password = domain::PasswordHash::new(value.password_hash).map_err(invalid_data)?;

        Ok(User {
            id: UserId::from(value.id),
            username,
            email,
            password,
            status: value.status,
            is_superuser: value.is_superuser,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

#[derive(Clone)]
pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn create(&self, user: User) -> Result<User, RepositoryError> {
        let record = sqlx::query_as::<_, UserRecord>(
            r#"
            INSERT INTO users (id, username, email, password_hash, status, is_superuser, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, username, email, password_hash, status, is_superuser, created_at, updated_at
            "#,
        )
        .bind(Uuid::from(user.id))
        .bind(user.username.as_str())
        .bind(user.email.as_str())
        .bind(user.password.as_str())
        .bind(&user.status)
        .bind(user.is_superuser)
        .bind(user.created_at)
        .bind(user.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        User::try_from(record)
    }

    async fn update(&self, user: User) -> Result<User, RepositoryError> {
        let record = sqlx::query_as::<_, UserRecord>(
            r#"
            UPDATE users
            SET username = $2, email = $3, password_hash = $4, status = $5, is_superuser = $6, updated_at = $7
            WHERE id = $1
            RETURNING id, username, email, password_hash, status, is_superuser, created_at, updated_at
            "#,
        )
        .bind(Uuid::from(user.id))
        .bind(user.username.as_str())
        .bind(user.email.as_str())
        .bind(user.password.as_str())
        .bind(&user.status)
        .bind(user.is_superuser)
        .bind(user.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        User::try_from(record)
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let record = sqlx::query_as::<_, UserRecord>(
            r#"SELECT id, username, email, password_hash, status, is_superuser, created_at, updated_at FROM users WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(User::try_from).transpose()
    }

    async fn find_by_email(&self, email: UserEmail) -> Result<Option<User>, RepositoryError> {
        let record = sqlx::query_as::<_, UserRecord>(
            r#"SELECT id, username, email, password_hash, status, is_superuser, created_at, updated_at FROM users WHERE email = $1"#,
        )
        .bind(email.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(User::try_from).transpose()
    }
}

#[derive(Debug, FromRow)]
struct RoomRecord {
    id: Uuid,
    name: String,
    owner_id: Uuid,
    is_private: bool,
    password_hash: Option<String>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    is_closed: bool,
}

impl TryFrom<RoomRecord> for ChatRoom {
    type Error = RepositoryError;

    fn try_from(value: RoomRecord) -> Result<Self, Self::Error> {
        let password = match value.password_hash {
            Some(hash) => Some(domain::PasswordHash::new(hash).map_err(invalid_data)?),
            None => None,
        };

        Ok(ChatRoom {
            id: RoomId::from(value.id),
            name: value.name,
            owner_id: UserId::from(value.owner_id),
            visibility: if value.is_private {
                ChatRoomVisibility::Private
            } else {
                ChatRoomVisibility::Public
            },
            password,
            created_at: value.created_at,
            updated_at: value.updated_at,
            is_closed: value.is_closed,
        })
    }
}

#[derive(Clone)]
pub struct PgChatRoomRepository {
    pool: PgPool,
}

impl PgChatRoomRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 内部函数：在事务中插入房间 - 单一职责
    async fn insert_room_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        room: &ChatRoom,
    ) -> Result<ChatRoom, RepositoryError> {
        let record = sqlx::query_as::<_, RoomRecord>(
            "INSERT INTO chat_rooms (id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING *"
        )
        .bind(Uuid::from(room.id))
        .bind(&room.name)
        .bind(Uuid::from(room.owner_id))
        .bind(matches!(room.visibility, ChatRoomVisibility::Private))
        .bind(room.password.as_ref().map(|h| h.as_str()))
        .bind(room.created_at)
        .bind(room.updated_at)
        .bind(room.is_closed)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_sqlx_err)?;

        ChatRoom::try_from(record)
    }

    /// 内部函数：在事务中插入成员 - 单一职责
    async fn insert_member_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        member: &RoomMember,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO room_members (room_id, user_id, role, joined_at, last_read_message_id)
             VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(Uuid::from(member.room_id))
        .bind(Uuid::from(member.user_id))
        .bind(&member.role)
        .bind(member.joined_at)
        .bind(member.last_read_message.map(Uuid::from))
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx_err)?;

        Ok(())
    }
}

#[async_trait]
impl ChatRoomRepository for PgChatRoomRepository {
    async fn create(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError> {
        let record = sqlx::query_as::<_, RoomRecord>(
            r#"
            INSERT INTO chat_rooms (id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed
            "#,
        )
        .bind(Uuid::from(room.id))
        .bind(room.name)
        .bind(Uuid::from(room.owner_id))
        .bind(matches!(room.visibility, ChatRoomVisibility::Private))
        .bind(room.password.as_ref().map(|hash| hash.as_str()))
        .bind(room.created_at)
        .bind(room.updated_at)
        .bind(room.is_closed)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        ChatRoom::try_from(record)
    }

    async fn update(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError> {
        let record = sqlx::query_as::<_, RoomRecord>(
            r#"
            UPDATE chat_rooms
            SET name = $2, owner_id = $3, is_private = $4, password_hash = $5, updated_at = $6, is_closed = $7
            WHERE id = $1
            RETURNING id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed
            "#,
        )
        .bind(Uuid::from(room.id))
        .bind(room.name)
        .bind(Uuid::from(room.owner_id))
        .bind(matches!(room.visibility, ChatRoomVisibility::Private))
        .bind(room.password.as_ref().map(|hash| hash.as_str()))
        .bind(room.updated_at)
        .bind(room.is_closed)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        ChatRoom::try_from(record)
    }

    async fn find_by_id(&self, id: RoomId) -> Result<Option<ChatRoom>, RepositoryError> {
        let record = sqlx::query_as::<_, RoomRecord>(
            r#"SELECT id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed FROM chat_rooms WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(ChatRoom::try_from).transpose()
    }

    async fn delete(&self, id: RoomId) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM chat_rooms WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        Ok(())
    }

    async fn find_by_owner(&self, owner_id: UserId) -> Result<Vec<ChatRoom>, RepositoryError> {
        let records = sqlx::query_as::<_, RoomRecord>(
            r#"SELECT id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed FROM chat_rooms WHERE owner_id = $1"#,
        )
        .bind(Uuid::from(owner_id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        records.into_iter().map(ChatRoom::try_from).collect()
    }

    /// Linus式原子操作：消除特殊情况，直接解决问题
    /// 不需要什么该死的TransactionManager抽象
    async fn create_with_owner(
        &self,
        room: ChatRoom,
        owner: RoomMember,
    ) -> Result<ChatRoom, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_err)?;

        // 1. 创建房间 - 单一职责，短小精悍
        let created_room = self.insert_room_tx(&mut tx, &room).await?;

        // 2. 创建owner成员 - 同样简单直接
        self.insert_member_tx(&mut tx, &owner).await?;

        // 3. 提交事务 - 原子性保证
        tx.commit().await.map_err(map_sqlx_err)?;

        Ok(created_room)
    }

    // === 向后兼容方法 ===
    // 注意：向后兼容方法已移动到 trait 的默认实现中
}

#[derive(Debug, FromRow)]
struct MemberRecord {
    room_id: Uuid,
    user_id: Uuid,
    role: RoomRole,
    joined_at: OffsetDateTime,
    last_read_message_id: Option<Uuid>,
}

impl From<MemberRecord> for RoomMember {
    fn from(value: MemberRecord) -> Self {
        Self {
            room_id: RoomId::from(value.room_id),
            user_id: UserId::from(value.user_id),
            role: value.role,
            joined_at: value.joined_at,
            last_read_message: value.last_read_message_id.map(MessageId::from),
        }
    }
}

#[derive(Clone)]
pub struct PgRoomMemberRepository {
    pool: PgPool,
}

impl PgRoomMemberRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RoomMemberRepository for PgRoomMemberRepository {
    async fn upsert(&self, member: RoomMember) -> Result<RoomMember, RepositoryError> {
        let record = sqlx::query_as::<_, MemberRecord>(
            r#"
            INSERT INTO room_members (room_id, user_id, role, joined_at, last_read_message_id)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (room_id, user_id)
            DO UPDATE SET role = $3, joined_at = $4, last_read_message_id = $5
            RETURNING room_id, user_id, role, joined_at, last_read_message_id
            "#,
        )
        .bind(Uuid::from(member.room_id))
        .bind(Uuid::from(member.user_id))
        .bind(&member.role)
        .bind(member.joined_at)
        .bind(member.last_read_message.map(Uuid::from))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(RoomMember::from(record))
    }

    async fn find_member(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<Option<RoomMember>, RepositoryError> {
        let record = sqlx::query_as::<_, MemberRecord>(
            r#"SELECT room_id, user_id, role, joined_at, last_read_message_id FROM room_members WHERE room_id = $1 AND user_id = $2"#,
        )
        .bind(Uuid::from(room_id))
        .bind(Uuid::from(user_id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(record.map(RoomMember::from))
    }

    async fn delete_member(&self, room_id: RoomId, user_id: UserId) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM room_members WHERE room_id = $1 AND user_id = $2")
            .bind(Uuid::from(room_id))
            .bind(Uuid::from(user_id))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        Ok(())
    }

    async fn find_by_room(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError> {
        let records = sqlx::query_as::<_, MemberRecord>(
            r#"SELECT room_id, user_id, role, joined_at, last_read_message_id FROM room_members WHERE room_id = $1"#,
        )
        .bind(Uuid::from(room_id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(records.into_iter().map(RoomMember::from).collect())
    }

    // === 向后兼容方法 ===
    // 注意：向后兼容方法已移动到 trait 的默认实现中
}

#[derive(Debug, FromRow)]
struct MessageRecord {
    id: Uuid,
    room_id: Uuid,
    user_id: Uuid,
    content: String,
    message_type: MessageType,
    reply_to_message_id: Option<Uuid>,
    created_at: OffsetDateTime,
    updated_at: Option<OffsetDateTime>, // 对应SQL schema中的updated_at字段
    is_deleted: bool,
}

impl TryFrom<MessageRecord> for Message {
    type Error = RepositoryError;

    fn try_from(value: MessageRecord) -> Result<Self, Self::Error> {
        let content = MessageContent::new(value.content).map_err(invalid_data)?;

        // 如果消息被编辑过（updated_at存在且不同于created_at），创建revision记录
        let last_revision = if let Some(updated_at) = value.updated_at {
            if updated_at != value.created_at {
                Some(domain::MessageRevision {
                    content: content.clone(), // 当前存储的是最新内容，原始内容已丢失
                    updated_at,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok(Message {
            id: MessageId::from(value.id),
            room_id: RoomId::from(value.room_id),
            sender_id: UserId::from(value.user_id),
            content,
            message_type: value.message_type,
            reply_to: value.reply_to_message_id.map(MessageId::from),
            created_at: value.created_at,
            last_revision,
            is_deleted: value.is_deleted,
        })
    }
}

#[derive(Clone)]
pub struct PgMessageRepository {
    pool: PgPool,
}

impl PgMessageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MessageRepository for PgMessageRepository {
    // 新的核心方法：保存消息并返回消息ID
    async fn create(&self, message: Message) -> Result<MessageId, RepositoryError> {
        let record = sqlx::query_as::<_, MessageRecord>(
            r#"
            INSERT INTO messages (id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
            "#,
        )
        .bind(Uuid::from(message.id))
        .bind(Uuid::from(message.room_id))
        .bind(Uuid::from(message.sender_id))
        .bind(message.content.as_str())
        .bind(&message.message_type)
        .bind(message.reply_to.map(Uuid::from))
        .bind(message.created_at)
        .bind(message.created_at) // 新消息的updated_at初始值等于created_at
        .bind(message.is_deleted)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(MessageId::from(record.id))
    }

    async fn update(&self, message: Message) -> Result<(), RepositoryError> {
        // 计算updated_at时间：如果有编辑历史则使用编辑时间，否则使用当前时间
        let updated_at = if let Some(revision) = &message.last_revision {
            revision.updated_at
        } else {
            message.created_at // 如果没有编辑历史，保持原创建时间
        };

        sqlx::query(
            r#"
            UPDATE messages
            SET content = $2, updated_at = $3
            WHERE id = $1 AND is_deleted = FALSE
            "#,
        )
        .bind(Uuid::from(message.id))
        .bind(message.content.as_str())
        .bind(updated_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(())
    }

    async fn find_by_id(&self, id: MessageId) -> Result<Option<Message>, RepositoryError> {
        let record = sqlx::query_as::<_, MessageRecord>(
            r#"SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted FROM messages WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(Message::try_from).transpose()
    }

    // 增强的获取最近消息方法
    async fn find_recent_by_room(
        &self,
        room_id: RoomId,
        pagination: PaginationParams,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError> {
        let records = if let Some(before_id) = before {
            sqlx::query_as::<_, MessageRecord>(
                r#"
                SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
                FROM messages
                WHERE room_id = $1 AND id < $2 AND is_deleted = FALSE
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(Uuid::from(room_id))
            .bind(Uuid::from(before_id))
            .bind(pagination.limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, MessageRecord>(
                r#"
                SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
                FROM messages
                WHERE room_id = $1 AND is_deleted = FALSE
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(Uuid::from(room_id))
            .bind(pagination.limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(map_sqlx_err)?;

        records.into_iter().map(Message::try_from).collect()
    }

    // 新方法：获取指定时间之后的消息
    async fn find_since_timestamp(
        &self,
        room_id: RoomId,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Message>, RepositoryError> {
        // 精确转换时间类型（保留纳秒精度）
        let timestamp_nanos = timestamp
            .timestamp_nanos_opt()
            .ok_or_else(|| RepositoryError::storage("Timestamp out of range".to_string()))?;

        let time_offset = time::OffsetDateTime::from_unix_timestamp_nanos(timestamp_nanos as i128)
            .map_err(|e| {
                RepositoryError::storage_with_source("Invalid timestamp".to_string(), e)
            })?;

        let records = sqlx::query_as::<_, MessageRecord>(
            r#"
            SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
            FROM messages
            WHERE room_id = $1 AND created_at > $2 AND is_deleted = FALSE
            ORDER BY created_at ASC
            "#,
        )
        .bind(Uuid::from(room_id))
        .bind(time_offset)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        records.into_iter().map(Message::try_from).collect()
    }

    // 管理员专用：按时间范围获取历史消息（包含已删除消息）
    async fn find_by_time_range(
        &self,
        room_id: RoomId,
        time_range: TimeRangeParams,
        pagination: PaginationParams,
    ) -> Result<Vec<Message>, RepositoryError> {
        let query = if time_range.include_deleted {
            r#"
            SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
            FROM messages
            WHERE room_id = $1
                AND ($2::timestamptz IS NULL OR created_at >= $2)
                AND ($3::timestamptz IS NULL OR created_at <= $3)
            ORDER BY created_at DESC
            LIMIT $4
            "#
        } else {
            r#"
            SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
            FROM messages
            WHERE room_id = $1
                AND ($2::timestamptz IS NULL OR created_at >= $2)
                AND ($3::timestamptz IS NULL OR created_at <= $3)
                AND is_deleted = FALSE
            ORDER BY created_at DESC
            LIMIT $4
            "#
        };

        // 转换时间参数
        let start_time = if let Some(timestamp) = time_range.start {
            let timestamp_nanos = timestamp.timestamp_nanos_opt().ok_or_else(|| {
                RepositoryError::storage("Start timestamp out of range".to_string())
            })?;
            Some(
                time::OffsetDateTime::from_unix_timestamp_nanos(timestamp_nanos as i128).map_err(
                    |e| {
                        RepositoryError::storage_with_source(
                            "Invalid start timestamp".to_string(),
                            e,
                        )
                    },
                )?,
            )
        } else {
            None
        };

        let end_time = if let Some(timestamp) = time_range.end {
            let timestamp_nanos = timestamp.timestamp_nanos_opt().ok_or_else(|| {
                RepositoryError::storage("End timestamp out of range".to_string())
            })?;
            Some(
                time::OffsetDateTime::from_unix_timestamp_nanos(timestamp_nanos as i128).map_err(
                    |e| {
                        RepositoryError::storage_with_source("Invalid end timestamp".to_string(), e)
                    },
                )?,
            )
        } else {
            None
        };

        let records = sqlx::query_as::<_, MessageRecord>(query)
            .bind(Uuid::from(room_id))
            .bind(start_time)
            .bind(end_time)
            .bind(pagination.limit)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        records.into_iter().map(Message::try_from).collect()
    }

    // === 向后兼容方法 ===
    // 注意：向后兼容方法已移动到 trait 的默认实现中
}

pub async fn create_pg_pool(
    database_url: &str,
    max_connections: u32,
) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await
}

// MessageDelivery相关的实现

#[derive(Debug, FromRow)]
struct MessageDeliveryRecord {
    message_id: Uuid,
    user_id: Uuid,
    sent_at: OffsetDateTime,
    delivered_at: Option<OffsetDateTime>,
}

impl From<MessageDeliveryRecord> for MessageDelivery {
    fn from(value: MessageDeliveryRecord) -> Self {
        Self {
            message_id: MessageId::from(value.message_id),
            user_id: UserId::from(value.user_id),
            sent_at: value.sent_at,
            delivered_at: value.delivered_at,
        }
    }
}

#[derive(Clone)]
pub struct PgMessageDeliveryRepository {
    pool: PgPool,
}

impl PgMessageDeliveryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MessageDeliveryRepository for PgMessageDeliveryRepository {
    async fn record_sent(&self, delivery: MessageDelivery) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO message_deliveries (message_id, user_id, sent_at, delivered_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (message_id, user_id) DO NOTHING
            "#,
        )
        .bind(Uuid::from(delivery.message_id))
        .bind(Uuid::from(delivery.user_id))
        .bind(delivery.sent_at)
        .bind(delivery.delivered_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(())
    }

    async fn mark_delivered(
        &self,
        message_id: MessageId,
        user_id: UserId,
        delivered_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepositoryError> {
        // 精确转换时间类型
        let timestamp_nanos = delivered_at
            .timestamp_nanos_opt()
            .ok_or_else(|| RepositoryError::storage("Timestamp out of range".to_string()))?;

        let time_offset = time::OffsetDateTime::from_unix_timestamp_nanos(timestamp_nanos as i128)
            .map_err(|e| {
                RepositoryError::storage_with_source("Invalid timestamp".to_string(), e)
            })?;

        sqlx::query(
            r#"
            UPDATE message_deliveries
            SET delivered_at = $3
            WHERE message_id = $1 AND user_id = $2 AND delivered_at IS NULL
            "#,
        )
        .bind(Uuid::from(message_id))
        .bind(Uuid::from(user_id))
        .bind(time_offset)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(())
    }

    async fn find_undelivered_for_user(&self, user_id: UserId) -> Result<Vec<MessageDelivery>, RepositoryError> {
        let records = sqlx::query_as::<_, MessageDeliveryRecord>(
            r#"
            SELECT message_id, user_id, sent_at, delivered_at
            FROM message_deliveries
            WHERE user_id = $1 AND delivered_at IS NULL
            ORDER BY sent_at ASC
            "#,
        )
        .bind(Uuid::from(user_id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(records.into_iter().map(MessageDelivery::from).collect())
    }

    async fn find_by_message(&self, message_id: MessageId) -> Result<Vec<MessageDelivery>, RepositoryError> {
        let records = sqlx::query_as::<_, MessageDeliveryRecord>(
            r#"
            SELECT message_id, user_id, sent_at, delivered_at
            FROM message_deliveries
            WHERE message_id = $1
            ORDER BY sent_at ASC
            "#,
        )
        .bind(Uuid::from(message_id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(records.into_iter().map(MessageDelivery::from).collect())
    }

    async fn cleanup_delivered_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, RepositoryError> {
        // 精确转换时间类型
        let timestamp_nanos = before
            .timestamp_nanos_opt()
            .ok_or_else(|| RepositoryError::storage("Timestamp out of range".to_string()))?;

        let time_offset = time::OffsetDateTime::from_unix_timestamp_nanos(timestamp_nanos as i128)
            .map_err(|e| {
                RepositoryError::storage_with_source("Invalid timestamp".to_string(), e)
            })?;

        let result = sqlx::query(
            r#"
            DELETE FROM message_deliveries
            WHERE delivered_at IS NOT NULL AND delivered_at < $1
            "#,
        )
        .bind(time_offset)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(result.rows_affected())
    }
}
