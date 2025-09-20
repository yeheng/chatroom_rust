//! 消息Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    entities::message::{Message, MessageStatus, MessageType},
    errors::{DomainError, DomainResult},
    repositories::{
        message_repository::{
            MessageRepository, MessageSearchParams, MessageStatistics, ReplyChain,
        },
        PaginatedResult, Pagination, SortConfig,
    },
};
use serde_json::Value as JsonValue;
use sqlx::{query, query_as, FromRow, Row};
use std::sync::Arc;
use uuid::Uuid;

/// 数据库消息模型
#[derive(Debug, Clone, FromRow)]
struct DbMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Uuid,
    pub message_type: String,
    pub content: String,
    pub reply_to_id: Option<Uuid>,
    pub is_bot_message: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<DbMessage> for Message {
    fn from(db_message: DbMessage) -> Self {
        let message_type = match db_message.message_type.as_str() {
            "text" => MessageType::Text,
            "image" => MessageType::Text, // 简化处理，后续可以扩展
            "file" => MessageType::Text,  // 简化处理，后续可以扩展
            "emoji" => MessageType::Text, // 简化处理，后续可以扩展
            _ => MessageType::Text,
        };

        let status = match db_message.status.as_str() {
            "sent" => MessageStatus::Sent,
            "delivered" => MessageStatus::Delivered,
            "read" => MessageStatus::Read,
            "deleted" => MessageStatus::Deleted,
            "recalled" => MessageStatus::Recalled,
            _ => MessageStatus::Sent,
        };

        Message::with_id(
            db_message.id,
            db_message.room_id,
            db_message.sender_id,
            message_type,
            db_message.content,
            db_message.reply_to_id,
            db_message.is_bot_message,
            status,
            db_message.created_at,
            db_message.updated_at,
        )
        .unwrap() // 从数据库加载的数据应该是有效的
    }
}

impl From<&Message> for DbMessage {
    fn from(message: &Message) -> Self {
        let message_type = match &message.message_type {
            MessageType::Text => "text",
            MessageType::Image { .. } => "image",
            MessageType::File { .. } => "file",
            MessageType::Emoji { .. } => "emoji",
        };

        DbMessage {
            id: message.id,
            room_id: message.room_id,
            sender_id: message.sender_id,
            message_type: message_type.to_string(),
            content: message.content.clone(),
            reply_to_id: message.reply_to_id,
            is_bot_message: message.is_bot_message,
            status: message.status.to_string(),
            created_at: message.created_at,
            updated_at: message.updated_at,
        }
    }
}

/// 消息Repository实现
pub struct PostgresMessageRepository {
    pool: Arc<DbPool>,
}

impl PostgresMessageRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    /// 构建搜索查询条件
    fn build_search_query(params: &MessageSearchParams) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut values = Vec::new();
        let mut param_count = 1;

        if let Some(room_id) = &params.room_id {
            conditions.push(format!("room_id = ${}", param_count));
            values.push(room_id.to_string());
            param_count += 1;
        }

        if let Some(user_id) = &params.user_id {
            conditions.push(format!("sender_id = ${}", param_count));
            values.push(user_id.to_string());
            param_count += 1;
        }

        if let Some(content) = &params.content {
            conditions.push(format!("content ILIKE ${}", param_count));
            values.push(format!("%{}%", content));
            param_count += 1;
        }

        if let Some(message_type) = &params.message_type {
            conditions.push(format!("message_type = ${}", param_count));
            values.push(message_type.to_string());
            param_count += 1;
        }

        if let Some(is_deleted) = params.is_deleted {
            if is_deleted {
                conditions.push("status = 'deleted'".to_string());
            } else {
                conditions.push("status != 'deleted'".to_string());
            }
        }

        // is_bot_message 字段在 MessageSearchParams 中不存在

        if let Some(created_after) = &params.created_after {
            conditions.push(format!("created_at > ${}", param_count));
            values.push(created_after.to_string());
            param_count += 1;
        }

        if let Some(created_before) = &params.created_before {
            conditions.push(format!("created_at < ${}", param_count));
            values.push(created_before.to_string());
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
impl MessageRepository for PostgresMessageRepository {
    async fn create(&self, message: &Message) -> DomainResult<Message> {
        let db_message = DbMessage::from(message);

        let result = query_as::<_, DbMessage>(
            r#"
            INSERT INTO messages (id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            "#,
        )
        .bind(db_message.id)
        .bind(db_message.room_id)
        .bind(db_message.sender_id)
        .bind(&db_message.message_type)
        .bind(&db_message.content)
        .bind(db_message.reply_to_id)
        .bind(db_message.is_bot_message)
        .bind(&db_message.status)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<Message>> {
        let result = query_as::<_, DbMessage>(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            WHERE id = $1 AND status != 'deleted'
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn find_by_room(
        &self,
        room_id: Uuid,
        pagination: Pagination,
        include_deleted: bool,
    ) -> DomainResult<PaginatedResult<Message>> {
        // 构建查询条件
        let where_condition = if include_deleted {
            "WHERE room_id = $1"
        } else {
            "WHERE room_id = $1 AND status != 'deleted'"
        };

        // 获取总数
        let total_count: i64 = query(&format!(
            "SELECT COUNT(*) FROM messages {}",
            where_condition
        ))
        .bind(room_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        // 获取消息
        let messages: Vec<DbMessage> = query_as(&format!(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            {} ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            where_condition
        ))
        .bind(room_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let messages: Vec<Message> = messages.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(
            messages,
            total_count as u64,
            pagination,
        ))
    }

    async fn find_by_user(
        &self,
        sender_id: Uuid,
        pagination: Pagination,
        include_deleted: bool,
    ) -> DomainResult<PaginatedResult<Message>> {
        // 构建查询条件
        let where_condition = if include_deleted {
            "WHERE sender_id = $1"
        } else {
            "WHERE sender_id = $1 AND status != 'deleted'"
        };

        // 获取总数
        let total_count: i64 = query(&format!(
            "SELECT COUNT(*) FROM messages {}",
            where_condition
        ))
        .bind(sender_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        // 获取消息
        let messages: Vec<DbMessage> = query_as(&format!(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            {} ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            where_condition
        ))
        .bind(sender_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let messages: Vec<Message> = messages.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(
            messages,
            total_count as u64,
            pagination,
        ))
    }

    async fn update(&self, message: &Message) -> DomainResult<Message> {
        let db_message = DbMessage::from(message);

        let result = query_as::<_, DbMessage>(
            r#"
            UPDATE messages
            SET content = $2, status = $3, updated_at = NOW()
            WHERE id = $1
            RETURNING id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            "#,
        )
        .bind(db_message.id)
        .bind(&db_message.content)
        .bind(&db_message.status)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn soft_delete(&self, message_id: Uuid) -> DomainResult<()> {
        query("UPDATE messages SET status = 'deleted', updated_at = NOW() WHERE id = $1")
            .bind(message_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn search(
        &self,
        params: &MessageSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<Message>> {
        let (where_clause, _values) = Self::build_search_query(params);

        let order_clause = match sort {
            Some(sort_config) => {
                let direction = if sort_config.ascending { "ASC" } else { "DESC" };
                format!("ORDER BY {} {}", sort_config.field, direction)
            }
            None => "ORDER BY created_at DESC".to_string(),
        };

        let base_query = format!(
            "FROM messages {} AND status != 'deleted'",
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
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            {} {} LIMIT {} OFFSET {}
            "#,
            base_query, order_clause, pagination.limit, pagination.offset
        );

        let messages: Vec<DbMessage> = query_as(&data_query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        let messages: Vec<Message> = messages.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(
            messages,
            total_count as u64,
            pagination,
        ))
    }

    async fn get_statistics(&self) -> DomainResult<MessageStatistics> {
        let row = query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status != 'deleted') as total_messages,
                COUNT(*) FILTER (WHERE message_type = 'text' AND status != 'deleted') as text_messages,
                COUNT(*) FILTER (WHERE message_type = 'image' AND status != 'deleted') as image_messages,
                COUNT(*) FILTER (WHERE message_type = 'file' AND status != 'deleted') as file_messages,
                COUNT(*) FILTER (WHERE is_bot_message = true AND status != 'deleted') as bot_messages,
                COUNT(*) FILTER (WHERE created_at::date = CURRENT_DATE AND status != 'deleted') as messages_today,
                COUNT(DISTINCT sender_id) FILTER (WHERE status != 'deleted') as unique_senders
            FROM messages
            "#,
        )
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(MessageStatistics {
            total_messages: row.get::<i64, _>("total_messages") as u64,
            text_messages: row.get::<i64, _>("text_messages") as u64,
            image_messages: row.get::<i64, _>("image_messages") as u64,
            file_messages: row.get::<i64, _>("file_messages") as u64,
            emoji_messages: row.get::<i64, _>("emoji_messages") as u64,
            deleted_messages: row.get::<i64, _>("deleted_messages") as u64,
            messages_today: row.get::<i64, _>("messages_today") as u64,
            avg_messages_per_room: row.get::<f64, _>("avg_messages_per_room"),
        })
    }

    async fn count_by_room(&self, room_id: Uuid, include_deleted: bool) -> DomainResult<u64> {
        let query_str = if include_deleted {
            "SELECT COUNT(*) FROM messages WHERE room_id = $1"
        } else {
            "SELECT COUNT(*) FROM messages WHERE room_id = $1 AND status != 'deleted'"
        };

        let count: i64 = query(query_str)
            .bind(room_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        Ok(count as u64)
    }

    async fn find_replies(
        &self,
        message_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>> {
        // 获取总数
        let total_count: i64 =
            query("SELECT COUNT(*) FROM messages WHERE reply_to_id = $1 AND status != 'deleted'")
                .bind(message_id)
                .fetch_one(&*self.pool)
                .await
                .map_err(|e| DomainError::database_error(e.to_string()))?
                .get(0);

        // 获取回复
        let messages: Vec<DbMessage> = query_as(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            WHERE reply_to_id = $1 AND status != 'deleted'
            ORDER BY created_at ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(message_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let messages: Vec<Message> = messages.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(
            messages,
            total_count as u64,
            pagination,
        ))
    }

    // TODO: 实现缺失的方法 - 当前为基本实现，需要后续完善

    async fn mark_as_edited(&self, message_id: Uuid, new_content: &str) -> DomainResult<()> {
        query(
            "UPDATE messages SET content = $2, status = 'edited', updated_at = NOW() WHERE id = $1",
        )
        .bind(message_id)
        .bind(new_content)
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;
        Ok(())
    }

    async fn hard_delete(&self, message_id: Uuid) -> DomainResult<()> {
        query("DELETE FROM messages WHERE id = $1")
            .bind(message_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;
        Ok(())
    }

    async fn full_text_search(
        &self,
        query_text: &str,
        room_id: Option<Uuid>,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>> {
        // 简化实现，返回空结果
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }

    async fn get_reply_chain(&self, message_id: Uuid) -> DomainResult<Vec<ReplyChain>> {
        // 简化实现，返回空结果
        Ok(Vec::new())
    }

    async fn find_latest_by_room(&self, room_id: Uuid, limit: u32) -> DomainResult<Vec<Message>> {
        let messages: Vec<DbMessage> = query_as(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            WHERE room_id = $1 AND status != 'deleted'
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(room_id)
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(messages.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_room_after(
        &self,
        room_id: Uuid,
        after: DateTime<Utc>,
        limit: u32,
    ) -> DomainResult<Vec<Message>> {
        let messages: Vec<DbMessage> = query_as(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            WHERE room_id = $1 AND created_at > $2 AND status != 'deleted'
            ORDER BY created_at ASC
            LIMIT $3
            "#,
        )
        .bind(room_id)
        .bind(after)
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(messages.into_iter().map(|m| m.into()).collect())
    }

    async fn find_by_room_before(
        &self,
        room_id: Uuid,
        before: DateTime<Utc>,
        limit: u32,
    ) -> DomainResult<Vec<Message>> {
        let messages: Vec<DbMessage> = query_as(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            WHERE room_id = $1 AND created_at < $2 AND status != 'deleted'
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(room_id)
        .bind(before)
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(messages.into_iter().map(|m| m.into()).collect())
    }

    async fn count_by_user(&self, user_id: Uuid, include_deleted: bool) -> DomainResult<u64> {
        let count_query = if include_deleted {
            "SELECT COUNT(*) FROM messages WHERE sender_id = $1"
        } else {
            "SELECT COUNT(*) FROM messages WHERE sender_id = $1 AND status != 'deleted'"
        };

        let count: i64 = query(count_query)
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        Ok(count as u64)
    }

    async fn find_by_room_with_keyword(
        &self,
        room_id: Uuid,
        keyword: &str,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>> {
        // 简化实现，返回空结果
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }

    async fn find_by_type(
        &self,
        message_type: &str,
        room_id: Option<Uuid>,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>> {
        // 简化实现，返回空结果
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }

    async fn cleanup_deleted_messages(&self, older_than: DateTime<Utc>) -> DomainResult<u64> {
        let result = query("DELETE FROM messages WHERE status = 'deleted' AND updated_at < $1")
            .bind(older_than)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn find_mentions(&self, message_id: Uuid) -> DomainResult<Vec<Uuid>> {
        // 简化实现，返回空结果
        Ok(Vec::new())
    }

    async fn update_metadata(&self, message_id: Uuid, metadata: &JsonValue) -> DomainResult<()> {
        // 简化实现，暂时不做任何操作
        Ok(())
    }

    async fn get_room_message_stats_by_day(
        &self,
        room_id: Uuid,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> DomainResult<Vec<(DateTime<Utc>, u64)>> {
        // 简化实现，返回空结果
        Ok(Vec::new())
    }

    async fn find_by_ids(&self, ids: &[Uuid]) -> DomainResult<Vec<Message>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let messages: Vec<DbMessage> = query_as(
            r#"
            SELECT id, room_id, sender_id, message_type, content, reply_to_id, is_bot_message, status, created_at, updated_at
            FROM messages
            WHERE id = ANY($1) AND status != 'deleted'
            ORDER BY created_at DESC
            "#,
        )
        .bind(ids)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(messages.into_iter().map(|m| m.into()).collect())
    }
}
