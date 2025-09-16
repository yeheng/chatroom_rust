//! 总线实现
//!
//! 包含命令总线、查询总线、事件总线的具体实现

use crate::errors::ApplicationResult;
use crate::cqrs::{Command, Query, CommandBus, QueryBus, EventBus, CommandHandler, QueryHandler, EventHandler, DomainEvent};
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// 内存中的命令总线实现
pub struct InMemoryCommandBus {
    handlers: Arc<RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>>,
}

impl InMemoryCommandBus {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_handler<C: Command>(&self, handler: Arc<dyn CommandHandler<C>>) {
        let type_name = std::any::type_name::<C>().to_string();
        let mut handlers = self.handlers.write().await;
        handlers.insert(type_name, Box::new(handler));
    }
}

#[async_trait]
impl CommandBus for InMemoryCommandBus {
    async fn dispatch<C: Command>(&self, _command: C) -> ApplicationResult<C::Result> {
        // 由于 Rust 的类型系统限制，这里需要更复杂的实现
        // 暂时返回错误，实际项目中需要使用 trait objects 或其他模式
        todo!("Command bus dispatch implementation needed")
    }
}

/// 内存中的查询总线实现
pub struct InMemoryQueryBus {
    handlers: Arc<RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>>,
}

impl InMemoryQueryBus {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register_handler<Q: Query>(&self, handler: Arc<dyn QueryHandler<Q>>) {
        let type_name = std::any::type_name::<Q>().to_string();
        let mut handlers = self.handlers.write().await;
        handlers.insert(type_name, Box::new(handler));
    }
}

#[async_trait]
impl QueryBus for InMemoryQueryBus {
    async fn dispatch<Q: Query>(&self, _query: Q) -> ApplicationResult<Q::Result> {
        // 由于 Rust 的类型系统限制，这里需要更复杂的实现
        // 暂时返回错误，实际项目中需要使用 trait objects 或其他模式
        todo!("Query bus dispatch implementation needed")
    }
}

/// 内存中的事件总线实现
pub struct InMemoryEventBus {
    handlers: Arc<RwLock<Vec<Arc<dyn EventHandler>>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(&self, event: Arc<dyn DomainEvent>) -> ApplicationResult<()> {
        let handlers = self.handlers.read().await;
        let event_type = event.event_type();

        for handler in handlers.iter() {
            if handler.can_handle(event_type) {
                handler.handle(event.clone()).await?;
            }
        }

        Ok(())
    }

    async fn subscribe(&self, handler: Arc<dyn EventHandler>) -> ApplicationResult<()> {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
        Ok(())
    }
}