use std::time::Duration;

use async_trait::async_trait;
use sqlx::types::chrono::{DateTime, Utc};
use application::{UserPresenceEvent, ApplicationError};

/// 事件存储trait - 用于将用户状态事件持久化到数据库
#[async_trait]
pub trait EventStorage: Send + Sync {
    /// 批量插入用户状态事件到数据库
    async fn insert_events(&self, events: &[UserPresenceEvent]) -> Result<(), ApplicationError>;

    /// 获取事件总数（用于监控）
    async fn get_event_count(&self) -> Result<i64, ApplicationError>;

    /// 获取指定时间范围内的事件数量
    async fn get_event_count_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, ApplicationError>;
}

/// 事件收集器队列状态
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub queue_size: usize,
    pub max_queue_size: usize,
    pub is_running: bool,
    pub batch_size: usize,
    pub flush_interval: Duration,
}