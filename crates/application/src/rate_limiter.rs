use domain::UserId;
use std::sync::Arc;
use std::time::Duration;

/// Redis-based用户消息配额管理
/// 使用Redis的原子操作实现高效的分布式限流

/// 限流错误类型
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded: {current}/{max} messages per minute")]
    RateLimitExceeded { current: u32, max: u32 },

    #[error("Too many connections: {current}/{max} connections per user")]
    TooManyConnections { current: u32, max: u32 },

    #[error("User temporarily banned: {reason}")]
    UserBanned { reason: String },

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
}

/// Redis-based消息限流器
/// 使用Redis原子操作实现分布式限流，支持水平扩展
pub struct MessageRateLimiter {
    /// 每分钟最大消息数
    max_messages_per_minute: u32,
    /// 每用户最大连接数
    max_connections_per_user: u32,
    /// 时间窗口大小（秒）
    window_duration: Duration,
    /// Redis客户端
    redis_client: Arc<redis::Client>,
}

impl MessageRateLimiter {
    /// 创建新的Redis-based限流器
    pub fn new(redis_client: Arc<redis::Client>, max_messages_per_minute: u32, max_connections_per_user: u32) -> Self {
        Self {
            max_messages_per_minute,
            max_connections_per_user,
            window_duration: Duration::from_secs(60), // 1分钟
            redis_client,
        }
    }

    /// 获取Redis连接
    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, RateLimitError> {
        self.redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RateLimitError::UserBanned {
                reason: format!("Redis connection failed: {}", e),
            })
    }

    /// 生成用户限流键
    fn rate_limit_key(&self, user_id: UserId) -> String {
        format!("rate_limit:{}", user_id)
    }

    /// 生成用户连接数键
    fn connection_count_key(&self, user_id: UserId) -> String {
        format!("connection_count:{}", user_id)
    }

    /// 检查用户是否可以发送消息
    /// 使用Redis原子操作实现分布式限流
    pub async fn check_message_rate(&self, user_id: UserId) -> Result<(), RateLimitError> {
        let mut conn = self.get_connection().await?;
        let key = self.rate_limit_key(user_id);

        // 使用Lua脚本确保原子性：INCR + EXPIRE
        let script = redis::Script::new(
            r#"
            local key = KEYS[1]
            local limit = tonumber(ARGV[1])
            local window = tonumber(ARGV[2])

            local current = redis.call('INCR', key)
            if current == 1 then
                redis.call('EXPIRE', key, window)
            end

            if current > limit then
                return {0, current}
            else
                return {1, current}
            end
            "#,
        );

        let result: Vec<i64> = script
            .key(&key)
            .arg(self.max_messages_per_minute as i64)
            .arg(self.window_duration.as_secs() as i64)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| RateLimitError::UserBanned {
                reason: format!("Redis script execution failed: {}", e),
            })?;

        if result[0] == 0 {
            return Err(RateLimitError::RateLimitExceeded {
                current: result[1] as u32,
                max: self.max_messages_per_minute,
            });
        }

        Ok(())
    }

    /// 检查用户连接数限制
    pub async fn check_connection_limit(&self, user_id: UserId) -> Result<(), RateLimitError> {
        let mut conn = self.get_connection().await?;
        let key = self.connection_count_key(user_id);

        let count: i64 = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        if count >= self.max_connections_per_user as i64 {
            return Err(RateLimitError::TooManyConnections {
                current: count as u32,
                max: self.max_connections_per_user,
            });
        }

        Ok(())
    }

    /// 用户连接时调用
    pub async fn add_connection(&self, user_id: UserId) -> Result<(), RateLimitError> {
        self.check_connection_limit(user_id).await?;

        let mut conn = self.get_connection().await?;
        let key = self.connection_count_key(user_id);

        let _: i64 = redis::cmd("INCR")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| RateLimitError::UserBanned {
                reason: format!("Redis INCR failed: {}", e),
            })?;

        // 设置过期时间，防止内存泄漏
        let _: () = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(86400) // 24小时
            .query_async(&mut conn)
            .await
            .unwrap_or(());

        Ok(())
    }

    /// 用户断开连接时调用
    pub async fn remove_connection(&self, user_id: UserId) {
        let mut conn = match self.get_connection().await {
            Ok(conn) => conn,
            Err(_) => return, // Redis连接失败，忽略错误
        };

        let key = self.connection_count_key(user_id);

        // 使用Lua脚本安全地减少计数
        let script = redis::Script::new(
            r#"
            local key = KEYS[1]
            local current = redis.call('GET', key)

            if current and tonumber(current) > 0 then
                redis.call('DECR', key)
                if tonumber(current) == 1 then
                    redis.call('DEL', key)
                end
            end
            "#,
        );

        let _: () = script
            .key(&key)
            .invoke_async(&mut conn)
            .await
            .unwrap_or(());
    }

    /// 获取用户当前状态
    pub async fn get_user_status(&self, user_id: UserId) -> Result<(u32, u32), RateLimitError> {
        let mut conn = self.get_connection().await?;

        let rate_limit_key = self.rate_limit_key(user_id);
        let connection_key = self.connection_count_key(user_id);

        let message_count: i64 = redis::cmd("GET")
            .arg(&rate_limit_key)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        let connection_count: i64 = redis::cmd("GET")
            .arg(&connection_key)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        Ok((message_count as u32, connection_count as u32))
    }

    /// 重置用户配额（管理员功能）
    pub async fn reset_user_quota(&self, user_id: UserId) -> Result<(), RateLimitError> {
        let mut conn = self.get_connection().await?;
        let rate_limit_key = self.rate_limit_key(user_id);
        let connection_key = self.connection_count_key(user_id);

        let _: () = redis::cmd("DEL")
            .arg(&rate_limit_key)
            .query_async(&mut conn)
            .await?;

        let _: () = redis::cmd("DEL")
            .arg(&connection_key)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }
}

// 移除Default实现，因为需要Redis客户端
// 使用者必须显式提供Redis客户端

// 测试需要Redis环境，暂时注释
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use uuid::Uuid;
//
//     #[tokio::test]
//     async fn test_rate_limiting() {
//         // 需要Redis客户端的测试实现
//     }
// }
