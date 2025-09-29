use application::{
    broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster, MessageStream,
};
use async_trait::async_trait;
use domain::RoomId;
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, Instant};
use tracing::{error, info};

/// 降级广播器：Redis主，本地备
///
/// Linus式设计原则：
/// 1. "好品味" - 消除所有特殊情况，只有一个逻辑：尝试主要，失败用备用
/// 2. "简洁执念" - 不要什么状态机，就一个冷却时间
/// 3. "实用主义" - 解决真实问题：Redis挂了怎么办
pub struct FallbackBroadcaster {
    redis: Arc<dyn MessageBroadcaster>,
    local: Arc<dyn MessageBroadcaster>,
    last_failure: Arc<Mutex<Option<Instant>>>, // 最后一次Redis失败的时间
    cooldown: Duration,                        // Redis失败后的冷却时间
}

impl FallbackBroadcaster {
    pub fn new(redis: Arc<dyn MessageBroadcaster>, local: Arc<dyn MessageBroadcaster>) -> Self {
        Self {
            redis,
            local,
            last_failure: Arc::new(Mutex::new(None)),
            cooldown: Duration::from_secs(5), // 5秒冷却时间，简单实用
        }
    }

    /// 检查是否在冷却期内 - 唯一的"状态"检查
    fn is_in_cooldown(&self) -> bool {
        if let Ok(guard) = self.last_failure.lock() {
            if let Some(failure_time) = *guard {
                return failure_time.elapsed() < self.cooldown;
            }
        }
        false
    }

    /// 记录Redis失败 - 更新冷却时间
    fn record_redis_failure(&self) {
        if let Ok(mut guard) = self.last_failure.lock() {
            *guard = Some(Instant::now());
        }
        error!(
            "Redis失败，启动{}秒冷却期，使用本地广播",
            self.cooldown.as_secs()
        );
    }

    /// 清除失败记录 - Redis恢复正常
    fn clear_failure(&self) {
        if let Ok(mut guard) = self.last_failure.lock() {
            *guard = None;
        }
    }
}

#[async_trait]
impl MessageBroadcaster for FallbackBroadcaster {
    /// 降级广播：尝试Redis，失败用本地
    ///
    /// Linus式逻辑 - 消除所有特殊情况：
    /// 1. 如果在冷却期 → 直接用本地
    /// 2. 否则尝试Redis，成功就清除失败记录
    /// 3. Redis失败 → 记录失败时间，用本地降级
    async fn broadcast(&self, message: MessageBroadcast) -> Result<(), BroadcastError> {
        // 冷却期内直接跳过Redis
        if self.is_in_cooldown() {
            info!("Redis冷却期内，使用本地广播");
            return self.local.broadcast(message).await;
        }

        // 尝试Redis
        match self.redis.broadcast(message.clone()).await {
            Ok(_) => {
                // Redis成功，清除失败记录
                self.clear_failure();
                Ok(())
            }
            Err(_) => {
                // Redis失败，记录并降级到本地
                self.record_redis_failure();
                self.local.broadcast(message).await
            }
        }
    }

    /// 降级订阅：尝试Redis，失败用本地
    ///
    /// 订阅逻辑更简单：选定一个就用到底，不能中途切换
    async fn subscribe(&self, room_id: RoomId) -> Result<MessageStream, BroadcastError> {
        // 冷却期内直接用本地
        if self.is_in_cooldown() {
            info!("Redis冷却期内，使用本地订阅房间 {:?}", room_id);
            return self.local.subscribe(room_id).await;
        }

        // 尝试Redis订阅
        match self.redis.subscribe(room_id).await {
            Ok(stream) => {
                info!("Redis订阅成功，房间 {:?}", room_id);
                self.clear_failure();
                Ok(stream)
            }
            Err(e) => {
                // Redis订阅失败，记录并用本地
                error!("Redis订阅失败，房间 {:?}: {:?}，降级到本地", room_id, e);
                self.record_redis_failure();
                self.local.subscribe(room_id).await
            }
        }
    }
}
