//! Redis 消息发布者
//!
//! 使用连接池避免连接创建开销，支持房间频道和全局频道。

use crate::{ChatEvent, RedisConfig, RedisError, RedisMessageType, RedisResult, RedisRoomMessage};
use redis::{Client, Connection};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Redis 发布者
///
/// 负责将消息发布到 Redis 频道，支持房间频道和全局频道。
pub struct RedisPublisher {
    client: Client,
    connections: Arc<Mutex<Vec<Connection>>>,
    config: RedisConfig,
    channel_stats: Arc<RwLock<HashMap<String, ChannelStats>>>,
}

/// 频道统计信息
#[derive(Debug, Clone, Default)]
struct ChannelStats {
    pub messages_sent: u64,
    pub last_publish: Option<Instant>,
    pub errors: u64,
}

impl RedisPublisher {
    /// 创建新的 Redis 发布者
    ///
    /// # 参数
    /// - `config`: Redis 配置
    ///
    /// # 返回
    /// - `Ok(RedisPublisher)`: 成功创建的发布者
    /// - `Err(RedisError)`: 创建失败的错误
    pub async fn new(config: &RedisConfig) -> RedisResult<Self> {
        let client = Client::open(config.url.as_str()).map_err(|e| RedisError::ConfigError {
            message: format!("创建 Redis 客户端失败: {}", e),
        })?;

        // 预创建连接池
        let mut connections = Vec::new();
        for i in 0..config.pool_size {
            match client.get_connection() {
                Ok(conn) => connections.push(conn),
                Err(e) => {
                    warn!("创建连接 {} 失败: {}", i, e);
                    if connections.is_empty() {
                        return Err(RedisError::ConnectionError {
                            message: format!("无法创建任何连接: {}", e),
                        });
                    }
                    break;
                }
            }
        }

        info!("Redis 发布者创建成功，连接池大小: {}", connections.len());

        Ok(Self {
            client,
            connections: Arc::new(Mutex::new(connections)),
            config: config.clone(),
            channel_stats: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 发布聊天事件到房间频道
    ///
    /// # 参数
    /// - `room_id`: 房间ID
    /// - `event`: 聊天事件
    ///
    /// # 返回
    /// - `Ok(u32)`: 订阅者数量
    /// - `Err(RedisError)`: 发布失败
    pub async fn publish_to_room(&self, room_id: Uuid, event: ChatEvent) -> RedisResult<u32> {
        if !event.should_broadcast() {
            return Ok(0);
        }

        let room_message =
            RedisRoomMessage::from_chat_event(event).ok_or_else(|| RedisError::PublishError {
                message: "无法从事件创建房间消息".to_string(),
            })?;

        let channel = format!("{}{}", self.config.room_channel_prefix, room_id);
        self.publish_message(&channel, &room_message).await
    }

    /// 发布系统通知到全局频道
    ///
    /// # 参数
    /// - `message`: 通知消息
    /// - `level`: 通知级别
    ///
    /// # 返回
    /// - `Ok(u32)`: 订阅者数量
    /// - `Err(RedisError)`: 发布失败
    pub async fn publish_system_notification(
        &self,
        message: String,
        level: crate::NotificationLevel,
    ) -> RedisResult<u32> {
        let notification = RedisMessageType::SystemNotification {
            message,
            level,
            timestamp: chrono::Utc::now(),
        };

        let room_message = RedisRoomMessage::new(Uuid::nil(), notification);
        self.publish_message(&self.config.global_channel, &room_message)
            .await
    }

    /// 批量发布消息到多个房间
    ///
    /// # 参数
    /// - `messages`: 房间ID和事件的映射
    ///
    /// # 返回
    /// - `Ok(usize)`: 成功发布的消息数量
    /// - `Err(RedisError)`: 发布失败
    pub async fn publish_batch(&self, messages: Vec<(Uuid, ChatEvent)>) -> RedisResult<usize> {
        let mut published_count = 0;
        let mut batch_futures = Vec::new();

        // 分组发布以提高效率
        for (room_id, event) in messages {
            if !event.should_broadcast() {
                continue;
            }

            let future = self.publish_to_room(room_id, event);
            batch_futures.push(future);

            // 批量大小限制
            if batch_futures.len() >= 10 {
                for future in batch_futures.drain(..) {
                    match future.await {
                        Ok(_) => published_count += 1,
                        Err(e) => error!("批量发布失败: {}", e),
                    }
                }
            }
        }

        // 发布剩余消息
        for future in batch_futures {
            match future.await {
                Ok(_) => published_count += 1,
                Err(e) => error!("批量发布失败: {}", e),
            }
        }

        info!("批量发布完成，成功发布 {} 条消息", published_count);
        Ok(published_count)
    }

    /// 发布消息到指定频道
    async fn publish_message<T>(&self, channel: &str, message: &T) -> RedisResult<u32>
    where
        T: serde::Serialize,
    {
        let payload =
            serde_json::to_string(message).map_err(|e| RedisError::SerializationError {
                message: format!("序列化消息失败: {}", e),
            })?;

        let result = self.publish_with_retry(channel, &payload, 0).await;

        // 更新统计信息
        self.update_channel_stats(channel, result.is_ok()).await;

        result
    }

    /// 带重试的发布
    async fn publish_with_retry(
        &self,
        channel: &str,
        payload: &str,
        retry_count: u32,
    ) -> RedisResult<u32> {
        let mut connections = self.connections.lock().await;

        if connections.is_empty() {
            return Err(RedisError::ConnectionError {
                message: "没有可用的连接".to_string(),
            });
        }

        // 从连接池获取连接
        let mut connection = connections.pop().unwrap();

        let result = {
            use redis::Commands;
            connection.publish::<_, _, u32>(channel, payload)
        };

        match result {
            Ok(subscriber_count) => {
                // 归还连接到池
                connections.push(connection);
                debug!(
                    "发布消息到频道 {} 成功，订阅者数量: {}",
                    channel, subscriber_count
                );
                Ok(subscriber_count)
            }
            Err(e) => {
                error!("发布消息到频道 {} 失败: {}", channel, e);

                if retry_count < 3 {
                    // 尝试重新创建连接
                    match self.client.get_connection() {
                        Ok(new_conn) => {
                            connections.push(new_conn);
                            drop(connections);

                            let delay = Duration::from_millis(100 * (2_u64.pow(retry_count)));
                            sleep(delay).await;

                            // 使用 Box::pin 来处理递归
                            return Box::pin(self.publish_with_retry(
                                channel,
                                payload,
                                retry_count + 1,
                            ))
                            .await;
                        }
                        Err(conn_err) => {
                            warn!("重新创建连接失败: {}", conn_err);
                        }
                    }
                }

                Err(RedisError::PublishError {
                    message: format!("发布失败: {}", e),
                })
            }
        }
    }

    /// 更新频道统计信息
    async fn update_channel_stats(&self, channel: &str, success: bool) {
        let mut stats = self.channel_stats.write().await;
        let channel_stat = stats.entry(channel.to_string()).or_default();

        if success {
            channel_stat.messages_sent += 1;
            channel_stat.last_publish = Some(Instant::now());
        } else {
            channel_stat.errors += 1;
        }
    }

    /// 获取频道统计信息
    pub async fn get_channel_stats(&self) -> HashMap<String, (u64, u64)> {
        let stats = self.channel_stats.read().await;
        stats
            .iter()
            .map(|(channel, stat)| (channel.clone(), (stat.messages_sent, stat.errors)))
            .collect()
    }

    /// 获取连接池状态
    pub async fn get_pool_status(&self) -> (usize, usize) {
        let connections = self.connections.lock().await;
        (connections.len(), self.config.pool_size as usize)
    }

    /// 清理过期统计信息
    pub async fn cleanup_old_stats(&self, max_age: Duration) {
        let mut stats = self.channel_stats.write().await;
        let now = Instant::now();

        stats.retain(|_, stat| {
            stat.last_publish
                .map(|last| now.duration_since(last) < max_age)
                .unwrap_or(false)
        });
    }
}

impl Drop for RedisPublisher {
    fn drop(&mut self) {
        info!("Redis 发布者正在关闭");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::ChatEvent;
    use chrono::Utc;
    use domain::message::Message;

    fn create_test_config() -> RedisConfig {
        RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 5,
            connection_timeout_ms: 1000,
            reconnect_interval_ms: 500,
            max_reconnect_attempts: 3,
            room_channel_prefix: "test_room:".to_string(),
            global_channel: "test_global".to_string(),
            message_ttl_seconds: 300,
        }
    }

    fn create_test_message_event() -> ChatEvent {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let message = Message::new_text(room_id, user_id, "Test message", None).unwrap();

        ChatEvent::MessageSent {
            message,
            room_id,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_publisher_creation() {
        let config = create_test_config();

        // 注意：这个测试需要运行 Redis 实例才能通过
        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let publisher = RedisPublisher::new(&config).await;
            assert!(publisher.is_ok());
        }
    }

    #[test]
    fn test_room_message_serialization() {
        let event = create_test_message_event();
        let room_message = RedisRoomMessage::from_chat_event(event).unwrap();

        let json = serde_json::to_string(&room_message);
        assert!(json.is_ok());

        let deserialized: Result<RedisRoomMessage, _> = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
    }

    #[tokio::test]
    async fn test_channel_stats_update() {
        let config = create_test_config();

        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let publisher = RedisPublisher::new(&config).await.unwrap();

            publisher.update_channel_stats("test_channel", true).await;
            publisher.update_channel_stats("test_channel", false).await;

            let stats = publisher.get_channel_stats().await;
            assert!(stats.contains_key("test_channel"));

            let (sent, errors) = stats.get("test_channel").unwrap();
            assert_eq!(*sent, 1);
            assert_eq!(*errors, 1);
        }
    }

    #[tokio::test]
    async fn test_pool_status() {
        let config = create_test_config();

        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let publisher = RedisPublisher::new(&config).await.unwrap();
            let (available, total) = publisher.get_pool_status().await;

            assert!(available <= total);
            assert_eq!(total, config.pool_size as usize);
        }
    }
}
