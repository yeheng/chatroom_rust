//! CQRS（命令查询职责分离）架构核心组件
//!
//! 本模块实现了 CQRS 模式的核心接口和组件，包括：
//! - 命令和查询的基础接口
//! - 命令总线、查询总线、事件总线
//! - 处理器接口

pub mod commands;
pub mod queries;
pub mod handlers;
pub mod dtos;
pub mod buses;
pub mod events;

pub use commands::*;
pub use queries::*;
pub use handlers::*;
pub use dtos::*;
// pub use buses::*;
// pub use events::*;

use crate::errors::ApplicationResult;
use async_trait::async_trait;
use std::sync::Arc;

/// 命令特征 - 所有命令必须实现此特征
pub trait Command: Send + Sync + 'static {
    /// 命令执行的返回类型
    type Result;
}

/// 查询特征 - 所有查询必须实现此特征
pub trait Query: Send + Sync + 'static {
    /// 查询的返回类型
    type Result;
}

/// 命令处理器接口
#[async_trait]
pub trait CommandHandler<C: Command>: Send + Sync {
    async fn handle(&self, command: C) -> ApplicationResult<C::Result>;
}

/// 查询处理器接口
#[async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync {
    async fn handle(&self, query: Q) -> ApplicationResult<Q::Result>;
}

/// 事件处理器接口
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: Arc<dyn DomainEvent>) -> ApplicationResult<()>;
    fn can_handle(&self, event_type: &str) -> bool;
}

/// 命令总线接口
#[async_trait]
pub trait CommandBus: Send + Sync {
    async fn dispatch<C: Command>(&self, command: C) -> ApplicationResult<C::Result>;
}

/// 查询总线接口
#[async_trait]
pub trait QueryBus: Send + Sync {
    async fn dispatch<Q: Query>(&self, query: Q) -> ApplicationResult<Q::Result>;
}

/// 事件总线接口
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: Arc<dyn DomainEvent>) -> ApplicationResult<()>;
    async fn subscribe(&self, handler: Arc<dyn EventHandler>) -> ApplicationResult<()>;
}

/// 领域事件接口
pub trait DomainEvent: Send + Sync {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> String;
    fn version(&self) -> u64;
    fn timestamp(&self) -> chrono::DateTime<chrono::Utc>;
}

/// 应用服务基础结构
pub struct ChatRoomApplicationService {
    // 暂时注释掉，避免 trait object 兼容性问题
    // pub command_bus: Arc<dyn CommandBus>,
    // pub query_bus: Arc<dyn QueryBus>,
    pub event_bus: Option<Arc<dyn EventBus>>,
}