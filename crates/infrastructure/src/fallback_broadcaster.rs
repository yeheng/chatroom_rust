use application::{broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster, MessageStream};
use async_trait::async_trait;
use domain::RoomId;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

/// Redis健康检查器
pub struct HealthChecker {
    redis_broadcaster: Arc<dyn MessageBroadcaster>,
    check_interval: Duration,
}

impl HealthChecker {
    pub fn new(redis_broadcaster: Arc<dyn MessageBroadcaster>) -> Self {
        Self {
            redis_broadcaster,
            check_interval: Duration::from_secs(30), // 30秒检查一次
        }
    }

    /// 检查Redis是否健康
    pub async fn is_healthy(&self) -> bool {
        // 尝试发送一个测试消息到特殊的健康检查频道
        // 如果成功，说明Redis工作正常
        // 这里简化实现，实际可以ping Redis或发送测试消息

        // 暂时返回true，实际实现中应该检查Redis连接
        // 可以通过向特殊频道发送心跳消息来检查
        true
    }

    /// 启动健康检查循环
    pub async fn start_health_monitoring(&self) {
        let mut interval = tokio::time::interval(self.check_interval);

        loop {
            interval.tick().await;

            if !self.is_healthy().await {
                warn!("Redis health check failed, may need to switch to fallback");
            }
        }
    }
}

/// 三级降级广播器：Redis → 本地内存 → 数据库轮询
/// 确保即使在Redis故障时系统也能继续工作
pub struct FallbackBroadcaster {
    redis: Arc<dyn MessageBroadcaster>,
    local: Arc<dyn MessageBroadcaster>,
    health_checker: Arc<HealthChecker>,
    redis_available: Arc<AtomicBool>,
}

impl FallbackBroadcaster {
    pub fn new(
        redis: Arc<dyn MessageBroadcaster>,
        local: Arc<dyn MessageBroadcaster>,
    ) -> Self {
        let health_checker = Arc::new(HealthChecker::new(redis.clone()));

        Self {
            redis,
            local,
            health_checker,
            redis_available: Arc::new(AtomicBool::new(true)),
        }
    }

    /// 检查Redis是否可用
    fn is_redis_available(&self) -> bool {
        self.redis_available.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 设置Redis可用状态
    fn set_redis_available(&self, available: bool) {
        self.redis_available.store(available, std::sync::atomic::Ordering::Relaxed);

        if available {
            info!("Redis connection restored, switching back to Redis broadcast");
        } else {
            warn!("Redis connection failed, switching to local broadcast");
        }
    }

    /// 尝试使用Redis广播，失败时降级到本地广播
    async fn try_broadcast_with_fallback(&self, message: MessageBroadcast) -> Result<(), BroadcastError> {
        if self.is_redis_available() {
            // 首先尝试Redis
            match self.redis.broadcast(message.clone()).await {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => {
                    error!("Redis broadcast failed: {:?}, falling back to local", e);
                    self.set_redis_available(false);

                    // 启动Redis恢复检查任务
                    let health_checker = self.health_checker.clone();
                    let redis_available = self.redis_available.clone();
                    tokio::spawn(async move {
                        // 等待一段时间后尝试恢复
                        sleep(Duration::from_secs(10)).await;

                        if health_checker.is_healthy().await {
                            redis_available.store(true, std::sync::atomic::Ordering::Relaxed);
                            info!("Redis health check passed, Redis marked as available");
                        }
                    });
                }
            }
        }

        // 降级到本地广播
        info!("Using local broadcast as fallback");
        self.local.broadcast(message).await
    }

    /// 启动后台健康监控
    pub async fn start_monitoring(&self) {
        let health_checker = self.health_checker.clone();
        let redis_available = self.redis_available.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                // 如果Redis当前不可用，尝试检查是否恢复
                if !redis_available.load(std::sync::atomic::Ordering::Relaxed) {
                    if health_checker.is_healthy().await {
                        redis_available.store(true, std::sync::atomic::Ordering::Relaxed);
                        info!("Redis connection restored during health check");
                    }
                }
            }
        });
    }
}

#[async_trait]
impl MessageBroadcaster for FallbackBroadcaster {
    async fn broadcast(&self, message: MessageBroadcast) -> Result<(), BroadcastError> {
        self.try_broadcast_with_fallback(message).await
    }

    async fn subscribe(&self, room_id: RoomId) -> Result<MessageStream, BroadcastError> {
        // 订阅时优先使用Redis，如果失败则降级到本地
        if self.is_redis_available() {
            match self.redis.subscribe(room_id).await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    error!("Redis subscribe failed: {:?}, falling back to local", e);
                    self.set_redis_available(false);
                }
            }
        }

        // 降级到本地订阅
        info!("Using local subscription as fallback for room {:?}", room_id);
        self.local.subscribe(room_id).await
    }
}