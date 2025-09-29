use application::{
    broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster, MessageStream,
};
use async_trait::async_trait;
use domain::RoomId;
use std::sync::{
    atomic::{AtomicU64, AtomicU8, Ordering},
    Arc, Mutex,
};
use tokio::time::{Duration, Instant};
use tracing::{error, info, warn};

/// 断路器状态
#[derive(Debug, Clone, Copy, PartialEq)]
enum CircuitState {
    Closed = 0,   // 正常状态，允许请求通过
    Open = 1,     // 断开状态，直接失败
    HalfOpen = 2, // 半开状态，允许少量请求测试服务是否恢复
}

impl From<u8> for CircuitState {
    fn from(value: u8) -> Self {
        match value {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Open, // 默认为断开状态
        }
    }
}

/// 断路器配置
#[derive(Debug, Clone)]
struct CircuitConfig {
    failure_threshold: u64,   // 失败阈值
    success_threshold: u64,   // 成功阈值（半开状态下）
    timeout: Duration,        // 超时时间
    half_open_max_calls: u64, // 半开状态下最大尝试次数
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            half_open_max_calls: 3,
        }
    }
}

/// 断路器统计信息
#[derive(Debug)]
struct CircuitMetrics {
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: Mutex<Option<Instant>>,
    half_open_calls: AtomicU64,
}

impl CircuitMetrics {
    fn new() -> Self {
        Self {
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: Mutex::new(None),
            half_open_calls: AtomicU64::new(0),
        }
    }

    fn reset(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        self.success_count.store(0, Ordering::Relaxed);
        self.half_open_calls.store(0, Ordering::Relaxed);
        *self.last_failure_time.lock().unwrap() = None;
    }
}

/// 断路器广播器：使用断路器模式保护Redis连接
/// 三个状态：CLOSED（正常）→ OPEN（故障）→ HALF_OPEN（测试恢复）→ CLOSED
///
/// 设计原则（Linus式）：
/// 1. 状态明确，无歧义 - 每个状态行为清晰定义
/// 2. 一致性优先 - 宁愿失败也不破坏数据一致性
/// 3. 简单直接 - 三状态机，无复杂逻辑
pub struct FallbackBroadcaster {
    redis: Arc<dyn MessageBroadcaster>,
    local: Arc<dyn MessageBroadcaster>,
    state: AtomicU8, // 使用 u8 表示 CircuitState
    config: CircuitConfig,
    metrics: CircuitMetrics,
}

impl FallbackBroadcaster {
    pub fn new(redis: Arc<dyn MessageBroadcaster>, local: Arc<dyn MessageBroadcaster>) -> Self {
        Self {
            redis,
            local,
            state: AtomicU8::new(CircuitState::Closed as u8),
            config: CircuitConfig::default(),
            metrics: CircuitMetrics::new(),
        }
    }

    /// 获取当前断路器状态
    fn current_state(&self) -> CircuitState {
        CircuitState::from(self.state.load(Ordering::Relaxed))
    }

    /// 安全地转换状态并记录日志
    fn transition_to(&self, new_state: CircuitState) {
        let old_state = self.current_state();
        if old_state != new_state {
            self.state.store(new_state as u8, Ordering::Relaxed);

            match new_state {
                CircuitState::Closed => {
                    self.metrics.reset();
                    info!(
                        "Circuit breaker: {} → CLOSED (Normal operation)",
                        format!("{:?}", old_state)
                    );
                }
                CircuitState::Open => {
                    *self.metrics.last_failure_time.lock().unwrap() = Some(Instant::now());
                    error!(
                        "Circuit breaker: {} → OPEN (Redis unavailable, using local fallback only)",
                        format!("{:?}", old_state)
                    );
                }
                CircuitState::HalfOpen => {
                    self.metrics.half_open_calls.store(0, Ordering::Relaxed);
                    warn!(
                        "Circuit breaker: {} → HALF_OPEN (Testing Redis recovery)",
                        format!("{:?}", old_state)
                    );
                }
            }
        }
    }

    /// 记录成功操作
    fn record_success(&self) {
        let current_state = self.current_state();

        match current_state {
            CircuitState::Closed => {
                // 在正常状态下，重置失败计数
                self.metrics.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::HalfOpen => {
                let success_count = self.metrics.success_count.fetch_add(1, Ordering::Relaxed) + 1;
                info!(
                    "Circuit breaker: Success in HALF_OPEN state ({}/{})",
                    success_count, self.config.success_threshold
                );

                if success_count >= self.config.success_threshold {
                    self.transition_to(CircuitState::Closed);
                }
            }
            CircuitState::Open => {
                // 在断开状态下不应该有成功操作
                warn!("Circuit breaker: Unexpected success in OPEN state");
            }
        }
    }

    /// 记录失败操作
    fn record_failure(&self) {
        let current_state = self.current_state();

        match current_state {
            CircuitState::Closed => {
                let failure_count = self.metrics.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                warn!(
                    "Circuit breaker: Failure in CLOSED state ({}/{})",
                    failure_count, self.config.failure_threshold
                );

                if failure_count >= self.config.failure_threshold {
                    self.transition_to(CircuitState::Open);
                }
            }
            CircuitState::HalfOpen => {
                error!("Circuit breaker: Failure in HALF_OPEN state, reopening circuit");
                self.transition_to(CircuitState::Open);
            }
            CircuitState::Open => {
                // 在断开状态下，更新最后失败时间
                *self.metrics.last_failure_time.lock().unwrap() = Some(Instant::now());
            }
        }
    }

    /// 检查是否可以尝试请求
    fn can_attempt_request(&self) -> bool {
        match self.current_state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // 检查是否超过超时时间，可以转为半开状态
                if let Some(last_failure) = *self.metrics.last_failure_time.lock().unwrap() {
                    if last_failure.elapsed() >= self.config.timeout {
                        self.transition_to(CircuitState::HalfOpen);
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => {
                // 检查半开状态下的尝试次数
                let current_calls = self.metrics.half_open_calls.fetch_add(1, Ordering::Relaxed);
                current_calls < self.config.half_open_max_calls
            }
        }
    }

    /// 使用断路器保护的Redis操作
    async fn protected_redis_operation<F, T>(&self, operation: F) -> Result<T, BroadcastError>
    where
        F: FnOnce() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, BroadcastError>> + Send + 'static>,
        >,
    {
        if !self.can_attempt_request() {
            return Err(BroadcastError::failed("Circuit breaker is OPEN"));
        }

        match operation().await {
            Ok(result) => {
                self.record_success();
                Ok(result)
            }
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }
}

#[async_trait]
impl MessageBroadcaster for FallbackBroadcaster {
    /// 断路器保护的广播操作
    ///
    /// 行为规则（Linus式简洁明确）：
    /// - CLOSED: 使用Redis，失败则计数，达到阈值转OPEN
    /// - OPEN: 直接使用本地广播，不尝试Redis
    /// - HALF_OPEN: 限量尝试Redis，成功则转CLOSED，失败则转OPEN
    async fn broadcast(&self, message: MessageBroadcast) -> Result<(), BroadcastError> {
        let state = self.current_state();

        match state {
            CircuitState::Open => {
                // 断路器断开：直接使用本地广播，保证服务可用性
                info!("Circuit breaker OPEN: Using local broadcast only");
                self.local.broadcast(message).await
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                // 尝试Redis，失败时有两种策略：
                // 1. 如果是CLOSED状态，失败后计数，可能转OPEN
                // 2. 如果是HALF_OPEN状态，失败后立即转OPEN
                let redis = self.redis.clone();
                let msg_clone = message.clone();

                match self
                    .protected_redis_operation(move || {
                        Box::pin(async move { redis.broadcast(msg_clone).await })
                    })
                    .await
                {
                    Ok(_) => Ok(()),
                    Err(_) => {
                        // Redis失败，使用本地广播作为降级
                        info!("Redis broadcast failed, using local fallback");
                        self.local.broadcast(message).await
                    }
                }
            }
        }
    }

    /// 断路器保护的订阅操作
    ///
    /// 关键设计决策：订阅必须保持一致性
    /// - 一旦选择了Redis或本地，该连接的整个生命周期都使用同一个源
    /// - 不允许中途切换，避免消息来源混乱
    async fn subscribe(&self, room_id: RoomId) -> Result<MessageStream, BroadcastError> {
        let state = self.current_state();

        match state {
            CircuitState::Open => {
                // 断路器断开：只能使用本地订阅
                warn!(
                    "Circuit breaker OPEN: Subscribe using local broadcaster for room {:?}",
                    room_id
                );
                self.local.subscribe(room_id).await
            }
            CircuitState::Closed | CircuitState::HalfOpen => {
                // 尝试Redis订阅
                let redis = self.redis.clone();

                match self
                    .protected_redis_operation(move || {
                        Box::pin(async move { redis.subscribe(room_id).await })
                    })
                    .await
                {
                    Ok(stream) => {
                        info!(
                            "Circuit breaker: Redis subscription established for room {:?}",
                            room_id
                        );
                        Ok(stream)
                    }
                    Err(e) => {
                        // Redis订阅失败，降级到本地订阅
                        warn!(
                            "Redis subscription failed for room {:?}: {:?}, using local fallback",
                            room_id, e
                        );
                        self.local.subscribe(room_id).await
                    }
                }
            }
        }
    }
}
