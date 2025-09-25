use std::sync::Arc;

use domain::{
    ChatRoom, ChatRoomRepository, ChatRoomVisibility, Message, MessageContent, MessageId,
    MessageRepository, MessageRevision, MessageType, RepositoryError, RepositoryFuture, RoomId,
    RoomMember, RoomMemberRepository, RoomRole, User, UserEmail, UserId, UserRepository,
    UserStatus,
};
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

fn map_sqlx_err(err: sqlx::Error) -> RepositoryError {
    RepositoryError::storage(err.to_string())
}

fn invalid_data(message: impl Into<String>) -> RepositoryError {
    RepositoryError::storage(message)
}

#[derive(Debug, FromRow)]
struct UserRecord {
    id: Uuid,
    username: String,
    email: String,
    password_hash: String,
    status: UserStatus,  // 直接使用枚举类型
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
            status: value.status,  // 直接使用，不需要转换
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
    role: RoomRole,  // 直接使用枚举类型
    joined_at: OffsetDateTime,
    last_read_message_id: Option<Uuid>,
}

impl TryFrom<MemberRecord> for RoomMember {
    type Error = RepositoryError;

    fn try_from(value: MemberRecord) -> Result<Self, Self::Error> {
        Ok(RoomMember {
            room_id: RoomId::from(value.room_id),
            user_id: UserId::from(value.user_id),
            role: value.role,  // 直接使用，不需要转换
            joined_at: value.joined_at,
            last_read_message: value.last_read_message_id.map(MessageId::from),
        })
    }
}

#[derive(Debug, FromRow)]
struct MessageRecord {
    id: Uuid,
    room_id: Uuid,
    user_id: Uuid,
    content: String,
    message_type: MessageType,  // 直接使用枚举类型
    reply_to_message_id: Option<Uuid>,
    created_at: OffsetDateTime,
    updated_at: Option<OffsetDateTime>,
    is_deleted: bool,
}

impl TryFrom<MessageRecord> for Message {
    type Error = RepositoryError;

    fn try_from(value: MessageRecord) -> Result<Self, Self::Error> {
        let content =
            MessageContent::new(value.content).map_err(|err| invalid_data(err.to_string()))?;
        let mut message = Message::new(
            MessageId::from(value.id),
            RoomId::from(value.room_id),
            UserId::from(value.user_id),
            content,
            value.message_type,  // 直接使用，不需要转换
            value.reply_to_message_id.map(MessageId::from),
            value.created_at,
        )?;
        message.is_deleted = value.is_deleted;
        message.last_revision =
            value
                .updated_at
                .filter(|ts| *ts > value.created_at)
                .map(|updated_at| MessageRevision {
                    content: message.content.clone(),
                    updated_at,
                });
        Ok(message)
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

impl UserRepository for PgUserRepository {
    fn create(&self, user: User) -> RepositoryFuture<User> {
        let pool = self.pool.clone();
        Box::pin(async move {
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
            .bind(&user.status)  // 直接绑定枚举
            .bind(user.created_at)
            .bind(user.updated_at)
            .fetch_one(&pool)
            .await
            .map_err(map_sqlx_err)?;

            User::try_from(record)
        })
    }

    fn update(&self, user: User) -> RepositoryFuture<User> {
        let pool = self.pool.clone();
        Box::pin(async move {
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
            .bind(&user.status)  // 直接绑定枚举
            .bind(user.updated_at)
            .fetch_one(&pool)
            .await
            .map_err(map_sqlx_err)?;

            User::try_from(record)
        })
    }

    fn find_by_id(&self, id: UserId) -> RepositoryFuture<Option<User>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let record = sqlx::query_as::<_, UserRecord>(
                r#"SELECT id, username, email, password_hash, status, created_at, updated_at FROM users WHERE id = $1"#,
            )
            .bind(Uuid::from(id))
            .fetch_optional(&pool)
            .await
            .map_err(map_sqlx_err)?;

            record.map(User::try_from).transpose()
        })
    }

    fn find_by_email(&self, email: UserEmail) -> RepositoryFuture<Option<User>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let record = sqlx::query_as::<_, UserRecord>(
                r#"SELECT id, username, email, password_hash, status, created_at, updated_at FROM users WHERE email = $1"#,
            )
            .bind(email.as_str())
            .fetch_optional(&pool)
            .await
            .map_err(map_sqlx_err)?;

            record.map(User::try_from).transpose()
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

impl ChatRoomRepository for PgChatRoomRepository {
    fn create(&self, room: ChatRoom) -> RepositoryFuture<ChatRoom> {
        let pool = self.pool.clone();
        Box::pin(async move {
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
            .fetch_one(&pool)
            .await
            .map_err(map_sqlx_err)?;

            ChatRoom::try_from(record)
        })
    }

    fn update(&self, room: ChatRoom) -> RepositoryFuture<ChatRoom> {
        let pool = self.pool.clone();
        Box::pin(async move {
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
            .fetch_one(&pool)
            .await
            .map_err(map_sqlx_err)?;

            ChatRoom::try_from(record)
        })
    }

    fn delete(&self, id: RoomId) -> RepositoryFuture<()> {
        let pool = self.pool.clone();
        Box::pin(async move {
            sqlx::query("DELETE FROM chat_rooms WHERE id = $1")
                .bind(Uuid::from(id))
                .execute(&pool)
                .await
                .map_err(map_sqlx_err)?;

            Ok(())
        })
    }

    fn find_by_id(&self, id: RoomId) -> RepositoryFuture<Option<ChatRoom>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let record = sqlx::query_as::<_, RoomRecord>(
                r#"SELECT id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed FROM chat_rooms WHERE id = $1"#,
            )
            .bind(Uuid::from(id))
            .fetch_optional(&pool)
            .await
            .map_err(map_sqlx_err)?;

            record.map(ChatRoom::try_from).transpose()
        })
    }

    fn list_by_owner(&self, owner: UserId) -> RepositoryFuture<Vec<ChatRoom>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let records = sqlx::query_as::<_, RoomRecord>(
                r#"SELECT id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed FROM chat_rooms WHERE owner_id = $1 ORDER BY created_at DESC"#,
            )
            .bind(Uuid::from(owner))
            .fetch_all(&pool)
            .await
            .map_err(map_sqlx_err)?;

            records.into_iter().map(ChatRoom::try_from).collect()
        })
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

impl RoomMemberRepository for PgRoomMemberRepository {
    fn upsert(&self, member: RoomMember) -> RepositoryFuture<RoomMember> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let record = sqlx::query_as::<_, MemberRecord>(
                r#"
                INSERT INTO room_members (room_id, user_id, role, joined_at, last_read_message_id)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (room_id, user_id)
                DO UPDATE SET role = EXCLUDED.role, last_read_message_id = EXCLUDED.last_read_message_id
                RETURNING room_id, user_id, role, joined_at, last_read_message_id
                "#,
            )
            .bind(Uuid::from(member.room_id))
            .bind(Uuid::from(member.user_id))
            .bind(&member.role)  // 直接绑定枚举
            .bind(member.joined_at)
            .bind(member.last_read_message.map(Uuid::from))
            .fetch_one(&pool)
            .await
            .map_err(map_sqlx_err)?;

            RoomMember::try_from(record)
        })
    }

    fn remove(&self, room_id: RoomId, user_id: UserId) -> RepositoryFuture<()> {
        let pool = self.pool.clone();
        Box::pin(async move {
            sqlx::query(r#"DELETE FROM room_members WHERE room_id = $1 AND user_id = $2"#)
                .bind(Uuid::from(room_id))
                .bind(Uuid::from(user_id))
                .execute(&pool)
                .await
                .map_err(map_sqlx_err)?;
            Ok(())
        })
    }

    fn find(&self, room_id: RoomId, user_id: UserId) -> RepositoryFuture<Option<RoomMember>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let record = sqlx::query_as::<_, MemberRecord>(
                r#"SELECT room_id, user_id, role, joined_at, last_read_message_id FROM room_members WHERE room_id = $1 AND user_id = $2"#,
            )
            .bind(Uuid::from(room_id))
            .bind(Uuid::from(user_id))
            .fetch_optional(&pool)
            .await
            .map_err(map_sqlx_err)?;

            record.map(RoomMember::try_from).transpose()
        })
    }

    fn list_members(&self, room_id: RoomId) -> RepositoryFuture<Vec<RoomMember>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let records = sqlx::query_as::<_, MemberRecord>(
                r#"SELECT room_id, user_id, role, joined_at, last_read_message_id FROM room_members WHERE room_id = $1"#,
            )
            .bind(Uuid::from(room_id))
            .fetch_all(&pool)
            .await
            .map_err(map_sqlx_err)?;

            records.into_iter().map(RoomMember::try_from).collect()
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

impl MessageRepository for PgMessageRepository {
    fn create(&self, message: Message) -> RepositoryFuture<Message> {
        let pool = self.pool.clone();
        Box::pin(async move {
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
            .bind(&message.message_type)  // 直接绑定枚举
            .bind(message.reply_to.map(Uuid::from))
            .bind(message.created_at)
            .bind(message.last_revision.as_ref().map(|rev| rev.updated_at))
            .bind(message.is_deleted)
            .fetch_one(&pool)
            .await
            .map_err(map_sqlx_err)?;

            Message::try_from(record)
        })
    }

    fn find_by_id(&self, id: MessageId) -> RepositoryFuture<Option<Message>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let record = sqlx::query_as::<_, MessageRecord>(
                r#"SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted FROM messages WHERE id = $1"#,
            )
            .bind(Uuid::from(id))
            .fetch_optional(&pool)
            .await
            .map_err(map_sqlx_err)?;

            record.map(Message::try_from).transpose()
        })
    }

    fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> RepositoryFuture<Vec<Message>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            let records = if let Some(before_id) = before {
                let cutoff: Option<OffsetDateTime> =
                    sqlx::query_scalar(r#"SELECT created_at FROM messages WHERE id = $1"#)
                        .bind(Uuid::from(before_id))
                        .fetch_optional(&pool)
                        .await
                        .map_err(map_sqlx_err)?;

                if let Some(cutoff) = cutoff {
                    sqlx::query_as::<_, MessageRecord>(
                        r#"SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
                        FROM messages
                        WHERE room_id = $1 AND created_at < $2
                        ORDER BY created_at DESC
                        LIMIT $3"#,
                    )
                    .bind(Uuid::from(room_id))
                    .bind(cutoff)
                    .bind(limit as i64)
                    .fetch_all(&pool)
                    .await
                    .map_err(map_sqlx_err)?
                } else {
                    Vec::new()
                }
            } else {
                sqlx::query_as::<_, MessageRecord>(
                    r#"SELECT id, room_id, user_id, content, message_type, reply_to_message_id, created_at, updated_at, is_deleted
                    FROM messages
                    WHERE room_id = $1
                    ORDER BY created_at DESC
                    LIMIT $2"#,
                )
                .bind(Uuid::from(room_id))
                .bind(limit as i64)
                .fetch_all(&pool)
                .await
                .map_err(map_sqlx_err)?
            };

            let mut items: Vec<Message> = records
                .into_iter()
                .map(Message::try_from)
                .collect::<Result<_, _>>()?;
            items.reverse();
            Ok(items)
        })
    }
}

#[derive(Clone)]
pub struct PgStorage {
    pub pool: PgPool,
    pub user_repository: Arc<PgUserRepository>,
    pub room_repository: Arc<PgChatRoomRepository>,
    pub member_repository: Arc<PgRoomMemberRepository>,
    pub message_repository: Arc<PgMessageRepository>,
}

impl PgStorage {
    pub fn new(pool: PgPool) -> Self {
        Self {
            user_repository: Arc::new(PgUserRepository::new(pool.clone())),
            room_repository: Arc::new(PgChatRoomRepository::new(pool.clone())),
            member_repository: Arc::new(PgRoomMemberRepository::new(pool.clone())),
            message_repository: Arc::new(PgMessageRepository::new(pool.clone())),
            pool,
        }
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
