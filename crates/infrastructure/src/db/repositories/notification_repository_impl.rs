//! 通知Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    Notification,
    errors::{DomainError, DomainResult},
    repositories::{NotificationRepository, NotificationSearchParams, NotificationStatistics, Pagination, PaginatedResult, SortConfig},
};
use sqlx::{query, query_as, FromRow};
use std::sync::Arc;
use uuid::Uuid;

/// 数据库通知模型
#[derive(Debug, Clone, FromRow)]
struct DbNotification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub content: String,
    pub priority: String,
    pub is_read: bool,
    pub is_dismissed: bool,
    pub metadata: sqlx::types::Json<serde_json::Value>,
    pub expires_at: Option<DateTime<Utc>>,
    pub template_id: Option<Uuid>,
    pub source_id: Option<Uuid>,
    pub group_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub dismissed_at: Option<DateTime<Utc>>,
}

impl From<DbNotification> for Notification {
    fn from(db_notification: DbNotification) -> Self {
        Notification {
            id: db_notification.id,
            user_id: db_notification.user_id,
            notification_type: db_notification.notification_type,
            title: db_notification.title,
            content: db_notification.content,
            priority: db_notification.priority,
            is_read: db_notification.is_read,
            is_dismissed: db_notification.is_dismissed,
            metadata: db_notification.metadata.0,
            expires_at: db_notification.expires_at,
            template_id: db_notification.template_id,
            source_id: db_notification.source_id,
            group_key: db_notification.group_key,
            created_at: db_notification.created_at,
            read_at: db_notification.read_at,
            dismissed_at: db_notification.dismissed_at,
        }
    }
}

impl From<&Notification> for DbNotification {
    fn from(notification: &Notification) -> Self {
        DbNotification {
            id: notification.id,
            user_id: notification.user_id,
            notification_type: notification.notification_type.clone(),
            title: notification.title.clone(),
            content: notification.content.clone(),
            priority: notification.priority.clone(),
            is_read: notification.is_read,
            is_dismissed: notification.is_dismissed,
            metadata: sqlx::types::Json(notification.metadata.clone()),
            expires_at: notification.expires_at,
            template_id: notification.template_id,
            source_id: notification.source_id,
            group_key: notification.group_key.clone(),
            created_at: notification.created_at,
            read_at: notification.read_at,
            dismissed_at: notification.dismissed_at,
        }
    }
}

/// 通知Repository实现
pub struct PostgresNotificationRepository {
    pool: Arc<DbPool>,
}

impl PostgresNotificationRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationRepository for PostgresNotificationRepository {
    async fn create(&self, notification: &Notification) -> DomainResult<Notification> {
        let db_notification = DbNotification::from(notification);

        let result = query_as::<_, DbNotification>(
            r#"
            INSERT INTO notifications (
                id, user_id, notification_type, title, content, priority,
                is_read, is_dismissed, metadata, expires_at, template_id,
                source_id, group_key
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id, user_id, notification_type, title, content, priority,
                     is_read, is_dismissed, metadata, expires_at, template_id,
                     source_id, group_key, created_at, read_at, dismissed_at
            "#,
        )
        .bind(db_notification.id)
        .bind(db_notification.user_id)
        .bind(&db_notification.notification_type)
        .bind(&db_notification.title)
        .bind(&db_notification.content)
        .bind(&db_notification.priority)
        .bind(db_notification.is_read)
        .bind(db_notification.is_dismissed)
        .bind(&db_notification.metadata)
        .bind(db_notification.expires_at)
        .bind(db_notification.template_id)
        .bind(db_notification.source_id)
        .bind(&db_notification.group_key)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.into())
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<Notification>> {
        let result = query_as::<_, DbNotification>(
            r#"
            SELECT id, user_id, notification_type, title, content, priority,
                   is_read, is_dismissed, metadata, expires_at, template_id,
                   source_id, group_key, created_at, read_at, dismissed_at
            FROM notifications
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn find_by_user(&self, user_id: Uuid, pagination: Pagination, unread_only: bool) -> DomainResult<PaginatedResult<Notification>> {
        let where_clause = if unread_only {
            "WHERE user_id = $1 AND is_read = false AND (expires_at IS NULL OR expires_at > NOW())"
        } else {
            "WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())"
        };

        // 获取总数
        let count_query = format!("SELECT COUNT(*) FROM notifications {}", where_clause);
        let total_count: i64 = query(&count_query)
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        // 获取通知
        let data_query = format!(
            r#"
            SELECT id, user_id, notification_type, title, content, priority,
                   is_read, is_dismissed, metadata, expires_at, template_id,
                   source_id, group_key, created_at, read_at, dismissed_at
            FROM notifications
            {} ORDER BY created_at DESC
            LIMIT {} OFFSET {}
            "#,
            where_clause, pagination.limit, pagination.offset
        );

        let notifications: Vec<DbNotification> = query_as(&data_query)
            .bind(user_id)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let notifications: Vec<Notification> = notifications.into_iter().map(|n| n.into()).collect();
        Ok(PaginatedResult::new(notifications, total_count as u64, pagination))
    }

    async fn mark_as_read(&self, notification_id: Uuid) -> DomainResult<()> {
        query("UPDATE notifications SET is_read = true, read_at = NOW() WHERE id = $1")
            .bind(notification_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn mark_multiple_as_read(&self, notification_ids: &[Uuid]) -> DomainResult<u64> {
        if notification_ids.is_empty() {
            return Ok(0);
        }

        // 为了简化，逐个标记为已读
        let mut count = 0;
        for &id in notification_ids {
            match self.mark_as_read(id).await {
                Ok(_) => count += 1,
                Err(_) => continue,
            }
        }

        Ok(count)
    }

    async fn mark_all_as_read(&self, user_id: Uuid) -> DomainResult<u64> {
        let result = query("UPDATE notifications SET is_read = true, read_at = NOW() WHERE user_id = $1 AND is_read = false")
            .bind(user_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn mark_type_as_read(&self, user_id: Uuid, notification_type: &str) -> DomainResult<u64> {
        let result = query("UPDATE notifications SET is_read = true, read_at = NOW() WHERE user_id = $1 AND notification_type = $2 AND is_read = false")
            .bind(user_id)
            .bind(notification_type)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn dismiss(&self, notification_id: Uuid) -> DomainResult<()> {
        query("UPDATE notifications SET is_dismissed = true, dismissed_at = NOW() WHERE id = $1")
            .bind(notification_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, notification_id: Uuid) -> DomainResult<bool> {
        let result = query("DELETE FROM notifications WHERE id = $1")
            .bind(notification_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn count_unread(&self, user_id: Uuid) -> DomainResult<u64> {
        let count: i64 = query("SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND is_read = false AND (expires_at IS NULL OR expires_at > NOW())")
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        Ok(count as u64)
    }

    async fn count_unread_by_type(&self, user_id: Uuid, notification_type: &str) -> DomainResult<u64> {
        let count: i64 = query("SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND notification_type = $2 AND is_read = false AND (expires_at IS NULL OR expires_at > NOW())")
            .bind(user_id)
            .bind(notification_type)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        Ok(count as u64)
    }

    async fn search(
        &self,
        params: &NotificationSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<Notification>> {
        // 简化实现，返回空结果
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }

    async fn get_statistics(&self, user_id: Uuid) -> DomainResult<NotificationStatistics> {
        let row = query(
            r#"
            SELECT
                COUNT(*) as total_notifications,
                COUNT(*) FILTER (WHERE is_read = false) as unread_notifications,
                COUNT(*) FILTER (WHERE is_dismissed = true) as dismissed_notifications,
                COUNT(*) FILTER (WHERE priority = 'high') as high_priority_notifications,
                COUNT(*) FILTER (WHERE created_at::date = CURRENT_DATE) as notifications_today
            FROM notifications
            WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > NOW())
            "#,
        )
        .bind(user_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(NotificationStatistics {
            total_notifications: row.get::<i64, _>("total_notifications") as u64,
            unread_notifications: row.get::<i64, _>("unread_notifications") as u64,
            dismissed_notifications: row.get::<i64, _>("dismissed_notifications") as u64,
            high_priority_notifications: row.get::<i64, _>("high_priority_notifications") as u64,
            notifications_today: row.get::<i64, _>("notifications_today") as u64,
        })
    }

    async fn cleanup_expired(&self) -> DomainResult<u64> {
        let result = query("DELETE FROM notifications WHERE expires_at IS NOT NULL AND expires_at < NOW()")
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn create_from_template(
        &self,
        user_id: Uuid,
        template_id: &str,
        template_data: &serde_json::Value,
        source_id: Option<Uuid>,
        group_key: Option<Uuid>,
        expires_at: Option<Uuid>,
    ) -> DomainResult<Option<Notification>> {
        // 简化实现，暂时返回 None
        Ok(None)
    }

    async fn find_high_priority_unread(&self, user_id: Uuid, limit: u32) -> DomainResult<Vec<Notification>> {
        let notifications: Vec<DbNotification> = query_as(
            r#"
            SELECT id, user_id, notification_type, title, content, priority,
                   is_read, is_dismissed, metadata, expires_at, template_id,
                   source_id, group_key, created_at, read_at, dismissed_at
            FROM notifications
            WHERE user_id = $1 AND is_read = false AND priority = 'high'
                  AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(notifications.into_iter().map(|n| n.into()).collect())
    }

    async fn create_batch(&self, notifications: &[Notification]) -> DomainResult<Vec<Notification>> {
        let mut created_notifications = Vec::new();

        for notification in notifications {
            match self.create(notification).await {
                Ok(created) => created_notifications.push(created),
                Err(_) => continue,
            }
        }

        Ok(created_notifications)
    }
}