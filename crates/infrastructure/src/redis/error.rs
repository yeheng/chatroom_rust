//! Redis 错误类型定义

use thiserror::Error;

/// Redis 操作错误
#[derive(Error, Debug)]
pub enum RedisError {
    /// 连接错误
    #[error("Redis 连接错误: {message}")]
    ConnectionError { message: String },

    /// 发布错误
    #[error("Redis 发布错误: {message}")]
    PublishError { message: String },

    /// 订阅错误
    #[error("Redis 订阅错误: {message}")]
    SubscribeError { message: String },

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

    /// 重连错误
    #[error("重连失败: {message}")]
    ReconnectError { message: String },
}

/// Redis 结果类型
pub type RedisResult<T> = Result<T, RedisError>;

impl From<redis::RedisError> for RedisError {
    fn from(err: redis::RedisError) -> Self {
        match err.kind() {
            redis::ErrorKind::InvalidClientConfig => RedisError::ConfigError {
                message: err.to_string(),
            },
            redis::ErrorKind::IoError => RedisError::ConnectionError {
                message: err.to_string(),
            },
            _ => RedisError::ConnectionError {
                message: err.to_string(),
            },
        }
    }
}

impl From<serde_json::Error> for RedisError {
    fn from(err: serde_json::Error) -> Self {
        RedisError::SerializationError {
            message: err.to_string(),
        }
    }
}
