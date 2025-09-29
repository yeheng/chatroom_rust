/// Linus式的简单事务解决方案
/// 不搞复杂的UnitOfWork抽象，直接解决create_room的事务问题
use async_trait::async_trait;
use domain::{ChatRoom, ChatRoomVisibility, RepositoryError, RoomMember};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use application::{error::ApplicationError, services::TransactionManager};
use crate::repository::map_sqlx_err;

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
            Some(hash) => Some(domain::PasswordHash::new(hash).map_err(|e| RepositoryError::storage_with_source(e.to_string(), e))?),
            None => None,
        };

        Ok(ChatRoom {
            id: domain::RoomId::from(value.id),
            name: value.name,
            owner_id: domain::UserId::from(value.owner_id),
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
pub struct SimpleTransactionManager {
    pool: PgPool,
}

impl SimpleTransactionManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 事务性地创建房间和成员 - 这解决了create_room的核心问题
    pub async fn create_room_with_owner(
        &self,
        room: ChatRoom,
        owner_member: RoomMember,
    ) -> Result<ChatRoom, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_err)?;

        // 1. 创建房间
        let room_record = sqlx::query_as::<_, RoomRecord>(
            r#"
            INSERT INTO chat_rooms (id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, name, owner_id, is_private, password_hash, created_at, updated_at, is_closed
            "#,
        )
        .bind(Uuid::from(room.id))
        .bind(room.name.clone())
        .bind(Uuid::from(room.owner_id))
        .bind(matches!(room.visibility, ChatRoomVisibility::Private))
        .bind(room.password.as_ref().map(|hash| hash.as_str()))
        .bind(room.created_at)
        .bind(room.updated_at)
        .bind(room.is_closed)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx_err)?;

        // 2. 创建owner成员记录
        sqlx::query(
            r#"
            INSERT INTO room_members (room_id, user_id, role, joined_at, last_read_message_id)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::from(owner_member.room_id))
        .bind(Uuid::from(owner_member.user_id))
        .bind(&owner_member.role)
        .bind(owner_member.joined_at)
        .bind(owner_member.last_read_message.map(Uuid::from))
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_err)?;

        // 3. 提交事务
        tx.commit().await.map_err(map_sqlx_err)?;

        // 4. 返回创建的房间
        ChatRoom::try_from(room_record)
    }
}

#[async_trait]
impl TransactionManager for SimpleTransactionManager {
    async fn create_room_with_owner(
        &self,
        room: ChatRoom,
        owner_member: RoomMember,
    ) -> Result<ChatRoom, ApplicationError> {
        self.create_room_with_owner(room, owner_member)
            .await
            .map_err(ApplicationError::from)
    }
}