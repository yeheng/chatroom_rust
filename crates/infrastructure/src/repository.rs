use std::sync::Arc;

use application::repository::{
    ChatRoomRepository, MessageRepository, RoomMemberRepository, UserRepository,
};
use async_trait::async_trait;
use domain::{
    ChatRoom, ChatRoomVisibility, Message, MessageContent, MessageId, MessageType, RepositoryError,
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
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<UserRecord> for User {
    type Error = RepositoryError;

    fn try_from(value: UserRecord) -> Result<Self, Self::Error> {
        let username = domain::Username::parse(value.username).map_err(|err| invalid_data(err))?;
        let email = domain::UserEmail::parse(value.email).map_err(|err| invalid_data(err))?;
        let password =
            domain::PasswordHash::new(value.password_hash).map_err(|err| invalid_data(err))?;

        Ok(User {
            id: UserId::from(value.id),
            username,
            email,
            password,
            status: value.status,
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
            INSERT INTO users (id, username, email, password_hash, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, username, email, password_hash, status, created_at, updated_at
            "#,
        )
        .bind(Uuid::from(user.id))
        .bind(user.username.as_str())
        .bind(user.email.as_str())
        .bind(user.password.as_str())
        .bind(&user.status)
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
            SET username = $2, email = $3, password_hash = $4, status = $5, updated_at = $6
            WHERE id = $1
            RETURNING id, username, email, password_hash, status, created_at, updated_at
            "#,
        )
        .bind(Uuid::from(user.id))
        .bind(user.username.as_str())
        .bind(user.email.as_str())
        .bind(user.password.as_str())
        .bind(&user.status)
        .bind(user.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        User::try_from(record)
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let record = sqlx::query_as::<_, UserRecord>(
            r#"SELECT id, username, email, password_hash, status, created_at, updated_at FROM users WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(User::try_from).transpose()
    }

    async fn find_by_email(&self, email: UserEmail) -> Result<Option<User>, RepositoryError> {
        let record = sqlx::query_as::<_, UserRecord>(
            r#"SELECT id, username, email, password_hash, status, created_at, updated_at FROM users WHERE email = $1"#,
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
            Some(hash) => Some(domain::PasswordHash::new(hash).map_err(|err| invalid_data(err))?),
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

    async fn list_by_owner(&self, owner_id: UserId) -> Result<Vec<ChatRoom>, RepositoryError> {
        let records = sqlx::query_as::<_, RoomRecord>(
            r#"SELECT id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed FROM chat_rooms WHERE owner_id = $1"#,
        )
        .bind(Uuid::from(owner_id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        records.into_iter().map(ChatRoom::try_from).collect()
    }
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

    async fn find(
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

    async fn remove(&self, room_id: RoomId, user_id: UserId) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM room_members WHERE room_id = $1 AND user_id = $2")
            .bind(Uuid::from(room_id))
            .bind(Uuid::from(user_id))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        Ok(())
    }

    async fn list_members(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError> {
        let records = sqlx::query_as::<_, MemberRecord>(
            r#"SELECT room_id, user_id, role, joined_at, last_read_message_id FROM room_members WHERE room_id = $1"#,
        )
        .bind(Uuid::from(room_id))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(records.into_iter().map(RoomMember::from).collect())
    }
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
    is_deleted: bool,
}

impl TryFrom<MessageRecord> for Message {
    type Error = RepositoryError;

    fn try_from(value: MessageRecord) -> Result<Self, Self::Error> {
        let content = MessageContent::new(value.content).map_err(|err| invalid_data(err))?;

        Ok(Message {
            id: MessageId::from(value.id),
            room_id: RoomId::from(value.room_id),
            sender_id: UserId::from(value.user_id),
            content,
            message_type: value.message_type,
            reply_to: value.reply_to_message_id.map(MessageId::from),
            created_at: value.created_at,
            last_revision: None,
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
    async fn save_message(&self, message: Message) -> Result<MessageId, RepositoryError> {
        let record = sqlx::query_as::<_, MessageRecord>(
            r#"
            INSERT INTO messages (id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted
            "#,
        )
        .bind(Uuid::from(message.id))
        .bind(Uuid::from(message.room_id))
        .bind(Uuid::from(message.sender_id))
        .bind(message.content.as_str())
        .bind(&message.message_type)
        .bind(message.reply_to.map(Uuid::from))
        .bind(message.created_at)
        .bind(message.is_deleted)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(MessageId::from(record.id))
    }

    async fn find_by_id(&self, id: MessageId) -> Result<Option<Message>, RepositoryError> {
        let record = sqlx::query_as::<_, MessageRecord>(
            r#"SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted FROM messages WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(Message::try_from).transpose()
    }

    // 增强的获取最近消息方法
    async fn get_recent_messages(
        &self,
        room_id: RoomId,
        limit: i64,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError> {
        let records = if let Some(before_id) = before {
            sqlx::query_as::<_, MessageRecord>(
                r#"
                SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted
                FROM messages
                WHERE room_id = $1 AND id < $2 AND is_deleted = FALSE
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(Uuid::from(room_id))
            .bind(Uuid::from(before_id))
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, MessageRecord>(
                r#"
                SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted
                FROM messages
                WHERE room_id = $1 AND is_deleted = FALSE
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(Uuid::from(room_id))
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(map_sqlx_err)?;

        records.into_iter().map(Message::try_from).collect()
    }

    // 新方法：获取指定时间之后的消息
    async fn get_messages_since(
        &self,
        room_id: RoomId,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Message>, RepositoryError> {
        // 精确转换时间类型（保留纳秒精度）
        let timestamp_nanos = timestamp.timestamp_nanos_opt()
            .ok_or_else(|| RepositoryError::storage("Timestamp out of range".to_string()))?;

        let time_offset = time::OffsetDateTime::from_unix_timestamp_nanos(timestamp_nanos as i128)
            .map_err(|e| RepositoryError::storage_with_source("Invalid timestamp".to_string(), e))?;

        let records = sqlx::query_as::<_, MessageRecord>(
            r#"
            SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted
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

    // 保留原有的 create 方法实现（通过默认实现调用新方法）
    async fn create(&self, message: Message) -> Result<Message, RepositoryError> {
        let message_id = self.save_message(message.clone()).await?;
        self.find_by_id(message_id).await?.ok_or(RepositoryError::NotFound)
    }

    // 保留原有的 list_recent 方法实现（通过默认实现调用新方法）
    async fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError> {
        self.get_recent_messages(room_id, limit as i64, before).await
    }
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
