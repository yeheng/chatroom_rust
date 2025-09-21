//! 基础设施层
//!
//! 处理外部依赖和技术细节，包括消息队列、缓存、数据库等。

pub mod auth;
pub mod config;
pub mod db;
pub mod events;
pub mod kafka;
pub mod redis;
pub mod retry;
pub mod websocket;

// #[cfg(test)]
// mod websocket_test;

// 重新导出常用类型（避免使用通配符以防止名称冲突）
pub use auth::*;
pub use config::*;
pub use db::*;
pub use events::*;
pub use retry::*;
pub use websocket::*;

// 有选择地导出 Kafka/Redis 组件，避免将其内部的 `error` 模块名提升到顶层
pub use kafka::{consumer::KafkaMessageConsumer, producer::KafkaMessageProducer};
pub use redis::{publisher::RedisPublisher, subscriber::RedisSubscriber};
