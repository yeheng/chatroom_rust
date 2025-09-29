//! Stats Consumer 库
//!
//! 提供从 Redis Stream 读取事件并写入 PostgreSQL 的功能

pub mod event_storage;
pub mod pg_event_storage;

pub use event_storage::EventStorage;
pub use pg_event_storage::{create_event_storage, PgEventStorage};
