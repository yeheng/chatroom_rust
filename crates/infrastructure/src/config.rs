//! 基础设施配置
//!
//! 定义 Kafka 和 Redis 的连接配置。

use serde::{Deserialize, Serialize};

/// Kafka 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaConfig {
    /// Kafka 服务器地址列表
    pub brokers: Vec<String>,
    /// 聊天事件主题名称
    pub chat_events_topic: String,
    /// 消费者组ID
    pub consumer_group_id: String,
    /// 消息发送超时时间（毫秒）
    pub send_timeout_ms: u32,
    /// 重试次数
    pub retry_count: u32,
    /// 分区数量
    pub partition_count: i32,
    /// 副本因子
    pub replication_factor: i32,
    /// 确认模式（all, 1, 0）
    pub acks: String,
    /// 批量大小
    pub batch_size: u32,
    /// 延迟时间（毫秒）
    pub linger_ms: u32,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            brokers: vec!["localhost:9092".to_string()],
            chat_events_topic: "chat-events".to_string(),
            consumer_group_id: "chatroom-service".to_string(),
            send_timeout_ms: 5000,
            retry_count: 3,
            partition_count: 12,
            replication_factor: 3,
            acks: "all".to_string(),
            batch_size: 16384,
            linger_ms: 5,
        }
    }
}

/// Redis 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis 服务器地址
    pub url: String,
    /// 连接池大小
    pub pool_size: u32,
    /// 连接超时时间（毫秒）
    pub connection_timeout_ms: u32,
    /// 重连间隔（毫秒）
    pub reconnect_interval_ms: u32,
    /// 最大重连次数
    pub max_reconnect_attempts: u32,
    /// 房间频道前缀
    pub room_channel_prefix: String,
    /// 全局频道名称
    pub global_channel: String,
    /// 消息过期时间（秒）
    pub message_ttl_seconds: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 20,
            connection_timeout_ms: 3000,
            reconnect_interval_ms: 1000,
            max_reconnect_attempts: 5,
            room_channel_prefix: "room:".to_string(),
            global_channel: "global".to_string(),
            message_ttl_seconds: 3600,
        }
    }
}

/// 消息架构配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingConfig {
    /// Kafka 配置
    pub kafka: KafkaConfig,
    /// Redis 配置
    pub redis: RedisConfig,
    /// 是否启用消息持久化
    pub enable_persistence: bool,
    /// 是否启用实时广播
    pub enable_broadcast: bool,
    /// 消息批量大小
    pub batch_size: usize,
    /// 批量处理间隔（毫秒）
    pub batch_interval_ms: u64,
}

impl Default for MessagingConfig {
    fn default() -> Self {
        Self {
            kafka: KafkaConfig::default(),
            redis: RedisConfig::default(),
            enable_persistence: true,
            enable_broadcast: true,
            batch_size: 100,
            batch_interval_ms: 100,
        }
    }
}

impl MessagingConfig {
    /// 从环境变量创建配置
    pub fn from_env() -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            // 设置默认值
            .set_default("kafka.brokers", vec!["localhost:9092"])?
            .set_default("kafka.chat_events_topic", "chat-events")?
            .set_default("kafka.consumer_group_id", "chatroom-service")?
            .set_default("kafka.send_timeout_ms", 5000)?
            .set_default("kafka.retry_count", 3)?
            .set_default("kafka.partition_count", 12)?
            .set_default("kafka.replication_factor", 3)?
            .set_default("kafka.acks", "all")?
            .set_default("kafka.batch_size", 16384)?
            .set_default("kafka.linger_ms", 5)?
            .set_default("redis.url", "redis://localhost:6379")?
            .set_default("redis.pool_size", 20)?
            .set_default("redis.connection_timeout_ms", 3000)?
            .set_default("redis.reconnect_interval_ms", 1000)?
            .set_default("redis.max_reconnect_attempts", 5)?
            .set_default("redis.room_channel_prefix", "room:")?
            .set_default("redis.global_channel", "global")?
            .set_default("redis.message_ttl_seconds", 3600)?
            .set_default("enable_persistence", true)?
            .set_default("enable_broadcast", true)?
            .set_default("batch_size", 100)?
            .set_default("batch_interval_ms", 100)?
            // 从环境变量加载
            .add_source(config::Environment::with_prefix("CHATROOM"))
            .build()?;

        settings.try_deserialize()
    }

    /// 验证配置
    pub fn validate(&self) -> Result<(), String> {
        if self.kafka.brokers.is_empty() {
            return Err("Kafka brokers cannot be empty".to_string());
        }

        if self.kafka.chat_events_topic.is_empty() {
            return Err("Kafka chat events topic cannot be empty".to_string());
        }

        if self.kafka.consumer_group_id.is_empty() {
            return Err("Kafka consumer group ID cannot be empty".to_string());
        }

        if self.redis.url.is_empty() {
            return Err("Redis URL cannot be empty".to_string());
        }

        if self.redis.pool_size == 0 {
            return Err("Redis pool size must be greater than 0".to_string());
        }

        if self.batch_size == 0 {
            return Err("Batch size must be greater than 0".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configs() {
        let kafka_config = KafkaConfig::default();
        assert!(!kafka_config.brokers.is_empty());
        assert_eq!(kafka_config.chat_events_topic, "chat-events");
        assert_eq!(kafka_config.acks, "all");

        let redis_config = RedisConfig::default();
        assert_eq!(redis_config.url, "redis://localhost:6379");
        assert_eq!(redis_config.room_channel_prefix, "room:");

        let messaging_config = MessagingConfig::default();
        assert!(messaging_config.enable_persistence);
        assert!(messaging_config.enable_broadcast);
    }

    #[test]
    fn test_config_validation() {
        let mut config = MessagingConfig::default();
        assert!(config.validate().is_ok());

        config.kafka.brokers.clear();
        assert!(config.validate().is_err());

        config.kafka.brokers = vec!["localhost:9092".to_string()];
        config.redis.pool_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = MessagingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MessagingConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.kafka.brokers, deserialized.kafka.brokers);
        assert_eq!(config.redis.url, deserialized.redis.url);
    }
}
