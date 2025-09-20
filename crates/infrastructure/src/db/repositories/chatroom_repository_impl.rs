//! 聊天室Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    entities::chatroom::{ChatRoom, ChatRoomStatus},
    errors::{DomainError, DomainResult},
    repositories::{
        chatroom_repository::{
            ChatRoomRepository, ChatRoomSearchParams, ChatRoomStatistics, RoomActivityStats,
        },
        PaginatedResult, Pagination, SortConfig,
    },
};
use sqlx::{query, query_as, FromRow, Row};
use std::sync::Arc;
use uuid::Uuid;

/// 数据库聊天室模型
#[derive(Debug, Clone, FromRow)]
struct DbChatRoom {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub password_hash: Option<String>,
    pub owner_id: Uuid,
    pub max_members: Option<i32>,
    pub member_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
}

impl From<DbChatRoom> for ChatRoom {
    fn from(db_room: DbChatRoom) -> Self {
        let status = match db_room.status.as_str() {
            "active" => ChatRoomStatus::Active,
            "archived" => ChatRoomStatus::Archived,
            "deleted" => ChatRoomStatus::Deleted,
            _ => ChatRoomStatus::Active,
        };

        ChatRoom::with_id(
            db_room.id,
            db_room.name,
            db_room.description,
            db_room.is_private,
            db_room.password_hash,
            db_room.owner_id,
            db_room.max_members.map(|m| m as u32),
            db_room.member_count as u32,
            status,
            db_room.created_at,
            db_room.updated_at,
            db_room.last_activity_at,
        )
        .unwrap() // 从数据库加载的数据应该是有效的
    }
}

impl From<&ChatRoom> for DbChatRoom {
    fn from(room: &ChatRoom) -> Self {
        DbChatRoom {
            id: room.id,
            name: room.name.clone(),
            description: room.description.clone(),
            is_private: room.is_private,
            password_hash: room.password_hash.clone(),
            owner_id: room.owner_id,
            max_members: room.max_members.map(|m| m as i32),
            member_count: room.member_count as i32,
            status: room.status.to_string(),
            created_at: room.created_at,
            updated_at: room.updated_at,
            last_activity_at: room.last_activity_at,
        }
    }
}

/// 聊天室Repository实现
pub struct PostgresChatRoomRepository {
    pool: Arc<DbPool>,
}

impl PostgresChatRoomRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    /// 构建搜索查询条件
    fn build_search_query(params: &ChatRoomSearchParams) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut values = Vec::new();
        let mut param_count = 1;

        if let Some(name) = &params.name {
            conditions.push(format!("name ILIKE ${}", param_count));
            values.push(format!("%{}%", name));
            param_count += 1;
        }

        if let Some(owner_id) = &params.owner_id {
            conditions.push(format!("owner_id = ${}", param_count));
            values.push(owner_id.to_string());
            param_count += 1;
        }

        if let Some(is_private) = params.is_private {
            conditions.push(format!("is_private = ${}", param_count));
            values.push(is_private.to_string());
            param_count += 1;
        }

        if let Some(status) = &params.status {
            conditions.push(format!("status = ${}", param_count));
            values.push(status.to_string());
            param_count += 1;
        }

        if let Some(min_members) = params.min_members {
            conditions.push(format!("member_count >= ${}", param_count));
            values.push(min_members.to_string());
            param_count += 1;
        }

        if let Some(max_members) = params.max_members {
            conditions.push(format!("member_count <= ${}", param_count));
            values.push(max_members.to_string());
            param_count += 1;
        }

        if let Some(created_after) = &params.created_after {
            conditions.push(format!("created_at > ${}", param_count));
            values.push(created_after.to_string());
            param_count += 1;
        }

        if let Some(active_since) = &params.last_activity_after {
            conditions.push(format!("last_activity_at > ${}", param_count));
            values.push(active_since.to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, values)
    }
}

#[async_trait]
impl ChatRoomRepository for PostgresChatRoomRepository {
    async fn create(&self, room: &ChatRoom) -> DomainResult<ChatRoom> {
        let db_room = DbChatRoom::from(room);

        let result = query_as::<_, DbChatRoom>(
            r#"
            INSERT INTO chat_rooms (id, name, description, is_private, password_hash, owner_id,
                                   max_members, member_count, status, last_activity_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, name, description, is_private, password_hash, owner_id,
                     max_members, member_count, status, created_at, updated_at, last_activity_at
            "#,
        )
        .bind(db_room.id)
        .bind(&db_room.name)
        .bind(&db_room.description)
        .bind(db_room.is_private)
        .bind(&db_room.password_hash)
        .bind(db_room.owner_id)
        .bind(db_room.max_members)
        .bind(db_room.member_count)
        .bind(&db_room.status)
        .bind(db_room.last_activity_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<ChatRoom>> {
        let result = query_as::<_, DbChatRoom>(
            r#"
            SELECT id, name, description, is_private, password_hash, owner_id,
                   max_members, member_count, status, created_at, updated_at, last_activity_at
            FROM chat_rooms
            WHERE id = $1 AND status != 'deleted'
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn find_by_name(&self, name: &str) -> DomainResult<Option<ChatRoom>> {
        let result = query_as::<_, DbChatRoom>(
            r#"
            SELECT id, name, description, is_private, password_hash, owner_id,
                   max_members, member_count, status, created_at, updated_at, last_activity_at
            FROM chat_rooms
            WHERE name = $1 AND status != 'deleted'
            "#,
        )
        .bind(name)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn find_by_owner(
        &self,
        owner_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>> {
        // 获取总数
        let total_count: i64 =
            query("SELECT COUNT(*) FROM chat_rooms WHERE owner_id = $1 AND status != 'deleted'")
                .bind(owner_id)
                .fetch_one(&*self.pool)
                .await
                .map_err(|e| DomainError::database_error(e.to_string()))?
                .get(0);

        // 获取数据
        let rooms: Vec<DbChatRoom> = query_as(
            r#"
            SELECT id, name, description, is_private, password_hash, owner_id,
                   max_members, member_count, status, created_at, updated_at, last_activity_at
            FROM chat_rooms
            WHERE owner_id = $1 AND status != 'deleted'
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(owner_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let rooms: Vec<ChatRoom> = rooms.into_iter().map(|r| r.into()).collect();

        Ok(PaginatedResult::new(rooms, total_count as u64, pagination))
    }

    async fn update(&self, room: &ChatRoom) -> DomainResult<ChatRoom> {
        let db_room = DbChatRoom::from(room);

        let result = query_as::<_, DbChatRoom>(
            r#"
            UPDATE chat_rooms
            SET name = $2, description = $3, is_private = $4, password_hash = $5,
                max_members = $6, member_count = $7, status = $8,
                last_activity_at = $9, updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, description, is_private, password_hash, owner_id,
                     max_members, member_count, status, created_at, updated_at, last_activity_at
            "#,
        )
        .bind(db_room.id)
        .bind(&db_room.name)
        .bind(&db_room.description)
        .bind(db_room.is_private)
        .bind(&db_room.password_hash)
        .bind(db_room.max_members)
        .bind(db_room.member_count)
        .bind(&db_room.status)
        .bind(db_room.last_activity_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_member_count(&self, room_id: Uuid, count: u32) -> DomainResult<()> {
        query("UPDATE chat_rooms SET member_count = $2, updated_at = NOW() WHERE id = $1")
            .bind(room_id)
            .bind(count as i32)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn update_last_activity(
        &self,
        room_id: Uuid,
        last_activity_at: DateTime<Utc>,
    ) -> DomainResult<()> {
        query("UPDATE chat_rooms SET last_activity_at = $2 WHERE id = $1")
            .bind(room_id)
            .bind(last_activity_at)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn soft_delete(&self, room_id: Uuid) -> DomainResult<()> {
        query("UPDATE chat_rooms SET status = 'deleted', updated_at = NOW() WHERE id = $1")
            .bind(room_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn search(
        &self,
        params: &ChatRoomSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<ChatRoom>> {
        let (where_clause, _values) = Self::build_search_query(params);

        let order_clause = match sort {
            Some(sort_config) => {
                let direction = if sort_config.ascending { "ASC" } else { "DESC" };
                format!("ORDER BY {} {}", sort_config.field, direction)
            }
            None => "ORDER BY created_at DESC".to_string(),
        };

        let base_query = format!(
            "FROM chat_rooms {} AND status != 'deleted'",
            if where_clause.is_empty() {
                "WHERE 1=1".to_string()
            } else {
                where_clause
            }
        );

        // 获取总数
        let count_query = format!("SELECT COUNT(*) {}", base_query);
        let total_count: i64 = query(&count_query)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取数据
        let data_query = format!(
            r#"
            SELECT id, name, description, is_private, password_hash, owner_id,
                   max_members, member_count, status, created_at, updated_at, last_activity_at
            {} {} LIMIT {} OFFSET {}
            "#,
            base_query, order_clause, pagination.limit, pagination.offset
        );

        let rooms: Vec<DbChatRoom> = query_as(&data_query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        let rooms: Vec<ChatRoom> = rooms.into_iter().map(|r| r.into()).collect();

        Ok(PaginatedResult::new(rooms, total_count as u64, pagination))
    }

    async fn get_statistics(&self) -> DomainResult<ChatRoomStatistics> {
        let row = query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status != 'deleted') as total_rooms,
                COUNT(*) FILTER (WHERE status = 'active') as active_rooms,
                COUNT(*) FILTER (WHERE status = 'archived') as archived_rooms,
                COUNT(*) FILTER (WHERE is_private = true AND status != 'deleted') as private_rooms,
                COUNT(*) FILTER (WHERE is_private = false AND status != 'deleted') as public_rooms,
                COUNT(*) FILTER (WHERE created_at::date = CURRENT_DATE) as rooms_created_today,
                AVG(member_count) FILTER (WHERE status != 'deleted') as avg_members_per_room,
                COUNT(*) FILTER (WHERE last_activity_at > NOW() - INTERVAL '1 hour' AND status = 'active') as rooms_active_last_hour
            FROM chat_rooms
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(ChatRoomStatistics {
            total_rooms: row.get::<i64, _>("total_rooms") as u64,
            active_rooms: row.get::<i64, _>("active_rooms") as u64,
            private_rooms: row.get::<i64, _>("private_rooms") as u64,
            public_rooms: row.get::<i64, _>("public_rooms") as u64,
            rooms_created_today: row.get::<i64, _>("rooms_created_today") as u64,
            rooms_with_activity_today: row.get::<i64, _>("rooms_active_last_hour") as u64,
            avg_members_per_room: row
                .get::<Option<f64>, _>("avg_members_per_room")
                .unwrap_or(0.0),
        })
    }

    async fn find_most_active(&self, limit: u32) -> DomainResult<Vec<ChatRoom>> {
        let rooms: Vec<DbChatRoom> = query_as(
            r#"
            SELECT id, name, description, is_private, password_hash, owner_id,
                   max_members, member_count, status, created_at, updated_at, last_activity_at
            FROM chat_rooms
            WHERE status = 'active' AND is_private = false
            ORDER BY member_count DESC, last_activity_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(rooms.into_iter().map(|r| r.into()).collect())
    }

    async fn find_recent_rooms(&self, limit: u32) -> DomainResult<Vec<ChatRoom>> {
        let rooms: Vec<DbChatRoom> = query_as(
            r#"
            SELECT id, name, description, is_private, password_hash, owner_id,
                   max_members, member_count, status, created_at, updated_at, last_activity_at
            FROM chat_rooms
            WHERE status = 'active'
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(rooms.into_iter().map(|r| r.into()).collect())
    }

    async fn find_public_rooms(
        &self,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>> {
        // 获取总数
        let total_count: i64 =
            query("SELECT COUNT(*) FROM chat_rooms WHERE status = 'active' AND is_private = false")
                .fetch_one(&*self.pool)
                .await
                .map_err(|e| DomainError::database_error(e.to_string()))?
                .get(0);

        // 获取数据
        let rooms: Vec<DbChatRoom> = query_as(
            r#"
            SELECT id, name, description, is_private, password_hash, owner_id,
                   max_members, member_count, status, created_at, updated_at, last_activity_at
            FROM chat_rooms
            WHERE status = 'active' AND is_private = false
            ORDER BY member_count DESC, last_activity_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let rooms: Vec<ChatRoom> = rooms.into_iter().map(|r| r.into()).collect();
        Ok(PaginatedResult::new(rooms, total_count as u64, pagination))
    }

    async fn find_by_member(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>> {
        // 获取总数
        let total_count: i64 = query(
            r#"
            SELECT COUNT(*)
            FROM chat_rooms cr
            JOIN room_members rm ON cr.id = rm.room_id
            WHERE rm.user_id = $1 AND cr.status != 'deleted'
            "#,
        )
        .bind(user_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        // 获取数据
        let rooms: Vec<DbChatRoom> = query_as(
            r#"
            SELECT cr.id, cr.name, cr.description, cr.is_private, cr.password_hash, cr.owner_id,
                   cr.max_members, cr.member_count, cr.status, cr.created_at, cr.updated_at, cr.last_activity_at
            FROM chat_rooms cr
            JOIN room_members rm ON cr.id = rm.room_id
            WHERE rm.user_id = $1 AND cr.status != 'deleted'
            ORDER BY cr.last_activity_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let rooms: Vec<ChatRoom> = rooms.into_iter().map(|r| r.into()).collect();
        Ok(PaginatedResult::new(rooms, total_count as u64, pagination))
    }

    async fn name_exists(&self, name: &str) -> DomainResult<bool> {
        let count: i64 =
            query("SELECT COUNT(*) FROM chat_rooms WHERE name = $1 AND status != 'deleted'")
                .bind(name)
                .fetch_one(&*self.pool)
                .await
                .map_err(|e| DomainError::database_error(e.to_string()))?
                .get(0);

        Ok(count > 0)
    }

    async fn verify_password(&self, room_id: Uuid, password: &str) -> DomainResult<bool> {
        let room = self.find_by_id(room_id).await?;
        match room {
            Some(room) => room.verify_password(password),
            None => Ok(false),
        }
    }

    async fn set_password(&self, room_id: Uuid, password_hash: Option<&str>) -> DomainResult<()> {
        query("UPDATE chat_rooms SET password_hash = $2, is_private = $3, updated_at = NOW() WHERE id = $1")
            .bind(room_id)
            .bind(password_hash)
            .bind(password_hash.is_some())
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn update_status(&self, room_id: Uuid, status: ChatRoomStatus) -> DomainResult<()> {
        query("UPDATE chat_rooms SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(room_id)
            .bind(status.to_string())
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn get_activity_stats(
        &self,
        room_id: Uuid,
        _date_from: DateTime<Utc>,
        _date_to: DateTime<Utc>,
    ) -> DomainResult<RoomActivityStats> {
        // 这里应该从messages表和其他相关表获取活动统计，暂时返回默认值
        Ok(RoomActivityStats {
            room_id,
            message_count: 0,
            active_members: 0,
            last_message_at: None,
            peak_concurrent_members: 0,
        })
    }

    async fn cleanup_inactive_rooms(&self, inactive_days: u32) -> DomainResult<u64> {
        let result = query(
            r#"
            DELETE FROM chat_rooms
            WHERE status = 'deleted'
            AND updated_at < NOW() - INTERVAL '$1 days'
            "#,
        )
        .bind(inactive_days as i32)
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected())
    }

    #[cfg(feature = "enterprise")]
    async fn find_by_organization(
        &self,
        _org_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>> {
        // 企业版功能，暂时返回空结果
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }

    async fn find_by_ids(&self, ids: &[Uuid]) -> DomainResult<Vec<ChatRoom>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let chatrooms: Vec<DbChatRoom> = query_as(
            r#"
            SELECT id, name, description, room_type, is_public, max_members, creator_id,
                   created_at, updated_at, status, avatar_url, settings, room_password_hash,
                   last_activity_at
            FROM chatrooms
            WHERE id = ANY($1) AND status != 'deleted'
            ORDER BY created_at DESC
            "#,
        )
        .bind(ids)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(chatrooms.into_iter().map(|c| c.into()).collect())
    }
}
