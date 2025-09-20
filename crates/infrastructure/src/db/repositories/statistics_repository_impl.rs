//! 统计Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    errors::{DomainError, DomainResult},
    repositories::StatisticsRepository,
};
use sqlx::{query, Row};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// 系统统计信息
#[derive(Debug, Clone)]
pub struct SystemStatistics {
    pub total_users: u64,
    pub active_users: u64,
    pub total_rooms: u64,
    pub active_rooms: u64,
    pub total_messages: u64,
    pub messages_today: u64,
    pub online_users: u64,
    pub peak_concurrent_users: u64,
}

/// 用户活动统计
#[derive(Debug, Clone)]
pub struct UserActivityStats {
    pub user_id: Uuid,
    pub total_messages: u64,
    pub rooms_joined: u64,
    pub last_active_at: Option<DateTime<Utc>>,
    pub online_time_minutes: u64,
}

/// 统计Repository实现
pub struct PostgresStatisticsRepository {
    pool: Arc<DbPool>,
}

impl PostgresStatisticsRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl StatisticsRepository for PostgresStatisticsRepository {
    async fn get_system_statistics(&self) -> DomainResult<HashMap<String, u64>> {
        let mut stats = HashMap::new();

        // 用户统计
        let user_stats = query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status != 'deleted') as total_users,
                COUNT(*) FILTER (WHERE status = 'active') as active_users,
                COUNT(*) FILTER (WHERE last_active_at > NOW() - INTERVAL '15 minutes') as online_users
            FROM users
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        stats.insert(
            "total_users".to_string(),
            user_stats.get::<i64, _>("total_users") as u64,
        );
        stats.insert(
            "active_users".to_string(),
            user_stats.get::<i64, _>("active_users") as u64,
        );
        stats.insert(
            "online_users".to_string(),
            user_stats.get::<i64, _>("online_users") as u64,
        );

        // 房间统计
        let room_stats = query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status != 'deleted') as total_rooms,
                COUNT(*) FILTER (WHERE status = 'active') as active_rooms
            FROM chat_rooms
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        stats.insert(
            "total_rooms".to_string(),
            room_stats.get::<i64, _>("total_rooms") as u64,
        );
        stats.insert(
            "active_rooms".to_string(),
            room_stats.get::<i64, _>("active_rooms") as u64,
        );

        // 消息统计
        let message_stats = query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status != 'deleted') as total_messages,
                COUNT(*) FILTER (WHERE created_at::date = CURRENT_DATE AND status != 'deleted') as messages_today
            FROM messages
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        stats.insert(
            "total_messages".to_string(),
            message_stats.get::<i64, _>("total_messages") as u64,
        );
        stats.insert(
            "messages_today".to_string(),
            message_stats.get::<i64, _>("messages_today") as u64,
        );

        Ok(stats)
    }

    async fn get_user_activity_stats(
        &self,
        user_id: Uuid,
        date_from: DateTime<Utc>,
        date_to: DateTime<Utc>,
    ) -> DomainResult<HashMap<String, u64>> {
        let mut stats = HashMap::new();

        // 用户消息统计
        let message_count: i64 = query(
            r#"
            SELECT COUNT(*)
            FROM messages
            WHERE sender_id = $1 AND created_at BETWEEN $2 AND $3 AND status != 'deleted'
            "#,
        )
        .bind(user_id)
        .bind(date_from)
        .bind(date_to)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        stats.insert("message_count".to_string(), message_count as u64);

        // 用户参与的房间数量
        let room_count: i64 = query(
            r#"
            SELECT COUNT(DISTINCT room_id)
            FROM room_members
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        stats.insert("rooms_joined".to_string(), room_count as u64);

        Ok(stats)
    }

    async fn get_room_activity_stats(
        &self,
        room_id: Uuid,
        date_from: DateTime<Utc>,
        date_to: DateTime<Utc>,
    ) -> DomainResult<HashMap<String, u64>> {
        let mut stats = HashMap::new();

        // 房间消息统计
        let message_count: i64 = query(
            r#"
            SELECT COUNT(*)
            FROM messages
            WHERE room_id = $1 AND created_at BETWEEN $2 AND $3 AND status != 'deleted'
            "#,
        )
        .bind(room_id)
        .bind(date_from)
        .bind(date_to)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        stats.insert("message_count".to_string(), message_count as u64);

        // 活跃用户数
        let active_users: i64 = query(
            r#"
            SELECT COUNT(DISTINCT sender_id)
            FROM messages
            WHERE room_id = $1 AND created_at BETWEEN $2 AND $3 AND status != 'deleted'
            "#,
        )
        .bind(room_id)
        .bind(date_from)
        .bind(date_to)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        stats.insert("active_users".to_string(), active_users as u64);

        // 当前成员数
        let member_count: i64 = query("SELECT COUNT(*) FROM room_members WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        stats.insert("member_count".to_string(), member_count as u64);

        Ok(stats)
    }

    async fn record_user_online_time(
        &self,
        user_id: Uuid,
        duration_minutes: u64,
    ) -> DomainResult<()> {
        // 这里可以记录到专门的在线时长表，或者更新用户表的统计字段
        // 简化实现：暂时只更新最后活跃时间
        query("UPDATE users SET last_active_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn get_popular_rooms(&self, limit: u32) -> DomainResult<Vec<(Uuid, String, u64)>> {
        let rooms = query(
            r#"
            SELECT cr.id, cr.name, COUNT(rm.user_id) as member_count
            FROM chat_rooms cr
            LEFT JOIN room_members rm ON cr.id = rm.room_id
            WHERE cr.status = 'active' AND cr.is_private = false
            GROUP BY cr.id, cr.name
            ORDER BY member_count DESC, cr.last_activity_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let result = rooms
            .into_iter()
            .map(|row| {
                let id: Uuid = row.get("id");
                let name: String = row.get("name");
                let member_count: i64 = row.get("member_count");
                (id, name, member_count as u64)
            })
            .collect();

        Ok(result)
    }

    async fn get_active_users(&self, limit: u32) -> DomainResult<Vec<(Uuid, String, u64)>> {
        let users = query(
            r#"
            SELECT u.id, u.username, COUNT(m.id) as message_count
            FROM users u
            LEFT JOIN messages m ON u.id = m.sender_id
                AND m.created_at > NOW() - INTERVAL '30 days'
                AND m.status != 'deleted'
            WHERE u.status = 'active'
            GROUP BY u.id, u.username
            ORDER BY message_count DESC, u.last_active_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let result = users
            .into_iter()
            .map(|row| {
                let id: Uuid = row.get("id");
                let username: String = row.get("username");
                let message_count: i64 = row.get("message_count");
                (id, username, message_count as u64)
            })
            .collect();

        Ok(result)
    }

    async fn cleanup_old_statistics(&self, days_old: u32) -> DomainResult<u64> {
        // 这里可以清理老旧的统计记录
        // 简化实现：假设我们有一个统计记录表
        let result = query(
            r#"
            DELETE FROM statistics_records
            WHERE created_at < NOW() - INTERVAL '$1 days'
            "#,
        )
        .bind(days_old as i32)
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected())
    }
}
