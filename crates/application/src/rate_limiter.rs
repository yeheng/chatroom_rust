use domain::UserId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use time::OffsetDateTime;

/// 用户消息配额
#[derive(Debug, Clone)]
pub struct UserQuota {
    /// 当前时间窗口内的消息数量
    pub message_count: u32,
    /// 当前时间窗口的开始时间
    pub window_start: Instant,
    /// 最后一次发送消息的时间
    pub last_message_time: OffsetDateTime,
}

impl Default for UserQuota {
    fn default() -> Self {
        Self::new()
    }
}

impl UserQuota {
    pub fn new() -> Self {
        Self {
            message_count: 0,
            window_start: Instant::now(),
            last_message_time: OffsetDateTime::now_utc(),
        }
    }

    /// 重置时间窗口
    pub fn reset_window(&mut self) {
        self.message_count = 0;
        self.window_start = Instant::now();
    }

    /// 检查是否超过限制
    pub fn is_over_limit(&self, max_messages: u32) -> bool {
        self.message_count >= max_messages
    }

    /// 增加消息计数
    pub fn increment(&mut self) {
        self.message_count += 1;
        self.last_message_time = OffsetDateTime::now_utc();
    }
}

/// 限流错误类型
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded: {current}/{max} messages per minute")]
    RateLimitExceeded { current: u32, max: u32 },

    #[error("Too many connections: {current}/{max} connections per user")]
    TooManyConnections { current: u32, max: u32 },

    #[error("User temporarily banned: {reason}")]
    UserBanned { reason: String },
}

/// 消息限流器
/// 防止消息洪水攻击，保护系统稳定性
pub struct MessageRateLimiter {
    /// 每分钟最大消息数
    max_messages_per_minute: u32,
    /// 每用户最大连接数
    max_connections_per_user: u32,
    /// 时间窗口大小（分钟）
    window_duration: Duration,
    /// 用户配额存储
    user_quotas: Arc<RwLock<HashMap<UserId, UserQuota>>>,
    /// 用户连接计数
    user_connections: Arc<RwLock<HashMap<UserId, u32>>>,
}

impl MessageRateLimiter {
    pub fn new(max_messages_per_minute: u32, max_connections_per_user: u32) -> Self {
        Self {
            max_messages_per_minute,
            max_connections_per_user,
            window_duration: Duration::from_secs(60), // 1分钟
            user_quotas: Arc::new(RwLock::new(HashMap::new())),
            user_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查用户是否可以发送消息
    pub fn check_message_rate(&self, user_id: UserId) -> Result<(), RateLimitError> {
        let mut quotas = self
            .user_quotas
            .write()
            .map_err(|_| RateLimitError::UserBanned {
                reason: "Internal error".to_string(),
            })?;

        let quota = quotas.entry(user_id).or_insert_with(UserQuota::new);
        let now = Instant::now();

        // 检查是否需要重置时间窗口
        if now.duration_since(quota.window_start) >= self.window_duration {
            quota.reset_window();
        }

        // 检查是否超过限制
        if quota.is_over_limit(self.max_messages_per_minute) {
            return Err(RateLimitError::RateLimitExceeded {
                current: quota.message_count,
                max: self.max_messages_per_minute,
            });
        }

        // 增加计数
        quota.increment();
        Ok(())
    }

    /// 检查用户连接数限制
    pub fn check_connection_limit(&self, user_id: UserId) -> Result<(), RateLimitError> {
        let connections = self
            .user_connections
            .read()
            .map_err(|_| RateLimitError::UserBanned {
                reason: "Internal error".to_string(),
            })?;

        if let Some(&connection_count) = connections.get(&user_id) {
            if connection_count >= self.max_connections_per_user {
                return Err(RateLimitError::TooManyConnections {
                    current: connection_count,
                    max: self.max_connections_per_user,
                });
            }
        }

        Ok(())
    }

    /// 用户连接时调用
    pub fn add_connection(&self, user_id: UserId) -> Result<(), RateLimitError> {
        self.check_connection_limit(user_id)?;

        let mut connections =
            self.user_connections
                .write()
                .map_err(|_| RateLimitError::UserBanned {
                    reason: "Internal error".to_string(),
                })?;

        *connections.entry(user_id).or_insert(0) += 1;
        Ok(())
    }

    /// 用户断开连接时调用
    pub fn remove_connection(&self, user_id: UserId) {
        if let Ok(mut connections) = self.user_connections.write() {
            if let Some(count) = connections.get_mut(&user_id) {
                if *count > 0 {
                    *count -= 1;
                }

                // 如果连接数为0，从map中移除
                if *count == 0 {
                    connections.remove(&user_id);
                }
            }
        }
    }

    /// 获取用户当前状态
    pub fn get_user_status(&self, user_id: UserId) -> Option<(u32, u32)> {
        let quotas = self.user_quotas.read().ok()?;
        let connections = self.user_connections.read().ok()?;

        let message_count = quotas.get(&user_id).map(|q| q.message_count).unwrap_or(0);

        let connection_count = connections.get(&user_id).copied().unwrap_or(0);

        Some((message_count, connection_count))
    }

    /// 清理过期的配额记录（防止内存泄漏）
    pub fn cleanup_expired_quotas(&self) {
        if let Ok(mut quotas) = self.user_quotas.write() {
            let now = Instant::now();
            let window_duration = self.window_duration;

            quotas.retain(|_, quota| now.duration_since(quota.window_start) < window_duration * 2);
        }
    }

    /// 重置用户配额（管理员功能）
    pub fn reset_user_quota(&self, user_id: UserId) {
        if let Ok(mut quotas) = self.user_quotas.write() {
            quotas.remove(&user_id);
        }
    }
}

impl Default for MessageRateLimiter {
    fn default() -> Self {
        Self::new(30, 5) // 默认每分钟30条消息，每用户5个连接
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_rate_limiting() {
        let limiter = MessageRateLimiter::new(5, 3); // 5 msg/min, 3 connections
        let user_id = UserId::from(Uuid::new_v4());

        // 发送5条消息应该成功
        for i in 0..5 {
            let result = limiter.check_message_rate(user_id);
            assert!(result.is_ok(), "Message {} should be allowed", i + 1);
        }

        // 第6条消息应该被限流
        let result = limiter.check_message_rate(user_id);
        assert!(result.is_err());

        if let Err(RateLimitError::RateLimitExceeded { current, max }) = result {
            assert_eq!(current, 5);
            assert_eq!(max, 5);
        } else {
            panic!("Expected RateLimitExceeded error");
        }
    }

    #[test]
    fn test_connection_limiting() {
        let limiter = MessageRateLimiter::new(10, 2); // 2 connections max
        let user_id = UserId::from(Uuid::new_v4());

        // 添加2个连接应该成功
        assert!(limiter.add_connection(user_id).is_ok());
        assert!(limiter.add_connection(user_id).is_ok());

        // 第3个连接应该被拒绝
        let result = limiter.add_connection(user_id);
        assert!(result.is_err());

        // 移除一个连接后应该可以再添加
        limiter.remove_connection(user_id);
        assert!(limiter.add_connection(user_id).is_ok());
    }

    #[test]
    fn test_window_reset() {
        let mut limiter = MessageRateLimiter::new(2, 5);
        limiter.window_duration = Duration::from_millis(100); // 短时间窗口用于测试

        let user_id = UserId::from(Uuid::new_v4());

        // 发送2条消息
        assert!(limiter.check_message_rate(user_id).is_ok());
        assert!(limiter.check_message_rate(user_id).is_ok());

        // 第3条消息应该被限流
        assert!(limiter.check_message_rate(user_id).is_err());

        // 等待时间窗口重置
        std::thread::sleep(Duration::from_millis(150));

        // 应该可以再发送消息
        assert!(limiter.check_message_rate(user_id).is_ok());
    }

    #[test]
    fn test_user_status() {
        let limiter = MessageRateLimiter::new(10, 5);
        let user_id = UserId::from(Uuid::new_v4());

        // 初始状态
        let status = limiter.get_user_status(user_id);
        assert_eq!(status, Some((0, 0)));

        // 发送消息和建立连接
        limiter.check_message_rate(user_id).unwrap();
        limiter.check_message_rate(user_id).unwrap();
        limiter.add_connection(user_id).unwrap();

        let status = limiter.get_user_status(user_id);
        assert_eq!(status, Some((2, 1)));
    }
}
