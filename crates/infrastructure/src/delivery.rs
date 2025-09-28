use application::delivery::DeliveryTracker;
use async_trait::async_trait;
use domain::{MessageId, RepositoryError, UserId};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::repository::map_sqlx_err;

#[derive(Debug, FromRow)]
struct DeliveryRecord {
    message_id: Uuid,
    _user_id: Uuid,
    _sent_at: OffsetDateTime,
    _delivered_at: Option<OffsetDateTime>,
}

/// PostgreSQL实现的消息传递追踪器
/// 提供可靠的消息确认机制
#[derive(Clone)]
pub struct PgDeliveryTracker {
    pool: PgPool,
}

impl PgDeliveryTracker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DeliveryTracker for PgDeliveryTracker {
    async fn mark_sent(
        &self,
        message_id: MessageId,
        user_id: UserId,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO message_deliveries (message_id, user_id, sent_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (message_id, user_id) DO NOTHING
            "#,
        )
        .bind(Uuid::from(message_id))
        .bind(Uuid::from(user_id))
        .bind(OffsetDateTime::now_utc())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(())
    }

    async fn mark_delivered(
        &self,
        message_id: MessageId,
        user_id: UserId,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            r#"
            UPDATE message_deliveries
            SET delivered_at = $3
            WHERE message_id = $1 AND user_id = $2 AND delivered_at IS NULL
            "#,
        )
        .bind(Uuid::from(message_id))
        .bind(Uuid::from(user_id))
        .bind(OffsetDateTime::now_utc())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        // 检查是否有行被更新
        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn get_undelivered(&self, user_id: UserId) -> Result<Vec<MessageId>, RepositoryError> {
        let records = sqlx::query_as::<_, DeliveryRecord>(
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

        Ok(records
            .into_iter()
            .map(|r| MessageId::from(r.message_id))
            .collect())
    }

    async fn cleanup_delivered(&self, older_than_hours: u32) -> Result<u64, RepositoryError> {
        let cutoff_time =
            OffsetDateTime::now_utc() - time::Duration::hours(older_than_hours as i64);

        let result = sqlx::query(
            r#"
            DELETE FROM message_deliveries
            WHERE delivered_at IS NOT NULL AND delivered_at < $1
            "#,
        )
        .bind(cutoff_time)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(result.rows_affected())
    }
}
