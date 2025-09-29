use application::{ApplicationError, UserPresenceEvent};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// 事件存储trait - 用于将用户状态事件持久化到数据库
#[async_trait]
pub trait EventStorage: Send + Sync {
    /// 批量插入用户状态事件到数据库
    async fn insert_events(&self, events: &[UserPresenceEvent]) -> Result<(), ApplicationError>;

    /// 获取事件总数（用于监控）
    #[allow(dead_code)]
    async fn get_event_count(&self) -> Result<i64, ApplicationError>;

    /// 获取指定时间范围内的事件数量
    #[allow(dead_code)]
    async fn get_event_count_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, ApplicationError>;
}
