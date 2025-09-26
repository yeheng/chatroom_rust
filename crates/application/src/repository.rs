use domain::{
    ChatRoom, ChatRoomVisibility, Message, MessageContent, MessageId,
    MessageType, RepositoryError, RoomId, RoomMember, RoomRole, User, UserEmail, UserId,
    UserStatus,
};
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

// 错误处理函数
fn map_sqlx_err(err: sqlx::Error) -> RepositoryError {
    RepositoryError::storage(err.to_string())
}

fn invalid_data(message: impl Into<String>) -> RepositoryError {
    RepositoryError::storage(message)
}

// 数据库记录结构
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
        let username =
            domain::Username::parse(value.username).map_err(|err| invalid_data(err.to_string()))?;
        let email =
            domain::UserEmail::parse(value.email).map_err(|err| invalid_data(err.to_string()))?;
        let password = domain::PasswordHash::new(value.password_hash)
            .map_err(|err| invalid_data(err.to_string()))?;

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
            Some(hash) => {
                Some(domain::PasswordHash::new(hash).map_err(|err| invalid_data(err.to_string()))?)
            }
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

#[derive(Debug, FromRow)]
struct MessageRecord {
    id: Uuid,
    room_id: Uuid,
    user_id: Uuid,
    content: String,
    message_type: String,
    reply_to_message_id: Option<Uuid>,
    created_at: OffsetDateTime,
    is_deleted: bool,
}

impl TryFrom<MessageRecord> for Message {
    type Error = RepositoryError;

    fn try_from(value: MessageRecord) -> Result<Self, Self::Error> {
        let content = MessageContent::new(value.content).map_err(|err| invalid_data(err.to_string()))?;

        let message_type = match value.message_type.as_str() {
            "text" => MessageType::Text,
            "image" => MessageType::Image,
            "file" => MessageType::File,
            _ => return Err(invalid_data(format!("Invalid message type: {}", value.message_type))),
        };

        Ok(Message {
            id: MessageId::from(value.id),
            room_id: RoomId::from(value.room_id),
            sender_id: UserId::from(value.user_id),
            content,
            message_type,
            reply_to: value.reply_to_message_id.map(MessageId::from),
            created_at: value.created_at,
            last_revision: None, // 暂时不从数据库读取修订记录
            is_deleted: value.is_deleted,
        })
    }
}

// 具体的 PostgreSQL Repository 实现 - 不再实现 trait！
#[derive(Clone)]
pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, user: User) -> Result<User, RepositoryError> {
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

    pub async fn update(&self, user: User) -> Result<User, RepositoryError> {
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

    pub async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let record = sqlx::query_as::<_, UserRecord>(
            r#"SELECT id, username, email, password_hash, status, created_at, updated_at FROM users WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(User::try_from).transpose()
    }

    pub async fn find_by_email(&self, email: UserEmail) -> Result<Option<User>, RepositoryError> {
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

#[derive(Clone)]
pub struct PgChatRoomRepository {
    pool: PgPool,
}

impl PgChatRoomRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError> {
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

    pub async fn update(&self, room: ChatRoom) -> Result<ChatRoom, RepositoryError> {
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

    pub async fn find_by_id(&self, id: RoomId) -> Result<Option<ChatRoom>, RepositoryError> {
        let record = sqlx::query_as::<_, RoomRecord>(
            r#"SELECT id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed FROM chat_rooms WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(ChatRoom::try_from).transpose()
    }

    pub async fn delete(&self, id: RoomId) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM chat_rooms WHERE id = $1")
            .bind(Uuid::from(id))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        Ok(())
    }

    pub async fn list_by_owner(&self, owner_id: UserId) -> Result<Vec<ChatRoom>, RepositoryError> {
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

#[derive(Clone)]
pub struct PgRoomMemberRepository {
    pool: PgPool,
}

impl PgRoomMemberRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, member: RoomMember) -> Result<RoomMember, RepositoryError> {
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

    pub async fn find(&self, room_id: RoomId, user_id: UserId) -> Result<Option<RoomMember>, RepositoryError> {
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

    pub async fn remove(&self, room_id: RoomId, user_id: UserId) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM room_members WHERE room_id = $1 AND user_id = $2")
            .bind(Uuid::from(room_id))
            .bind(Uuid::from(user_id))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        Ok(())
    }

    pub async fn list_members(&self, room_id: RoomId) -> Result<Vec<RoomMember>, RepositoryError> {
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

#[derive(Clone)]
pub struct PgMessageRepository {
    pool: PgPool,
}

impl PgMessageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, message: Message) -> Result<Message, RepositoryError> {
        let message_type_str = match message.message_type {
            MessageType::Text => "text",
            MessageType::Image => "image",
            MessageType::File => "file",
        };

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
        .bind(message_type_str)
        .bind(message.reply_to.map(Uuid::from))
        .bind(message.created_at)
        .bind(message.is_deleted)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Message::try_from(record)
    }

    pub async fn find_by_id(&self, id: MessageId) -> Result<Option<Message>, RepositoryError> {
        let record = sqlx::query_as::<_, MessageRecord>(
            r#"SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted FROM messages WHERE id = $1"#,
        )
        .bind(Uuid::from(id))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        record.map(Message::try_from).transpose()
    }

    pub async fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> Result<Vec<Message>, RepositoryError> {
        let records = if let Some(before_id) = before {
            sqlx::query_as::<_, MessageRecord>(
                r#"
                SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted
                FROM messages
                WHERE room_id = $1 AND id < $2
                ORDER BY created_at DESC
                LIMIT $3
                "#,
            )
            .bind(Uuid::from(room_id))
            .bind(Uuid::from(before_id))
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, MessageRecord>(
                r#"
                SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, is_deleted
                FROM messages
                WHERE room_id = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(Uuid::from(room_id))
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(map_sqlx_err)?;

        records.into_iter().map(Message::try_from).collect()
    }
}

// 数据库连接池创建函数
pub async fn create_pg_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await
}