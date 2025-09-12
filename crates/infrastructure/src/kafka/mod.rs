//! Kafka 消息队列模块
//!
//! 提供基于房间分区的 Kafka 生产者和消费者实现。

pub mod consumer;
pub mod error;
pub mod producer;

// 重新导出
pub use consumer::*;
pub use error::*;
pub use producer::*;
