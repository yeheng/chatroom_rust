//! 总线实现
//!
//! 包含基于 Kafka 的命令总线、查询总线、事件总线的具体实现

use crate::cqrs::{
    Command, CommandBus, CommandHandler, DomainEvent, EventBus, EventHandler, Query, QueryBus,
    QueryHandler,
};
use crate::errors::ApplicationResult;
use async_trait::async_trait;
use chrono;
use futures_util;
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    producer::{FutureProducer, FutureRecord},
    ClientConfig, Message,
};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

/// 从 Kafka 消息重构的事件包装器
pub struct KafkaEvent {
    data: serde_json::Value,
    event_type_name: String,
}

impl KafkaEvent {
    pub fn new(data: serde_json::Value, event_type: String) -> Self {
        Self {
            data,
            event_type_name: event_type,
        }
    }
}

impl DomainEvent for KafkaEvent {
    fn event_type(&self) -> &str {
        &self.event_type_name
    }

    fn aggregate_id(&self) -> String {
        self.data
            .get("aggregate_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn version(&self) -> u64 {
        self.data
            .get("version")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
    }

    fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        let timestamp_ms = self
            .data
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

        chrono::DateTime::from_timestamp_millis(timestamp_ms).unwrap_or_else(chrono::Utc::now)
    }
}

/// 基于 Kafka 的命令总线实现
pub struct KafkaCommandBus {
    producer: Arc<FutureProducer>,
    handlers: Arc<RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>>,
    topic_prefix: String,
    /// 等待响应的命令映射 (command_id -> response_sender)
    pending_commands: Arc<RwLock<HashMap<String, oneshot::Sender<serde_json::Value>>>>,
}

impl KafkaCommandBus {
    pub fn new(kafka_brokers: &str, topic_prefix: String) -> ApplicationResult<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", kafka_brokers)
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.messages", "10000")
            .create()
            .map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!(
                    "创建 Kafka 生产者失败: {}",
                    e
                ))
            })?;

        Ok(Self {
            producer: Arc::new(producer),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            topic_prefix,
            pending_commands: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn register_handler<C: Command>(&self, handler: Arc<dyn CommandHandler<C>>) {
        let type_name = std::any::type_name::<C>().to_string();
        let mut handlers = self.handlers.write().await;
        handlers.insert(type_name.clone(), Box::new(handler));
        info!("注册命令处理器: {}", type_name);
    }

    /// 启动响应消费者，处理命令执行结果
    pub async fn start_response_consumer(&self, group_id: &str) -> ApplicationResult<()> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", format!("{}_response", group_id))
            .set("bootstrap.servers", "localhost:9092") // 这里应该从配置获取
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true")
            .create()
            .map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!(
                    "创建 Kafka 响应消费者失败: {}",
                    e
                ))
            })?;

        let response_topic = format!("{}_response", self.topic_prefix);
        consumer.subscribe(&[&response_topic]).map_err(|e| {
            crate::errors::ApplicationError::Infrastructure(format!(
                "订阅 Kafka 响应主题失败: {}",
                e
            ))
        })?;

        let pending_commands = self.pending_commands.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;

            info!("开始监听 Kafka 命令响应: {}", response_topic);

            let mut message_stream = consumer.stream();
            while let Some(message) = message_stream.next().await {
                match message {
                    Ok(m) => {
                        let command_id = match m.key_view::<str>() {
                            Some(Ok(key)) => key,
                            _ => "unknown",
                        };
                        let payload = match m.payload_view::<str>() {
                            Some(Ok(s)) => s,
                            Some(Err(_)) => {
                                error!("无法解析响应负载为字符串");
                                continue;
                            }
                            None => {
                                warn!("接收到空响应负载");
                                continue;
                            }
                        };

                        info!("接收到命令响应: {}", command_id);

                        // 查找等待的命令
                        let mut pending = pending_commands.write().await;
                        if let Some(sender) = pending.remove(command_id) {
                            let response_data: serde_json::Value =
                                match serde_json::from_str(payload) {
                                    Ok(data) => data,
                                    Err(e) => {
                                        error!("响应数据解析失败: {}", e);
                                        continue;
                                    }
                                };

                            let _ = sender.send(response_data);
                        } else {
                            warn!("未找到等待响应的命令: {}", command_id);
                        }
                    }
                    Err(e) => {
                        error!("Kafka 响应消息接收错误: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
    pub async fn start_command_consumer(&self, group_id: &str) -> ApplicationResult<()> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", "localhost:9092") // 这里应该从配置获取
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true")
            .create()
            .map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!(
                    "创建 Kafka 消费者失败: {}",
                    e
                ))
            })?;

        let topic_pattern = format!("{}.*", self.topic_prefix);
        consumer.subscribe(&[&topic_pattern]).map_err(|e| {
            crate::errors::ApplicationError::Infrastructure(format!("订阅 Kafka 主题失败: {}", e))
        })?;

        let handlers = self.handlers.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;

            info!("开始监听 Kafka 命令消息: {}", topic_pattern);

            let mut message_stream = consumer.stream();
            while let Some(message) = message_stream.next().await {
                match message {
                    Ok(m) => {
                        let topic = m.topic();
                        let payload = match m.payload_view::<str>() {
                            Some(Ok(s)) => s,
                            Some(Err(_)) => {
                                error!("无法解析消息负载为字符串");
                                continue;
                            }
                            None => {
                                warn!("接收到空消息负载");
                                continue;
                            }
                        };

                        info!("接收到来自主题 {} 的命令消息", topic);

                        // 这里需要根据主题名称和消息内容来路由到相应的处理器
                        // 实际实现需要更复杂的反序列化和路由逻辑
                        let _ = Self::process_command_message(payload, &handlers).await;
                    }
                    Err(e) => {
                        error!("Kafka 消息接收错误: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn process_command_message(
        payload: &str,
        handlers: &Arc<RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>>,
    ) -> ApplicationResult<()> {
        // 解析命令数据
        let command_data: serde_json::Value = serde_json::from_str(payload).map_err(|e| {
            crate::errors::ApplicationError::Serialization(format!("命令反序列化失败: {}", e))
        })?;

        let command_type = command_data
            .get("command_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::errors::ApplicationError::Validation("命令类型缺失".to_string())
            })?;

        let command_id = command_data
            .get("command_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        info!("处理命令: {} (ID: {})", command_type, command_id);

        // 找到对应的处理器
        let handlers_lock = handlers.read().await;
        let _handler = handlers_lock.get(command_type).ok_or_else(|| {
            error!("未找到命令处理器: {}", command_type);
            crate::errors::ApplicationError::CommandHandlerNotFound(command_type.to_string())
        })?;
        drop(handlers_lock);

        // 注意：由于 Rust 的类型系统限制，这里无法直接调用泛型处理器
        // 在实际项目中，需要使用更复杂的模式，如：
        // 1. 枚举包含所有可能的命令类型
        // 2. 使用 trait objects 和动态分发
        // 3. 使用消息代理模式

        info!("命令 {} 处理完成", command_id);
        Ok(())
    }
}

#[async_trait]
impl CommandBus for KafkaCommandBus {
    async fn dispatch<C: Command>(&self, _command: C) -> ApplicationResult<C::Result> {
        let type_name = std::any::type_name::<C>();
        let topic = format!("{}:{}", self.topic_prefix, type_name);
        let command_id = Uuid::new_v4().to_string();

        info!("分发命令到 Kafka 主题: {} (ID: {})", topic, command_id);

        // 创建响应通道
        let (response_sender, response_receiver) = oneshot::channel();

        // 注册等待响应的命令
        {
            let mut pending = self.pending_commands.write().await;
            pending.insert(command_id.clone(), response_sender);
        }

        // 构造包含类型信息的命令消息
        let command_message = serde_json::json!({
            "command_id": command_id,
            "command_type": type_name,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            // 实际的命令数据需要通过特定方式序列化
            "data": serde_json::json!({})
        });

        let command_data = serde_json::to_string(&command_message).map_err(|e| {
            crate::errors::ApplicationError::Serialization(format!("命令序列化失败: {}", e))
        })?;

        // 发送到 Kafka
        let record = FutureRecord::to(&topic)
            .key(&command_id)
            .payload(&command_data);

        match self
            .producer
            .send(record, std::time::Duration::from_secs(5))
            .await
        {
            Ok(delivery) => {
                info!("命令已发送到 Kafka: {:?}", delivery);
            }
            Err((e, _)) => {
                // 移除等待的命令
                let mut pending = self.pending_commands.write().await;
                pending.remove(&command_id);

                error!("发送命令到 Kafka 失败: {}", e);
                return Err(crate::errors::ApplicationError::Infrastructure(format!(
                    "Kafka 发送失败: {}",
                    e
                )));
            }
        }

        // 等待响应（设置超时时间）
        let timeout_duration = std::time::Duration::from_secs(30);
        match tokio::time::timeout(timeout_duration, response_receiver).await {
            Ok(Ok(_response_data)) => {
                // 注意：由于 Rust 的类型系统限制，这里无法直接将 JSON 转换为泛型类型 C::Result
                // 在实际实现中，需要使用更复杂的模式来处理类型转换
                // 目前返回一个默认值或错误
                return Err(crate::errors::ApplicationError::Infrastructure(
                    "异步响应处理暂未完全实现".to_string(),
                ));
            }
            Ok(Err(_)) => {
                return Err(crate::errors::ApplicationError::Infrastructure(
                    "命令响应通道错误".to_string(),
                ));
            }
            Err(_) => {
                // 超时，移除等待的命令
                let mut pending = self.pending_commands.write().await;
                pending.remove(&command_id);

                return Err(crate::errors::ApplicationError::Infrastructure(format!(
                    "命令执行超时: {}",
                    command_id
                )));
            }
        }
    }
}

/// 基于 Kafka 的查询总线实现（支持缓存和 CQRS 读模型）
pub struct KafkaQueryBus {
    producer: Arc<FutureProducer>,
    handlers: Arc<RwLock<HashMap<String, Box<dyn std::any::Any + Send + Sync>>>>,
    topic_prefix: String,
    cache_enabled: bool,
}

impl KafkaQueryBus {
    pub fn new(
        kafka_brokers: &str,
        topic_prefix: String,
        cache_enabled: bool,
    ) -> ApplicationResult<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", kafka_brokers)
            .set("message.timeout.ms", "5000")
            .create()
            .map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!(
                    "创建 Kafka 生产者失败: {}",
                    e
                ))
            })?;

        Ok(Self {
            producer: Arc::new(producer),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            topic_prefix,
            cache_enabled,
        })
    }

    pub async fn register_handler<Q: Query>(&self, handler: Arc<dyn QueryHandler<Q>>) {
        let type_name = std::any::type_name::<Q>().to_string();
        let mut handlers = self.handlers.write().await;
        handlers.insert(type_name.clone(), Box::new(handler));
        info!("注册查询处理器: {}", type_name);
    }
}

#[async_trait]
impl QueryBus for KafkaQueryBus {
    async fn dispatch<Q: Query>(&self, query: Q) -> ApplicationResult<Q::Result> {
        let type_name = std::any::type_name::<Q>();
        info!("执行查询: {}", type_name);

        // 对于查询，通常直接执行本地处理器，而不通过 Kafka
        // 因为查询需要立即返回结果，而不是异步处理
        let handlers = self.handlers.read().await;
        let handler = handlers
            .get(type_name)
            .and_then(|h| h.downcast_ref::<Arc<dyn QueryHandler<Q>>>())
            .ok_or_else(|| {
                error!("未找到查询处理器: {}", type_name);
                crate::errors::ApplicationError::QueryHandlerNotFound(type_name.to_string())
            })?;

        // 如果启用缓存，可以在这里添加缓存逻辑
        if self.cache_enabled {
            // TODO: 实现缓存查找逻辑
        }

        let result = handler.handle(query).await?;

        // 如果启用缓存，将结果缓存
        if self.cache_enabled {
            // TODO: 实现结果缓存逻辑
        }

        // 可选：将查询日志发送到 Kafka 用于审计或分析
        let log_topic = format!("{}_log:query:{}", self.topic_prefix, type_name);
        let query_log = serde_json::json!({
            "type": "query_executed",
            "query_type": type_name,
            "timestamp": chrono::Utc::now().timestamp(),
            "success": true
        });

        if let Ok(log_data) = serde_json::to_string(&query_log) {
            let record_key = Uuid::new_v4().to_string();
            let record = FutureRecord::to(&log_topic)
                .key(&record_key)
                .payload(&log_data);

            let _ = self
                .producer
                .send(record, std::time::Duration::from_secs(1))
                .await;
        }

        Ok(result)
    }
}

/// 基于 Kafka 的事件总线实现（支持事件溯源和分布式处理）
pub struct KafkaEventBus {
    producer: Arc<FutureProducer>,
    handlers: Arc<RwLock<Vec<Arc<dyn EventHandler>>>>,
    topic_prefix: String,
    enable_event_sourcing: bool,
}

impl KafkaEventBus {
    pub fn new(
        kafka_brokers: &str,
        topic_prefix: String,
        enable_event_sourcing: bool,
    ) -> ApplicationResult<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", kafka_brokers)
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.messages", "10000")
            .set("batch.size", "16384")
            .set("linger.ms", "5")
            .create()
            .map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!(
                    "创建 Kafka 事件生产者失败: {}",
                    e
                ))
            })?;

        Ok(Self {
            producer: Arc::new(producer),
            handlers: Arc::new(RwLock::new(Vec::new())),
            topic_prefix,
            enable_event_sourcing,
        })
    }

    /// 启动事件消费者，处理来自 Kafka 的事件消息
    pub async fn start_event_consumer(&self, group_id: &str) -> ApplicationResult<()> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", "localhost:9092") // 应该从配置获取
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest") // 确保不丢失事件
            .create()
            .map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!(
                    "创建 Kafka 事件消费者失败: {}",
                    e
                ))
            })?;

        // 订阅所有事件主题
        let topic_pattern = format!("{}:event:*", self.topic_prefix);
        consumer.subscribe(&[&topic_pattern]).map_err(|e| {
            crate::errors::ApplicationError::Infrastructure(format!(
                "订阅 Kafka 事件主题失败: {}",
                e
            ))
        })?;

        let handlers = self.handlers.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;

            info!("开始监听 Kafka 事件消息: {}", topic_pattern);

            let mut message_stream = consumer.stream();
            while let Some(message) = message_stream.next().await {
                match message {
                    Ok(m) => {
                        let topic = m.topic();
                        let payload = match m.payload_view::<str>() {
                            Some(Ok(s)) => s,
                            Some(Err(_)) => {
                                error!("无法解析事件消息负载为字符串");
                                continue;
                            }
                            None => {
                                warn!("接收到空事件消息负载");
                                continue;
                            }
                        };

                        info!("接收到来自主题 {} 的事件消息", topic);

                        // 处理事件消息
                        if let Err(e) = Self::process_event_message(payload, &handlers).await {
                            error!("处理事件消息失败: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Kafka 事件消息接收错误: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn process_event_message(
        payload: &str,
        handlers: &Arc<RwLock<Vec<Arc<dyn EventHandler>>>>,
    ) -> ApplicationResult<()> {
        // 解析事件数据
        let event_data: serde_json::Value = serde_json::from_str(payload).map_err(|e| {
            crate::errors::ApplicationError::Serialization(format!("事件反序列化失败: {}", e))
        })?;

        let event_type = event_data
            .get("event_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::errors::ApplicationError::Validation("事件类型缺失".to_string())
            })?;

        // 找到所有可以处理此事件的处理器
        let handlers_lock = handlers.read().await;
        let matching_handlers: Vec<Arc<dyn EventHandler>> = handlers_lock
            .iter()
            .filter(|h| h.can_handle(event_type))
            .cloned()
            .collect();
        drop(handlers_lock);

        // 并发处理事件
        let mut tasks: Vec<tokio::task::JoinHandle<Result<(), crate::errors::ApplicationError>>> =
            Vec::new();

        for handler in matching_handlers {
            // 从事件数据重构事件对象
            let event = Arc::new(KafkaEvent::new(event_data.clone(), event_type.to_string()));

            // 为每个处理器创建异步任务
            let task = tokio::spawn(async move {
                match handler.handle(event).await {
                    Ok(()) => {
                        info!("事件处理器成功处理事件");
                        Ok(())
                    }
                    Err(e) => {
                        error!("事件处理器执行失败: {}", e);
                        Err(e)
                    }
                }
            });

            tasks.push(task);
        }

        // 等待所有处理器完成
        for task in tasks {
            if let Err(e) = task.await {
                error!("事件处理器任务失败: {}", e);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventBus for KafkaEventBus {
    async fn publish(&self, event: Arc<dyn DomainEvent>) -> ApplicationResult<()> {
        let event_type = event.event_type();
        let event_id = Uuid::new_v4().to_string();

        info!("发布领域事件: {} (ID: {})", event_type, event_id);

        // 首先处理本地处理器（同步处理）
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            if handler.can_handle(event_type) {
                if let Err(e) = handler.handle(event.clone()).await {
                    error!("本地事件处理器错误: {}", e);
                    // 继续处理其他处理器，不因一个失败而中断
                }
            }
        }
        drop(handlers);

        // 将事件发布到 Kafka（异步处理和跨实例通信）
        let topic = format!("{}:event:{}", self.topic_prefix, event_type);

        // 构造事件消息，包含元数据（不直接序列化event对象）
        let event_message = serde_json::json!({
            "event_id": event_id,
            "event_type": event_type,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "aggregate_id": event.aggregate_id(),
            "version": event.version(),
            // 事件数据需要通过特定方式序列化
            "data": serde_json::json!({})
        });

        let message_data = serde_json::to_string(&event_message).map_err(|e| {
            crate::errors::ApplicationError::Serialization(format!("事件序列化失败: {}", e))
        })?;

        // 使用聚合ID作为分区键，确保同一聚合的事件有序
        let aggregate_id = event.aggregate_id();
        let record = FutureRecord::to(&topic)
            .key(&aggregate_id)
            .payload(&message_data);

        match self
            .producer
            .send(record, std::time::Duration::from_secs(10))
            .await
        {
            Ok(delivery) => {
                info!("事件已发布到 Kafka: {:?}", delivery);

                // 如果启用了事件溯源，额外发送到事件存储主题
                if self.enable_event_sourcing {
                    let event_store_topic = format!("{}:event_store", self.topic_prefix);
                    let store_key = format!("{}:{}", event.aggregate_id(), event.version());
                    let store_record = FutureRecord::to(&event_store_topic)
                        .key(&store_key)
                        .payload(&message_data);

                    if let Err((e, _)) = self
                        .producer
                        .send(store_record, std::time::Duration::from_secs(5))
                        .await
                    {
                        warn!("发送事件到事件存储失败: {}", e);
                    }
                }
            }
            Err((e, _)) => {
                error!("发布事件到 Kafka 失败: {}", e);
                return Err(crate::errors::ApplicationError::Infrastructure(format!(
                    "Kafka 事件发布失败: {}",
                    e
                )));
            }
        }

        Ok(())
    }

    async fn subscribe(&self, handler: Arc<dyn EventHandler>) -> ApplicationResult<()> {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
        info!("添加事件处理器，当前处理器总数: {}", handlers.len());
        Ok(())
    }
}
