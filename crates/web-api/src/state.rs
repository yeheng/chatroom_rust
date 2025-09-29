use std::sync::Arc;
use std::time::Duration;

use application::{ChatService, MessageBroadcaster, PresenceManager, UserService, EventStorage};
use infrastructure::StatsAggregationService;

use crate::JwtService;

/// 事件收集器队列状态
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub queue_size: usize,
    pub max_queue_size: usize,
    pub is_running: bool,
    pub batch_size: usize,
    pub flush_interval: Duration,
}

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub chat_service: Arc<ChatService>,
    pub broadcaster: Arc<dyn MessageBroadcaster>,
    pub jwt_service: Arc<JwtService>,
    pub presence_manager: Arc<dyn PresenceManager>,
    pub stats_service: Arc<StatsAggregationService>,
    pub event_storage: Arc<dyn EventStorage>,
    // 注意：事件收集器已迁移到独立服务，这里保留兼容性接口
}

impl AppState {
    pub fn new(
        user_service: Arc<UserService>,
        chat_service: Arc<ChatService>,
        broadcaster: Arc<dyn MessageBroadcaster>,
        jwt_service: Arc<JwtService>,
        presence_manager: Arc<dyn PresenceManager>,
        stats_service: Arc<StatsAggregationService>,
        event_storage: Arc<dyn EventStorage>,
    ) -> Self {
        Self {
            user_service,
            chat_service,
            broadcaster,
            jwt_service,
            presence_manager,
            stats_service,
            event_storage,
        }
    }

    /// 获取事件收集器状态（兼容性接口）
    ///
    /// 现在事件处理由独立的 stats-consumer 服务完成，
    /// 这里返回虚拟状态用于API兼容性
    pub fn get_event_collector_status(&self) -> QueueStatus {
        use std::time::Duration;

        QueueStatus {
            queue_size: 0,  // 独立服务处理，主应用不知道队列状态
            max_queue_size: 10000,
            is_running: true,  // 假设独立服务正在运行
            batch_size: 1000,
            flush_interval: Duration::from_secs(5),
        }
    }
}
