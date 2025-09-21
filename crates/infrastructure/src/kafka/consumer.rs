//! Kafka 消息消费者
//!
//! 支持优雅关闭和错误恢复的 Kafka 消费者实现。

use crate::{ChatEvent, KafkaConfig};
use crate::kafka::{KafkaError, KafkaResult};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::{BorrowedMessage, Message};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// 消息处理器 trait
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    /// 处理接收到的聊天事件
    async fn handle_event(
        &self,
        event: ChatEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Kafka 消息消费者
///
/// 作为消费者组成员，利用 Kafka 自动分区重平衡机制。
pub struct KafkaMessageConsumer {
    consumer: StreamConsumer,
    topic: String,
    config: KafkaConfig,
    shutdown_signal: Arc<AtomicBool>,
    message_sender: Option<mpsc::UnboundedSender<ChatEvent>>,
}

impl KafkaMessageConsumer {
    /// 创建新的 Kafka 消费者
    ///
    /// # 参数
    /// - `config`: Kafka 配置
    ///
    /// # 返回
    /// - `Ok(KafkaMessageConsumer)`: 成功创建的消费者
    /// - `Err(KafkaError)`: 创建失败的错误
    pub async fn new(config: &KafkaConfig) -> KafkaResult<Self> {
        let mut client_config = ClientConfig::new();

        // 基本配置
        client_config
            .set("group.id", &config.consumer_group_id)
            .set("bootstrap.servers", config.brokers.join(","))
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "10000")
            .set("heartbeat.interval.ms", "3000")
            .set("enable.auto.commit", "true")
            .set("auto.commit.interval.ms", "1000")
            .set("auto.offset.reset", "latest") // 从最新位置开始消费
            .set("fetch.wait.max.ms", "100")
            .set("fetch.min.bytes", "1")
            .set("fetch.max.bytes", "50000000")
            .set("max.partition.fetch.bytes", "1048576");

        let consumer: StreamConsumer =
            client_config
                .create()
                .map_err(|e| KafkaError::ConfigError {
                    message: format!("创建 Kafka 消费者失败: {}", e),
                })?;

        info!(
            "Kafka 消费者创建成功，消费者组: {}",
            config.consumer_group_id
        );

        Ok(Self {
            consumer,
            topic: config.chat_events_topic.clone(),
            config: config.clone(),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            message_sender: None,
        })
    }

    /// 订阅主题并开始消费
    ///
    /// # 参数
    /// - `handler`: 消息处理器
    ///
    /// # 返回
    /// - `Ok(())`: 订阅成功
    /// - `Err(KafkaError)`: 订阅失败
    pub async fn subscribe_and_consume<H>(&mut self, handler: Arc<H>) -> KafkaResult<()>
    where
        H: MessageHandler + 'static,
    {
        // 订阅主题
        self.consumer
            .subscribe(&[&self.topic])
            .map_err(|e| KafkaError::ConsumerError {
                message: format!("订阅主题失败: {}", e),
            })?;

        info!("已订阅主题: {}", self.topic);

        // 开始消费循环
        self.consume_loop(handler).await
    }

    /// 使用通道模式消费消息
    ///
    /// # 返回
    /// - `Ok(mpsc::UnboundedReceiver<ChatEvent>)`: 消息接收通道
    /// - `Err(KafkaError)`: 启动失败
    pub async fn start_with_channel(&mut self) -> KafkaResult<mpsc::UnboundedReceiver<ChatEvent>> {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.message_sender = Some(sender);

        // 订阅主题
        self.consumer
            .subscribe(&[&self.topic])
            .map_err(|e| KafkaError::ConsumerError {
                message: format!("订阅主题失败: {}", e),
            })?;

        info!("已订阅主题: {}，开始通道模式消费", self.topic);

        // 为后台任务创建新的consumer而非clone
        let consumer_config = {
            let mut client_config = ClientConfig::new();
            client_config
                .set("group.id", &self.config.consumer_group_id)
                .set("bootstrap.servers", self.config.brokers.join(","))
                .set("enable.partition.eof", "false")
                .set("session.timeout.ms", "10000")
                .set("heartbeat.interval.ms", "3000")
                .set("enable.auto.commit", "true")
                .set("auto.commit.interval.ms", "1000")
                .set("auto.offset.reset", "latest")
                .set("fetch.wait.max.ms", "100")
                .set("fetch.min.bytes", "1")
                .set("fetch.max.bytes", "50000000")
                .set("max.partition.fetch.bytes", "1048576");
            client_config
        };

        let background_consumer: StreamConsumer =
            consumer_config
                .create()
                .map_err(|e| KafkaError::ConfigError {
                    message: format!("创建后台消费者失败: {}", e),
                })?;

        background_consumer
            .subscribe(&[&self.topic])
            .map_err(|e| KafkaError::ConsumerError {
                message: format!("后台消费者订阅失败: {}", e),
            })?;

        let shutdown_signal = Arc::clone(&self.shutdown_signal);
        let sender = self.message_sender.as_ref().unwrap().clone();

        // 在后台任务中运行消费循环
        tokio::spawn(async move {
            Self::channel_consume_loop(background_consumer, shutdown_signal, sender).await;
        });

        Ok(receiver)
    }

    /// 消费循环
    async fn consume_loop<H>(&self, handler: Arc<H>) -> KafkaResult<()>
    where
        H: MessageHandler + 'static,
    {
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 5;

        while !self.shutdown_signal.load(Ordering::Relaxed) {
            match self.consumer.recv().await {
                Ok(message) => {
                    retry_count = 0; // 重置重试计数

                    if let Err(e) = self.process_message(&message, &handler).await {
                        error!("处理消息失败: {}", e);
                        // 继续处理下一条消息，不中断消费
                    }
                }
                Err(e) => {
                    error!("接收消息失败: {}", e);
                    retry_count += 1;

                    if retry_count >= MAX_RETRIES {
                        error!("达到最大重试次数，停止消费");
                        return Err(KafkaError::ConsumerError {
                            message: format!("消费失败，已重试 {} 次", MAX_RETRIES),
                        });
                    }

                    // 指数退避
                    let delay = Duration::from_millis(1000 * (2_u64.pow(retry_count - 1)));
                    warn!("等待 {:?} 后重试...", delay);
                    sleep(delay).await;
                }
            }
        }

        info!("消费循环已停止");
        Ok(())
    }

    /// 通道模式的消费循环
    async fn channel_consume_loop(
        consumer: StreamConsumer,
        shutdown_signal: Arc<AtomicBool>,
        sender: mpsc::UnboundedSender<ChatEvent>,
    ) {
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 5;

        while !shutdown_signal.load(Ordering::Relaxed) {
            match consumer.recv().await {
                Ok(message) => {
                    retry_count = 0;

                    if let Err(e) = Self::process_message_to_channel(&message, &sender).await {
                        error!("处理消息到通道失败: {}", e);
                    }
                }
                Err(e) => {
                    error!("接收消息失败: {}", e);
                    retry_count += 1;

                    if retry_count >= MAX_RETRIES {
                        error!("达到最大重试次数，停止通道消费");
                        break;
                    }

                    let delay = Duration::from_millis(1000 * (2_u64.pow(retry_count - 1)));
                    sleep(delay).await;
                }
            }
        }

        info!("通道消费循环已停止");
    }

    /// 处理单条消息
    async fn process_message<H>(
        &self,
        message: &BorrowedMessage<'_>,
        handler: &Arc<H>,
    ) -> KafkaResult<()>
    where
        H: MessageHandler,
    {
        // 获取消息负载
        let payload = message
            .payload()
            .ok_or_else(|| KafkaError::DeserializationError {
                message: "消息负载为空".to_string(),
            })?;

        // 反序列化事件
        let event: ChatEvent =
            serde_json::from_slice(payload).map_err(|e| KafkaError::DeserializationError {
                message: format!("反序列化事件失败: {}", e),
            })?;

        debug!(
            "接收到事件: {} (分区: {}, 偏移量: {})",
            event.event_type(),
            message.partition(),
            message.offset()
        );

        // 处理事件
        if let Err(e) = handler.handle_event(event).await {
            return Err(KafkaError::ConsumerError {
                message: format!("事件处理失败: {}", e),
            });
        }

        Ok(())
    }

    /// 处理消息到通道
    async fn process_message_to_channel(
        message: &BorrowedMessage<'_>,
        sender: &mpsc::UnboundedSender<ChatEvent>,
    ) -> KafkaResult<()> {
        let payload = message
            .payload()
            .ok_or_else(|| KafkaError::DeserializationError {
                message: "消息负载为空".to_string(),
            })?;

        let event: ChatEvent =
            serde_json::from_slice(payload).map_err(|e| KafkaError::DeserializationError {
                message: format!("反序列化事件失败: {}", e),
            })?;

        debug!(
            "通道模式接收到事件: {} (分区: {}, 偏移量: {})",
            event.event_type(),
            message.partition(),
            message.offset()
        );

        if sender.send(event).is_err() {
            warn!("发送事件到通道失败，接收端可能已关闭");
        }

        Ok(())
    }

    /// 获取消费者统计信息
    pub fn get_stats(&self) -> String {
        format!(
            "Kafka Consumer - Topic: {}, Group: {}",
            self.topic, self.config.consumer_group_id
        )
    }

    /// 优雅关闭消费者
    pub async fn shutdown(&self) -> KafkaResult<()> {
        info!("开始关闭 Kafka 消费者");
        self.shutdown_signal.store(true, Ordering::Relaxed);

        // 等待一段时间让消费循环退出
        sleep(Duration::from_millis(1000)).await;

        info!("Kafka 消费者已关闭");
        Ok(())
    }

    /// 检查消费者是否正在运行
    pub fn is_running(&self) -> bool {
        !self.shutdown_signal.load(Ordering::Relaxed)
    }
}

impl Drop for KafkaMessageConsumer {
    fn drop(&mut self) {
        self.shutdown_signal.store(true, Ordering::Relaxed);
        info!("Kafka 消费者正在释放资源");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct TestMessageHandler {
        received_events: Arc<Mutex<Vec<ChatEvent>>>,
    }

    #[async_trait::async_trait]
    impl MessageHandler for TestMessageHandler {
        async fn handle_event(
            &self,
            event: ChatEvent,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let mut events = self.received_events.lock().unwrap();
            events.push(event);
            Ok(())
        }
    }

    impl TestMessageHandler {
        fn new() -> Self {
            Self {
                received_events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_received_events(&self) -> Vec<ChatEvent> {
            self.received_events.lock().unwrap().clone()
        }
    }

    fn create_test_config() -> KafkaConfig {
        KafkaConfig {
            brokers: vec!["localhost:9092".to_string()],
            chat_events_topic: "test-chat-events".to_string(),
            consumer_group_id: "test-consumer-group".to_string(),
            send_timeout_ms: 1000,
            retry_count: 2,
            partition_count: 3,
            replication_factor: 1,
            acks: "1".to_string(),
            batch_size: 1024,
            linger_ms: 1,
        }
    }

    #[tokio::test]
    async fn test_consumer_creation() {
        let config = create_test_config();

        if std::env::var("KAFKA_INTEGRATION_TEST").is_ok() {
            let consumer = KafkaMessageConsumer::new(&config).await;
            assert!(consumer.is_ok());
        }
    }

    #[tokio::test]
    async fn test_channel_mode() {
        let config = create_test_config();

        if std::env::var("KAFKA_INTEGRATION_TEST").is_ok() {
            let mut consumer = KafkaMessageConsumer::new(&config).await.unwrap();
            let receiver = consumer.start_with_channel().await;
            assert!(receiver.is_ok());

            // 测试关闭
            assert!(consumer.shutdown().await.is_ok());
        }
    }

    #[test]
    fn test_message_handler() {
        let handler = TestMessageHandler::new();
        assert!(handler.get_received_events().is_empty());
    }

    #[tokio::test]
    async fn test_shutdown_signal() {
        let config = create_test_config();

        if std::env::var("KAFKA_INTEGRATION_TEST").is_ok() {
            let consumer = KafkaMessageConsumer::new(&config).await.unwrap();
            assert!(consumer.is_running());

            consumer.shutdown().await.unwrap();
            assert!(!consumer.is_running());
        }
    }
}
