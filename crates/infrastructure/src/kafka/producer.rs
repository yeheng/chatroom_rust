//! Kafka 消息生产者
//!
//! 使用 room_id 作为分区键，确保同一房间消息的有序性。

use crate::{ChatEvent, KafkaConfig};
use crate::kafka::{KafkaError, KafkaResult};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

/// Kafka 消息生产者
///
/// 负责将聊天事件发送到 Kafka 主题，使用房间ID作为分区键确保消息顺序。
pub struct KafkaMessageProducer {
    producer: FutureProducer,
    topic: String,
    config: KafkaConfig,
}

impl KafkaMessageProducer {
    /// 创建新的 Kafka 生产者
    ///
    /// # 参数
    /// - `config`: Kafka 配置
    ///
    /// # 返回
    /// - `Ok(KafkaMessageProducer)`: 成功创建的生产者
    /// - `Err(KafkaError)`: 创建失败的错误
    pub async fn new(config: &KafkaConfig) -> KafkaResult<Self> {
        let mut client_config = ClientConfig::new();

        // 基本配置
        client_config
            .set("bootstrap.servers", config.brokers.join(","))
            .set("message.timeout.ms", config.send_timeout_ms.to_string())
            .set("acks", &config.acks)
            .set("retries", config.retry_count.to_string())
            .set("batch.size", config.batch_size.to_string())
            .set("linger.ms", config.linger_ms.to_string())
            .set("compression.type", "snappy") // 启用压缩
            .set("enable.idempotence", "true") // 启用幂等性
            .set("max.in.flight.requests.per.connection", "5");

        let producer: FutureProducer =
            client_config
                .create()
                .map_err(|e| KafkaError::ConfigError {
                    message: format!("创建 Kafka 生产者失败: {}", e),
                })?;

        info!("Kafka 生产者创建成功，连接到: {}", config.brokers.join(","));

        Ok(Self {
            producer,
            topic: config.chat_events_topic.clone(),
            config: config.clone(),
        })
    }

    /// 发送聊天事件
    ///
    /// # 参数
    /// - `event`: 要发送的聊天事件
    ///
    /// # 返回
    /// - `Ok(())`: 发送成功
    /// - `Err(KafkaError)`: 发送失败
    pub async fn send_event(&self, event: ChatEvent) -> KafkaResult<()> {
        if !event.should_persist() {
            // 不需要持久化的事件（如用户正在输入）直接跳过
            return Ok(());
        }

        let event_type = event.event_type();
        let room_id = event.room_id();

        // 序列化事件
        let payload =
            serde_json::to_string(&event).map_err(|e| KafkaError::SerializationError {
                message: format!("序列化事件失败: {}", e),
            })?;

        // 使用房间ID作为分区键，确保同一房间的消息有序
        let partition_key = match room_id {
            Some(id) => id.to_string(),
            None => "global".to_string(), // 全局事件使用固定键
        };

        self.send_with_retry(&payload, &partition_key, event_type, 0)
            .await
    }

    /// 批量发送事件
    ///
    /// # 参数
    /// - `events`: 要发送的事件列表
    ///
    /// # 返回
    /// - `Ok(usize)`: 成功发送的事件数量
    /// - `Err(KafkaError)`: 发送失败
    pub async fn send_events_batch(&self, events: Vec<ChatEvent>) -> KafkaResult<usize> {
        let mut sent_count = 0;

        for event in events {
            if !event.should_persist() {
                continue;
            }

            let event_type = event.event_type();
            let room_id = event.room_id();

            // 序列化事件
            let payload =
                serde_json::to_string(&event).map_err(|e| KafkaError::SerializationError {
                    message: format!("序列化事件失败: {}", e),
                })?;

            let partition_key = match room_id {
                Some(id) => id.to_string(),
                None => "global".to_string(),
            };

            // 直接发送每个事件，避免生命周期问题
            match self
                .send_with_retry(&payload, &partition_key, event_type, 0)
                .await
            {
                Ok(_) => {
                    sent_count += 1;
                }
                Err(e) => {
                    error!("批量发送事件 {} 失败: {}", event_type, e);
                    return Err(e);
                }
            }
        }

        info!("批量发送完成，成功发送 {} 个事件", sent_count);
        Ok(sent_count)
    }

    /// 带重试的发送
    async fn send_with_retry(
        &self,
        payload: &str,
        partition_key: &str,
        event_type: &str,
        retry_count: u32,
    ) -> KafkaResult<()> {
        let record = FutureRecord::to(&self.topic)
            .payload(payload)
            .key(partition_key);

        let timeout = Duration::from_millis(self.config.send_timeout_ms as u64);

        match self.producer.send(record, Timeout::After(timeout)).await {
            Ok(_) => {
                if retry_count > 0 {
                    info!("事件 {} 重试 {} 次后发送成功", event_type, retry_count);
                }
                Ok(())
            }
            Err((kafka_err, _)) => {
                if retry_count < self.config.retry_count {
                    warn!(
                        "事件 {} 发送失败，第 {} 次重试: {}",
                        event_type,
                        retry_count + 1,
                        kafka_err
                    );

                    // 指数退避
                    let delay = Duration::from_millis(100 * (2_u64.pow(retry_count)));
                    sleep(delay).await;

                    // 使用 Box::pin 来处理递归
                    return Box::pin(self.send_with_retry(
                        payload,
                        partition_key,
                        event_type,
                        retry_count + 1,
                    ))
                    .await;
                }

                error!(
                    "事件 {} 发送失败，已达最大重试次数: {}",
                    event_type, kafka_err
                );
                Err(KafkaError::ProducerError {
                    message: format!("发送失败: {}", kafka_err),
                })
            }
        }
    }

    /// 刷新生产者缓冲区
    pub async fn flush(&self) -> KafkaResult<()> {
        self.producer
            .flush(Timeout::After(Duration::from_secs(10)))
            .map_err(|e| KafkaError::ProducerError {
                message: format!("刷新生产者缓冲区失败: {}", e),
            })
    }

    /// 获取生产者统计信息
    pub fn get_stats(&self) -> String {
        // 这里可以扩展返回更详细的统计信息
        format!("Kafka Producer - Topic: {}", self.topic)
    }
}

impl Drop for KafkaMessageProducer {
    fn drop(&mut self) {
        info!("Kafka 生产者正在关闭");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::ChatEvent;
    use chrono::Utc;
    use domain::message::Message;
    use uuid::Uuid;

    fn create_test_config() -> KafkaConfig {
        KafkaConfig {
            brokers: vec!["localhost:9092".to_string()],
            chat_events_topic: "test-chat-events".to_string(),
            consumer_group_id: "test-group".to_string(),
            send_timeout_ms: 1000,
            retry_count: 2,
            partition_count: 3,
            replication_factor: 1,
            acks: "1".to_string(), // 测试环境使用较低要求
            batch_size: 1024,
            linger_ms: 1,
        }
    }

    fn create_test_message_event() -> ChatEvent {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let message = Message::new_text(room_id, user_id, "Test message".to_string()).unwrap();

        ChatEvent::MessageSent {
            message,
            room_id,
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_producer_creation() {
        let config = create_test_config();

        // 注意：这个测试需要运行 Kafka 实例才能通过
        // 在 CI 环境中可能需要跳过或使用 mock
        if std::env::var("KAFKA_INTEGRATION_TEST").is_ok() {
            let producer = KafkaMessageProducer::new(&config).await;
            assert!(producer.is_ok());
        }
    }

    #[test]
    fn test_event_serialization() {
        let event = create_test_message_event();
        let json = serde_json::to_string(&event);
        assert!(json.is_ok());

        let deserialized: Result<ChatEvent, _> = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
    }

    #[test]
    fn test_partition_key_generation() {
        let event = create_test_message_event();
        let room_id = event.room_id();
        assert!(room_id.is_some());

        let partition_key = room_id.unwrap().to_string();
        assert!(!partition_key.is_empty());
    }

    #[test]
    fn test_global_event_partition_key() {
        let user_id = Uuid::new_v4();
        let event = ChatEvent::UserStatusChanged {
            user_id,
            old_status: "Active".to_string(),
            new_status: "Offline".to_string(),
            timestamp: Utc::now(),
        };

        assert!(event.room_id().is_none());
        assert!(event.should_persist());
        assert!(event.should_broadcast());
    }

    #[test]
    fn test_typing_event_should_not_persist() {
        let event = ChatEvent::UserTyping {
            user_id: Uuid::new_v4(),
            room_id: Uuid::new_v4(),
            timestamp: Utc::now(),
        };

        assert!(!event.should_persist());
        assert!(event.should_broadcast());
    }
}
