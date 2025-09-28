use async_trait::async_trait;
use chrono::Utc;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::ApplicationError;
use crate::presence::{PresenceEventType, UserPresenceEvent};
use domain::{RoomId, UserId};

/// 异步事件采集器配置
#[derive(Debug, Clone)]
pub struct EventCollectorConfig {
    /// 批量写入大小
    pub batch_size: usize,
    /// 刷新间隔
    pub flush_interval: Duration,
    /// 最大队列长度
    pub max_queue_size: usize,
}

impl Default for EventCollectorConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            flush_interval: Duration::from_secs(5),
            max_queue_size: 10000,
        }
    }
}

/// 事件存储trait，抽象具体的数据库实现
#[async_trait]
pub trait EventStorage: Send + Sync {
    /// 批量插入事件
    async fn insert_events(&self, events: &[UserPresenceEvent]) -> Result<(), ApplicationError>;

    /// 获取事件总数（用于监控）
    async fn get_event_count(&self) -> Result<i64, ApplicationError>;

    /// 获取指定时间范围内的事件数量
    async fn get_event_count_in_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
    ) -> Result<i64, ApplicationError>;
}

/// 异步用户状态事件采集器
///
/// 设计原则：
/// 1. 异步写入，不阻塞实时功能
/// 2. 批量写入，提高数据库性能
/// 3. 内存队列缓冲，防止瞬时高峰
/// 4. 故障容错，数据库故障时缓存事件
pub struct PresenceEventCollector {
    /// 事件缓冲队列
    event_queue: Arc<RwLock<VecDeque<UserPresenceEvent>>>,
    /// 事件存储
    storage: Arc<dyn EventStorage>,
    /// 采集器配置
    config: EventCollectorConfig,
    /// 运行状态
    is_running: Arc<RwLock<bool>>,
}

impl PresenceEventCollector {
    /// 创建新的事件采集器
    pub fn new(storage: Arc<dyn EventStorage>, config: EventCollectorConfig) -> Self {
        Self {
            event_queue: Arc::new(RwLock::new(VecDeque::new())),
            storage,
            config,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动事件采集器
    pub async fn start(&self) -> Result<(), ApplicationError> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Ok(()); // 已经在运行
            }
            *running = true;
        }

        let queue = Arc::clone(&self.event_queue);
        let storage = Arc::clone(&self.storage);
        let config = self.config.clone();
        let running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.flush_interval);

            while *running.read().await {
                interval.tick().await;

                if let Err(err) =
                    Self::flush_events_batch(&queue, &storage, config.batch_size).await
                {
                    tracing::error!(error = ?err, "Failed to flush presence events");
                }
            }

            tracing::info!("Presence event collector stopped");
        });

        tracing::info!("Presence event collector started");
        Ok(())
    }

    /// 停止事件采集器
    pub async fn stop(&self) -> Result<(), ApplicationError> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // 最后一次刷新所有剩余事件
        Self::flush_events_batch(&self.event_queue, &self.storage, usize::MAX).await?;

        tracing::info!("Presence event collector stopped and flushed remaining events");
        Ok(())
    }

    /// 记录用户状态变化事件
    pub async fn record_event(&self, event: UserPresenceEvent) -> Result<(), ApplicationError> {
        let mut queue = self.event_queue.write().await;

        // 检查队列是否已满
        if queue.len() >= self.config.max_queue_size {
            // 队列满时，移除最旧的事件（FIFO）
            queue.pop_front();
            tracing::warn!(
                max_size = self.config.max_queue_size,
                "Event queue full, dropped oldest event"
            );
        }

        queue.push_back(event);

        tracing::debug!(
            queue_size = queue.len(),
            "Event queued for async processing"
        );

        Ok(())
    }

    /// 批量刷新事件到存储
    async fn flush_events_batch(
        queue: &Arc<RwLock<VecDeque<UserPresenceEvent>>>,
        storage: &Arc<dyn EventStorage>,
        max_batch_size: usize,
    ) -> Result<(), ApplicationError> {
        let events_to_flush = {
            let mut queue_guard = queue.write().await;
            if queue_guard.is_empty() {
                return Ok(());
            }

            let batch_size = std::cmp::min(queue_guard.len(), max_batch_size);
            let events: Vec<_> = queue_guard.drain(..batch_size).collect();
            events
        };

        if events_to_flush.is_empty() {
            return Ok(());
        }

        let batch_size = events_to_flush.len();
        let start_time = std::time::Instant::now();

        // 批量插入到存储
        match storage.insert_events(&events_to_flush).await {
            Ok(()) => {
                let duration = start_time.elapsed();
                tracing::info!(
                    batch_size = batch_size,
                    duration_ms = duration.as_millis(),
                    "Successfully flushed presence events"
                );
            }
            Err(err) => {
                // 写入失败时，将事件放回队列前端
                {
                    let mut queue_guard = queue.write().await;
                    for event in events_to_flush.into_iter().rev() {
                        queue_guard.push_front(event);
                    }
                }

                tracing::error!(
                    error = ?err,
                    batch_size = batch_size,
                    "Failed to flush events, requeued for retry"
                );

                return Err(err);
            }
        }

        Ok(())
    }

    /// 获取队列状态信息
    pub async fn get_queue_status(&self) -> QueueStatus {
        let queue = self.event_queue.read().await;
        let is_running = *self.is_running.read().await;

        QueueStatus {
            queue_size: queue.len(),
            max_queue_size: self.config.max_queue_size,
            is_running,
            batch_size: self.config.batch_size,
            flush_interval: self.config.flush_interval,
        }
    }
}

/// 队列状态信息
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub queue_size: usize,
    pub max_queue_size: usize,
    pub is_running: bool,
    pub batch_size: usize,
    pub flush_interval: Duration,
}

/// 便捷函数：创建用户连接事件
pub fn create_connection_event(
    user_id: UserId,
    room_id: RoomId,
    session_id: Uuid,
    user_ip: Option<String>,
    user_agent: Option<String>,
) -> UserPresenceEvent {
    UserPresenceEvent {
        event_id: Uuid::new_v4(),
        user_id,
        room_id,
        event_type: PresenceEventType::Connected,
        timestamp: Utc::now(),
        session_id,
        user_ip,
        user_agent,
    }
}

/// 便捷函数：创建用户断开事件
pub fn create_disconnection_event(
    user_id: UserId,
    room_id: RoomId,
    session_id: Uuid,
) -> UserPresenceEvent {
    UserPresenceEvent {
        event_id: Uuid::new_v4(),
        user_id,
        room_id,
        event_type: PresenceEventType::Disconnected,
        timestamp: Utc::now(),
        session_id,
        user_ip: None,
        user_agent: None,
    }
}

/// 便捷函数：创建心跳事件
pub fn create_heartbeat_event(
    user_id: UserId,
    room_id: RoomId,
    session_id: Uuid,
) -> UserPresenceEvent {
    UserPresenceEvent {
        event_id: Uuid::new_v4(),
        user_id,
        room_id,
        event_type: PresenceEventType::Heartbeat,
        timestamp: Utc::now(),
        session_id,
        user_ip: None,
        user_agent: None,
    }
}
