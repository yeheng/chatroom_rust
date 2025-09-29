use application::{ApplicationError, UserPresenceEvent};
use async_trait::async_trait;
use sqlx::{types::chrono, PgPool, Row};

use crate::event_storage::EventStorage;

/// PostgreSQL事件存储实现
/// 实现EventStorage trait，负责将用户状态事件批量插入数据库
#[derive(Clone)]
pub struct PgEventStorage {
    pool: PgPool,
}

impl PgEventStorage {
    /// 创建新的PostgreSQL事件存储实例
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 检查数据库连接是否正常
    pub async fn health_check(&self) -> Result<(), ApplicationError> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|err| {
                ApplicationError::infrastructure(format!("Database health check failed: {}", err))
            })?;
        Ok(())
    }
}

fn map_sqlx_err(err: sqlx::Error) -> ApplicationError {
    ApplicationError::infrastructure_with_source("Database operation failed".to_string(), err)
}

#[async_trait]
impl EventStorage for PgEventStorage {
    /// 批量插入用户状态事件到数据库
    ///
    /// 使用事务确保数据一致性，批量插入提高性能
    async fn insert_events(&self, events: &[UserPresenceEvent]) -> Result<(), ApplicationError> {
        if events.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx_err)?;

        // 构建批量插入SQL
        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO presence_events (event_id, user_id, room_id, event_type, timestamp, session_id, user_ip, user_agent) "
        );

        query_builder.push_values(events, |mut b, event| {
            b.push_bind(event.event_id)
                .push_bind(uuid::Uuid::from(event.user_id))
                .push_bind(uuid::Uuid::from(event.room_id))
                .push_bind(event.event_type.to_string()) // 转换为字符串
                .push_bind(event.timestamp)
                .push_bind(event.session_id)
                .push_bind(&event.user_ip)
                .push_bind(&event.user_agent);
        });

        let query = query_builder.build();

        query.execute(&mut *tx).await.map_err(map_sqlx_err)?;

        tx.commit().await.map_err(map_sqlx_err)?;

        tracing::info!(
            batch_size = events.len(),
            "Successfully inserted presence events batch"
        );

        Ok(())
    }

    /// 获取事件总数（用于监控）
    async fn get_event_count(&self) -> Result<i64, ApplicationError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM presence_events")
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        Ok(row.get::<i64, _>("count"))
    }

    /// 获取指定时间范围内的事件数量
    async fn get_event_count_in_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<i64, ApplicationError> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM presence_events WHERE timestamp >= $1 AND timestamp < $2"
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(row.get::<i64, _>("count"))
    }
}

/// 为基础设施构建器添加事件存储支持
pub fn create_event_storage(pool: PgPool) -> PgEventStorage {
    PgEventStorage::new(pool)
}
