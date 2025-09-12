//! Kafka 错误类型定义

use thiserror::Error;

/// Kafka 操作错误
#[derive(Error, Debug)]
pub enum KafkaError {
    /// 连接错误
    #[error("Kafka 连接错误: {message}")]
    ConnectionError { message: String },

    /// 生产者错误
    #[error("Kafka 生产者错误: {message}")]
    ProducerError { message: String },

    /// 消费者错误
    #[error("Kafka 消费者错误: {message}")]
    ConsumerError { message: String },

    /// 序列化错误
    #[error("序列化错误: {message}")]
    SerializationError { message: String },

    /// 反序列化错误
    #[error("反序列化错误: {message}")]
    DeserializationError { message: String },

    /// 超时错误
    #[error("操作超时: {operation}")]
    TimeoutError { operation: String },

    /// 配置错误
    #[error("配置错误: {message}")]
    ConfigError { message: String },
}

/// Kafka 结果类型
pub type KafkaResult<T> = Result<T, KafkaError>;

impl From<rdkafka::error::KafkaError> for KafkaError {
    fn from(err: rdkafka::error::KafkaError) -> Self {
        match err {
            rdkafka::error::KafkaError::ClientConfig(..) => KafkaError::ConfigError {
                message: err.to_string(),
            },
            rdkafka::error::KafkaError::ConsumerCommit(_) => KafkaError::ConsumerError {
                message: err.to_string(),
            },
            rdkafka::error::KafkaError::Canceled => KafkaError::ProducerError {
                message: "操作被取消".to_string(),
            },
            _ => KafkaError::ConnectionError {
                message: err.to_string(),
            },
        }
    }
}

impl From<serde_json::Error> for KafkaError {
    fn from(err: serde_json::Error) -> Self {
        KafkaError::SerializationError {
            message: err.to_string(),
        }
    }
}
