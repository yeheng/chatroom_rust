//! Redis 消息订阅者
//!
//! 支持动态订阅房间频道和自动重连的订阅者实现。

use crate::{RedisConfig, RedisError, RedisResult, RedisRoomMessage};
use futures_util::stream::StreamExt;
use redis::{Client, ConnectionLike};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 消息处理器 trait
#[async_trait::async_trait]
pub trait RedisMessageHandler: Send + Sync {
    /// 处理接收到的房间消息
    async fn handle_room_message(
        &self,
        room_id: Uuid,
        message: RedisRoomMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// 处理系统通知
    async fn handle_system_notification(
        &self,
        message: String,
        level: crate::NotificationLevel,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// 订阅状态信息
#[derive(Debug, Clone, Default)]
struct SubscriptionStats {
    pub messages_received: u64,
    pub last_message: Option<Instant>,
    pub subscription_time: Option<Instant>,
    pub errors: u64,
}

/// Redis 消息订阅者
///
/// 支持动态订阅房间频道、全局频道。
pub struct RedisSubscriber {
    client: Client,
    config: RedisConfig,
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionStats>>>,
    shutdown_signal: Arc<AtomicBool>,
    message_sender: Option<mpsc::UnboundedSender<(String, RedisRoomMessage)>>,
}

impl RedisSubscriber {
    /// 创建新的 Redis 订阅者
    ///
    /// # 参数
    /// - `config`: Redis 配置
    ///
    /// # 返回
    /// - `Ok(RedisSubscriber)`: 成功创建的订阅者
    /// - `Err(RedisError)`: 创建失败的错误
    pub async fn new(config: &RedisConfig) -> RedisResult<Self> {
        let client = Client::open(config.url.as_str()).map_err(|e| RedisError::ConfigError {
            message: format!("创建 Redis 客户端失败: {}", e),
        })?;

        // 测试连接
        let conn = client
            .get_connection()
            .map_err(|e| RedisError::ConnectionError {
                message: format!("连接 Redis 失败: {}", e),
            })?;

        // 检查连接状态
        if !conn.is_open() {
            return Err(RedisError::ConnectionError {
                message: "Redis 连接不可用".to_string(),
            });
        }

        info!("Redis 订阅者创建成功，已连接到: {}", config.url);

        Ok(Self {
            client,
            config: config.clone(),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            message_sender: None,
        })
    }

    /// 订阅房间频道
    ///
    /// # 参数
    /// - `room_id`: 房间ID
    ///
    /// # 返回
    /// - `Ok(())`: 订阅成功
    /// - `Err(RedisError)`: 订阅失败
    pub async fn subscribe_to_room(&self, room_id: Uuid) -> RedisResult<()> {
        let channel = format!("{}{}", self.config.room_channel_prefix, room_id);
        self.add_subscription(&channel).await
    }

    /// 取消订阅房间频道
    ///
    /// # 参数
    /// - `room_id`: 房间ID
    ///
    /// # 返回
    /// - `Ok(())`: 取消订阅成功
    /// - `Err(RedisError)`: 操作失败
    pub async fn unsubscribe_from_room(&self, room_id: Uuid) -> RedisResult<()> {
        let channel = format!("{}{}", self.config.room_channel_prefix, room_id);
        self.remove_subscription(&channel).await
    }

    /// 订阅全局频道
    pub async fn subscribe_to_global(&self) -> RedisResult<()> {
        self.add_subscription(&self.config.global_channel).await
    }

    /// 批量订阅房间频道
    ///
    /// # 参数
    /// - `room_ids`: 房间ID列表
    ///
    /// # 返回
    /// - `Ok(usize)`: 成功订阅的频道数量
    /// - `Err(RedisError)`: 订阅失败
    pub async fn subscribe_to_rooms(&self, room_ids: Vec<Uuid>) -> RedisResult<usize> {
        let mut success_count = 0;

        for room_id in room_ids {
            if let Ok(()) = self.subscribe_to_room(room_id).await {
                success_count += 1;
            }
        }

        info!("批量订阅完成，成功订阅 {} 个房间频道", success_count);
        Ok(success_count)
    }

    /// 使用通道模式开始监听
    ///
    /// # 返回
    /// - `Ok(mpsc::UnboundedReceiver<(String, RedisRoomMessage)>)`: 消息接收通道
    /// - `Err(RedisError)`: 启动失败
    pub async fn start_with_channel(
        &mut self,
    ) -> RedisResult<mpsc::UnboundedReceiver<(String, RedisRoomMessage)>> {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.message_sender = Some(sender);

        let subscriptions = {
            let subs = self.subscriptions.read().await;
            subs.keys().cloned().collect::<Vec<_>>()
        };

        if subscriptions.is_empty() {
            return Err(RedisError::SubscribeError {
                message: "没有订阅任何频道，请先添加订阅".to_string(),
            });
        }

        info!("开始通道模式监听 {} 个频道", subscriptions.len());

        let client = self.client.clone();
        let shutdown_signal = Arc::clone(&self.shutdown_signal);
        let config = self.config.clone();
        let subscriptions_arc = Arc::clone(&self.subscriptions);
        let sender = self.message_sender.as_ref().unwrap().clone();

        // 在后台任务中运行监听循环
        tokio::spawn(async move {
            Self::channel_listen_loop(
                client,
                shutdown_signal,
                config,
                subscriptions_arc,
                subscriptions,
                sender,
            )
            .await;
        });

        Ok(receiver)
    }

    /// 添加订阅
    async fn add_subscription(&self, channel: &str) -> RedisResult<()> {
        let mut subscriptions = self.subscriptions.write().await;
        if subscriptions.contains_key(channel) {
            debug!("频道 {} 已订阅", channel);
            return Ok(());
        }

        let mut stats = SubscriptionStats::default();
        stats.subscription_time = Some(Instant::now());
        subscriptions.insert(channel.to_string(), stats);

        info!("添加订阅频道: {}", channel);
        Ok(())
    }

    /// 移除订阅
    async fn remove_subscription(&self, channel: &str) -> RedisResult<()> {
        let mut subscriptions = self.subscriptions.write().await;
        if subscriptions.remove(channel).is_some() {
            info!("移除订阅频道: {}", channel);
        }

        Ok(())
    }

    /// 通道模式的监听循环
    async fn channel_listen_loop(
        client: Client,
        shutdown_signal: Arc<AtomicBool>,
        config: RedisConfig,
        subscriptions: Arc<RwLock<HashMap<String, SubscriptionStats>>>,
        _initial_channels: Vec<String>,
        sender: mpsc::UnboundedSender<(String, RedisRoomMessage)>,
    ) {
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 5;

        while !shutdown_signal.load(Ordering::Relaxed) {
            let channels = {
                let subs = subscriptions.read().await;
                subs.keys().cloned().collect::<Vec<_>>()
            };

            if channels.is_empty() {
                warn!("没有活跃的订阅频道，等待新订阅");
                sleep(Duration::from_millis(1000)).await;
                continue;
            }

            match Self::create_and_listen(
                &client,
                &channels,
                &sender,
                &shutdown_signal,
                &subscriptions,
            )
            .await
            {
                Ok(_) => {
                    retry_count = 0;
                    info!("Redis 通道监听正常退出");
                }
                Err(e) => {
                    error!("Redis 通道监听错误: {}", e);
                    retry_count += 1;

                    if retry_count >= MAX_RETRIES {
                        error!("连接失败，已达最大重试次数");
                        break;
                    }

                    let delay = Duration::from_millis(
                        config.reconnect_interval_ms as u64 * (2_u64.pow(retry_count - 1)),
                    );
                    sleep(delay).await;
                }
            }
        }

        info!("Redis 通道监听已停止");
    }

    /// 创建连接并监听
    async fn create_and_listen(
        client: &Client,
        channels: &[String],
        sender: &mpsc::UnboundedSender<(String, RedisRoomMessage)>,
        shutdown_signal: &Arc<AtomicBool>,
        subscriptions: &Arc<RwLock<HashMap<String, SubscriptionStats>>>,
    ) -> RedisResult<()> {
        let mut pubsub =
            client
                .get_async_pubsub()
                .await
                .map_err(|e| RedisError::ConnectionError {
                    message: format!("获取 PubSub 连接失败: {}", e),
                })?;

        for channel in channels {
            pubsub
                .subscribe(channel)
                .await
                .map_err(|e| RedisError::SubscribeError {
                    message: format!("订阅频道 {} 失败: {}", channel, e),
                })?;
        }

        info!("已订阅 {} 个频道", channels.len());

        loop {
            if shutdown_signal.load(Ordering::Relaxed) {
                break;
            }

            // 使用超时避免无限阻塞
            match tokio::time::timeout(Duration::from_millis(1000), async {
                pubsub.on_message().next().await
            })
            .await
            {
                Ok(Some(msg)) => {
                    let channel: String = msg.get_channel_name().to_string();
                    let payload: Result<String, _> = msg.get_payload();

                    match payload {
                        Ok(payload_str) => {
                            if let Err(e) =
                                Self::process_message_to_channel(&channel, &payload_str, sender)
                                    .await
                            {
                                error!("处理消息到通道失败: {}", e);
                                Self::update_subscription_stats_static(
                                    subscriptions,
                                    &channel,
                                    false,
                                )
                                .await;
                            } else {
                                Self::update_subscription_stats_static(
                                    subscriptions,
                                    &channel,
                                    true,
                                )
                                .await;
                            }
                        }
                        Err(e) => {
                            error!("获取消息负载失败: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // Stream 结束
                    break;
                }
                Err(_) => {
                    // 超时，继续循环检查信号
                    continue;
                }
            }
        }

        Ok(())
    }

    /// 处理消息到通道
    async fn process_message_to_channel(
        channel: &str,
        payload: &str,
        sender: &mpsc::UnboundedSender<(String, RedisRoomMessage)>,
    ) -> RedisResult<()> {
        let room_message: RedisRoomMessage =
            serde_json::from_str(payload).map_err(|e| RedisError::DeserializationError {
                message: format!("反序列化消息失败: {}", e),
            })?;

        debug!("通道模式接收到频道 {} 的消息", channel);

        if let Err(_) = sender.send((channel.to_string(), room_message)) {
            warn!("发送消息到通道失败，接收端可能已关闭");
        }

        Ok(())
    }

    /// 静态方法更新订阅统计信息
    async fn update_subscription_stats_static(
        subscriptions: &Arc<RwLock<HashMap<String, SubscriptionStats>>>,
        channel: &str,
        success: bool,
    ) {
        let mut stats = subscriptions.write().await;
        if let Some(channel_stat) = stats.get_mut(channel) {
            if success {
                channel_stat.messages_received += 1;
                channel_stat.last_message = Some(Instant::now());
            } else {
                channel_stat.errors += 1;
            }
        }
    }

    /// 获取订阅统计信息
    pub async fn get_subscription_stats(&self) -> HashMap<String, (u64, u64)> {
        let stats = self.subscriptions.read().await;
        stats
            .iter()
            .map(|(channel, stat)| (channel.clone(), (stat.messages_received, stat.errors)))
            .collect()
    }

    /// 获取当前订阅列表
    pub async fn get_subscriptions(&self) -> Vec<String> {
        let stats = self.subscriptions.read().await;
        stats.keys().cloned().collect()
    }

    /// 优雅关闭订阅者
    pub async fn shutdown(&self) -> RedisResult<()> {
        info!("开始关闭 Redis 订阅者");
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // 等待监听循环退出
        sleep(Duration::from_millis(2000)).await;

        info!("Redis 订阅者已关闭");
        Ok(())
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        !self.shutdown_signal.load(Ordering::Relaxed)
    }

    /// 清理过期订阅统计
    pub async fn cleanup_old_stats(&self, max_age: Duration) {
        let mut stats = self.subscriptions.write().await;
        let now = Instant::now();

        stats.retain(|_, stat| {
            stat.last_message
                .map(|last| now.duration_since(last) < max_age)
                .unwrap_or(false)
        });
    }
}

impl Drop for RedisSubscriber {
    fn drop(&mut self) {
        self.shutdown_signal.store(true, Ordering::Relaxed);
        info!("Redis 订阅者正在关闭");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct TestMessageHandler {
        received_messages: Arc<Mutex<Vec<(Uuid, RedisRoomMessage)>>>,
        received_notifications: Arc<Mutex<Vec<(String, crate::NotificationLevel)>>>,
    }

    #[async_trait::async_trait]
    impl RedisMessageHandler for TestMessageHandler {
        async fn handle_room_message(
            &self,
            room_id: Uuid,
            message: RedisRoomMessage,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut messages = self.received_messages.lock().unwrap();
            messages.push((room_id, message));
            Ok(())
        }

        async fn handle_system_notification(
            &self,
            message: String,
            level: crate::NotificationLevel,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut notifications = self.received_notifications.lock().unwrap();
            notifications.push((message, level));
            Ok(())
        }
    }

    impl TestMessageHandler {
        fn new() -> Self {
            Self {
                received_messages: Arc::new(Mutex::new(Vec::new())),
                received_notifications: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_received_messages(&self) -> Vec<(Uuid, RedisRoomMessage)> {
            self.received_messages.lock().unwrap().clone()
        }

        fn get_received_notifications(&self) -> Vec<(String, crate::NotificationLevel)> {
            self.received_notifications.lock().unwrap().clone()
        }
    }

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

    #[tokio::test]
    async fn test_subscriber_creation() {
        let config = create_test_config();

        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let subscriber = RedisSubscriber::new(&config).await;
            assert!(subscriber.is_ok());
        }
    }

    #[tokio::test]
    async fn test_room_subscription() {
        let config = create_test_config();

        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let subscriber = RedisSubscriber::new(&config).await.unwrap();
            let room_id = Uuid::new_v4();

            assert!(subscriber.subscribe_to_room(room_id).await.is_ok());

            let subscriptions = subscriber.get_subscriptions().await;
            assert!(!subscriptions.is_empty());

            assert!(subscriber.unsubscribe_from_room(room_id).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_batch_subscription() {
        let config = create_test_config();

        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let subscriber = RedisSubscriber::new(&config).await.unwrap();
            let room_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

            let result = subscriber.subscribe_to_rooms(room_ids.clone()).await;
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), room_ids.len());
        }
    }

    #[tokio::test]
    async fn test_channel_mode() {
        let config = create_test_config();

        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let mut subscriber = RedisSubscriber::new(&config).await.unwrap();

            // 先添加一些订阅
            subscriber.subscribe_to_global().await.unwrap();

            let receiver = subscriber.start_with_channel().await;
            assert!(receiver.is_ok());

            // 测试关闭
            assert!(subscriber.shutdown().await.is_ok());
        }
    }

    #[test]
    fn test_message_handler() {
        let handler = TestMessageHandler::new();
        assert!(handler.get_received_messages().is_empty());
        assert!(handler.get_received_notifications().is_empty());
    }

    #[tokio::test]
    async fn test_subscription_stats() {
        let config = create_test_config();

        if std::env::var("REDIS_INTEGRATION_TEST").is_ok() {
            let subscriber = RedisSubscriber::new(&config).await.unwrap();
            let room_id = Uuid::new_v4();

            subscriber.subscribe_to_room(room_id).await.unwrap();

            let stats = subscriber.get_subscription_stats().await;
            assert!(!stats.is_empty());
        }
    }
}
