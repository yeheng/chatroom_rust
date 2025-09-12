# 基础设施层设计

基础设施层提供技术支持，实现领域层定义的接口。本层负责数据库访问、Kafka消息队列、Redis缓存、WebSocket管理等技术组件的具体实现。

## 🏗️ 基础设施层架构

### 核心组件

```rust
// 数据库连接池
pub struct DatabasePool {
    pool: sqlx::PgPool,
}

// Kafka生产者
pub struct KafkaProducer {
    producer: rdkafka::producer::FutureProducer<rdkafka::config::ClientConfig>,
    topic: String,
}

// Kafka消费者
pub struct KafkaConsumer {
    consumer: rdkafka::consumer::StreamConsumer<rdkafka::config::ClientConfig>,
    topics: Vec<String>,
}

// Redis客户端
pub struct RedisClient {
    client: redis::Client,
    connection_pool: Arc<redis::aio::ConnectionManager>,
}

// WebSocket管理器
pub struct WebSocketManager {
    connections: Arc<RwLock<HashMap<Uuid, WebSocketConnection>>>,
    room_connections: Arc<RwLock<HashMap<Uuid, HashSet<Uuid>>>>,
    redis_client: Arc<RedisClient>,
}
```

## 📊 Kafka消息队列架构

### Kafka配置

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct KafkaConfig {
    pub bootstrap_servers: String,
    pub group_id: String,
    pub topics: KafkaTopics,
    pub producer_config: ProducerConfig,
    pub consumer_config: ConsumerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KafkaTopics {
    pub chat_events: String,
    pub user_events: String,
    pub system_events: String,
    pub notification_events: String,
    pub search_events: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProducerConfig {
    pub acks: String,
    pub retries: i32,
    pub max_in_flight_requests_per_connection: i32,
    pub request_timeout_ms: i32,
    pub enable_idempotence: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConsumerConfig {
    pub auto_offset_reset: String,
    pub enable_auto_commit: bool,
    pub auto_commit_interval_ms: i32,
    pub session_timeout_ms: i32,
    pub heartbeat_interval_ms: i32,
}

impl Default for KafkaConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            group_id: "chatroom-backend".to_string(),
            topics: KafkaTopics {
                chat_events: "chat-events".to_string(),
                user_events: "user-events".to_string(),
                system_events: "system-events".to_string(),
                notification_events: "notification-events".to_string(),
                search_events: "search-events".to_string(),
            },
            producer_config: ProducerConfig {
                acks: "all".to_string(),
                retries: 3,
                max_in_flight_requests_per_connection: 5,
                request_timeout_ms: 30000,
                enable_idempotence: true,
            },
            consumer_config: ConsumerConfig {
                auto_offset_reset: "earliest".to_string(),
                enable_auto_commit: false,
                auto_commit_interval_ms: 1000,
                session_timeout_ms: 30000,
                heartbeat_interval_ms: 3000,
            },
        }
    }
}
```

### Kafka生产者实现

```rust
pub struct ChatEventProducer {
    producer: Arc<rdkafka::producer::FutureProducer<rdkafka::config::ClientConfig>>,
    topic: String,
}

impl ChatEventProducer {
    pub fn new(config: &KafkaConfig) -> Result<Self> {
        let mut producer_config = rdkafka::config::ClientConfig::new();
        
        producer_config.set("bootstrap.servers", &config.bootstrap_servers);
        producer_config.set("acks", &config.producer_config.acks);
        producer_config.set("retries", &config.producer_config.retries.to_string());
        producer_config.set("max.in.flight.requests.per.connection", 
            &config.producer_config.max_in_flight_requests_per_connection.to_string());
        producer_config.set("request.timeout.ms", 
            &config.producer_config.request_timeout_ms.to_string());
        producer_config.set("enable.idempotence", 
            &config.producer_config.enable_idempotence.to_string());
        
        let producer: rdkafka::producer::FutureProducer<rdkafka::config::ClientConfig> = 
            producer_config.create()?;
        
        Ok(Self {
            producer: Arc::new(producer),
            topic: config.topics.chat_events.clone(),
        })
    }
    
    pub async fn send_chat_event(&self, event: ChatEvent) -> Result<()> {
        let payload = serde_json::to_string(&event)?;
        let key = event.get_key();
        
        let delivery_future = self.producer.send(
            rdkafka::producer::FutureRecord::to(&self.topic)
                .key(&key)
                .payload(&payload),
            0, // partition: 0 (let Kafka decide)
        );
        
        match delivery_future.await {
            Ok(delivery) => match delivery {
                Ok(_) => Ok(()),
                Err((e, _)) => Err(InfrastructureError::KafkaError(e)),
            },
            Err(e) => Err(InfrastructureError::KafkaError(e)),
        }
    }
    
    pub async fn send_user_event(&self, event: UserEvent) -> Result<()> {
        let payload = serde_json::to_string(&event)?;
        let key = event.get_key();
        
        let delivery_future = self.producer.send(
            rdkafka::producer::FutureRecord::to(&config.topics.user_events)
                .key(&key)
                .payload(&payload),
            0,
        );
        
        match delivery_future.await {
            Ok(delivery) => match delivery {
                Ok(_) => Ok(()),
                Err((e, _)) => Err(InfrastructureError::KafkaError(e)),
            },
            Err(e) => Err(InfrastructureError::KafkaError(e)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatEvent {
    MessageSent {
        message: Message,
        room_id: Uuid,
    },
    MessageUpdated {
        message_id: Uuid,
        room_id: Uuid,
        new_content: String,
        updated_by: Uuid,
    },
    MessageDeleted {
        message_id: Uuid,
        room_id: Uuid,
        deleted_by: Uuid,
    },
    UserJoinedRoom {
        user_id: Uuid,
        room_id: Uuid,
        joined_at: DateTime<Utc>,
    },
    UserLeftRoom {
        user_id: Uuid,
        room_id: Uuid,
        left_at: DateTime<Utc>,
    },
    RoomCreated {
        room_id: Uuid,
        name: String,
        owner_id: Uuid,
        is_private: bool,
        created_at: DateTime<Utc>,
    },
    RoomUpdated {
        room_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        updated_by: Uuid,
        updated_at: DateTime<Utc>,
    },
    RoomDeleted {
        room_id: Uuid,
        deleted_by: Uuid,
        deleted_at: DateTime<Utc>,
    },
}

impl ChatEvent {
    pub fn get_key(&self) -> String {
        match self {
            ChatEvent::MessageSent { room_id, .. } => room_id.to_string(),
            ChatEvent::MessageUpdated { room_id, .. } => room_id.to_string(),
            ChatEvent::MessageDeleted { room_id, .. } => room_id.to_string(),
            ChatEvent::UserJoinedRoom { room_id, .. } => room_id.to_string(),
            ChatEvent::UserLeftRoom { room_id, .. } => room_id.to_string(),
            ChatEvent::RoomCreated { room_id, .. } => room_id.to_string(),
            ChatEvent::RoomUpdated { room_id, .. } => room_id.to_string(),
            ChatEvent::RoomDeleted { room_id, .. } => room_id.to_string(),
        }
    }
}
```

### Kafka消费者实现

```rust
pub struct ChatEventConsumer {
    consumer: Arc<rdkafka::consumer::StreamConsumer<rdkafka::config::ClientConfig>>,
    websocket_manager: Arc<dyn WebSocketManager>,
    message_repository: Arc<dyn MessageRepository>,
    notification_service: Arc<dyn NotificationService>,
    search_service: Arc<dyn SearchService>,
}

impl ChatEventConsumer {
    pub fn new(config: &KafkaConfig, websocket_manager: Arc<dyn WebSocketManager>) -> Result<Self> {
        let mut consumer_config = rdkafka::config::ClientConfig::new();
        
        consumer_config.set("bootstrap.servers", &config.bootstrap_servers);
        consumer_config.set("group.id", &config.group_id);
        consumer_config.set("auto.offset.reset", &config.consumer_config.auto_offset_reset);
        consumer_config.set("enable.auto.commit", 
            &config.consumer_config.enable_auto_commit.to_string());
        consumer_config.set("auto.commit.interval.ms", 
            &config.consumer_config.auto_commit_interval_ms.to_string());
        consumer_config.set("session.timeout.ms", 
            &config.consumer_config.session_timeout_ms.to_string());
        consumer_config.set("heartbeat.interval.ms", 
            &config.consumer_config.heartbeat_interval_ms.to_string());
        
        let consumer: rdkafka::consumer::StreamConsumer<rdkafka::config::ClientConfig> = 
            consumer_config.create()?;
        
        Ok(Self {
            consumer: Arc::new(consumer),
            websocket_manager,
            message_repository: Arc::new(InMemoryMessageRepository::new()),
            notification_service: Arc::new(InMemoryNotificationService::new()),
            search_service: Arc::new(InMemorySearchService::new()),
        })
    }
    
    pub async fn start(&self) -> Result<()> {
        self.consumer.subscribe(&[&self.topic])?;
        
        let mut stream = self.consumer.stream();
        
        while let Some(message_result) = stream.next().await {
            match message_result {
                Ok(message) => {
                    if let Some(payload) = message.payload() {
                        match self.process_message(payload).await {
                            Ok(_) => {
                                // 手动提交偏移量
                                self.consumer.commit_message(&message, rdkafka::consumer::CommitMode::Sync)?;
                            }
                            Err(e) => {
                                tracing::error!("Failed to process message: {:?}", e);
                                // 根据业务逻辑决定是否重试或进入死信队列
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Kafka consumer error: {:?}", e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn process_message(&self, payload: &[u8]) -> Result<()> {
        let event: ChatEvent = serde_json::from_slice(payload)?;
        
        match event {
            ChatEvent::MessageSent { message, room_id } => {
                // 实时推送到WebSocket
                self.websocket_manager.broadcast_to_room(room_id, message.clone()).await?;
                
                // 发送通知
                self.notification_service.notify_room_members(room_id, &message).await?;
                
                // 索引到搜索引擎
                self.search_service.index_message(&message).await?;
            }
            ChatEvent::MessageUpdated { message_id, room_id, new_content, updated_by } => {
                // 更新消息
                if let Some(mut message) = self.message_repository.find_by_id(message_id).await? {
                    message.content = new_content;
                    message.updated_at = Utc::now();
                    self.message_repository.save(&message).await?;
                    
                    // 推送到WebSocket
                    self.websocket_manager.broadcast_to_room(room_id, message.clone()).await?;
                }
            }
            ChatEvent::MessageDeleted { message_id, room_id, deleted_by } => {
                // 从数据库删除
                self.message_repository.delete(message_id).await?;
                
                // 通知房间成员
                self.websocket_manager.notify_message_deleted(room_id, message_id).await?;
            }
            ChatEvent::UserJoinedRoom { user_id, room_id, joined_at } => {
                // 通知房间其他成员
                self.websocket_manager.notify_user_joined_room(room_id, user_id).await?;
            }
            ChatEvent::UserLeftRoom { user_id, room_id, left_at } => {
                // 通知房间其他成员
                self.websocket_manager.notify_user_left_room(room_id, user_id).await?;
            }
            ChatEvent::RoomCreated { room_id, name, owner_id, is_private, created_at } => {
                // 通知系统有新房间创建
                self.websocket_manager.broadcast_system_event(format!("Room '{}' created", name)).await?;
            }
            ChatEvent::RoomUpdated { room_id, name, description, updated_by, updated_at } => {
                // 通知房间成员房间信息已更新
                self.websocket_manager.notify_room_updated(room_id).await?;
            }
            ChatEvent::RoomDeleted { room_id, deleted_by, deleted_at } => {
                // 通知房间成员房间已删除
                self.websocket_manager.notify_room_deleted(room_id).await?;
            }
        }
        
        Ok(())
    }
}
```

## 🚀 Redis Pub/Sub跨实例通信

### Redis配置

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout_seconds: u64,
    pub cluster_mode: bool,
    pub cluster_nodes: Vec<String>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            connection_timeout_seconds: 30,
            cluster_mode: false,
            cluster_nodes: vec![],
        }
    }
}
```

### Redis客户端实现

```rust
pub struct RedisClient {
    client: redis::Client,
    connection_pool: Arc<redis::aio::ConnectionManager>,
}

impl RedisClient {
    pub async fn new(config: &RedisConfig) -> Result<Self> {
        let client = redis::Client::open(config.url.clone())?;
        let connection_pool = Arc::new(
            redis::aio::ConnectionManager::new(client.clone()).await?
        );
        
        Ok(Self {
            client,
            connection_pool,
        })
    }
    
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.connection_pool.get().await?;
        let value: Option<String> = conn.get(key).await?;
        Ok(value)
    }
    
    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.connection_pool.get().await?;
        conn.set(key, value).await?;
        Ok(())
    }
    
    pub async fn setex(&self, key: &str, value: &str, seconds: usize) -> Result<()> {
        let mut conn = self.connection_pool.get().await?;
        conn.setex(key, seconds, value).await?;
        Ok(())
    }
    
    pub async fn del(&self, key: &str) -> Result<()> {
        let mut conn = self.connection_pool.get().await?;
        conn.del(key).await?;
        Ok(())
    }
    
    pub async fn hget(&self, key: &str, field: &str) -> Result<Option<String>> {
        let mut conn = self.connection_pool.get().await?;
        let value: Option<String> = conn.hget(key, field).await?;
        Ok(value)
    }
    
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> Result<()> {
        let mut conn = self.connection_pool.get().await?;
        conn.hset(key, field, value).await?;
        Ok(())
    }
    
    pub async fn hgetall(&self, key: &str) -> Result<HashMap<String, String>> {
        let mut conn = self.connection_pool.get().await?;
        let values: HashMap<String, String> = conn.hgetall(key).await?;
        Ok(values)
    }
    
    pub async fn publish(&self, channel: &str, message: &str) -> Result<u32> {
        let mut conn = self.connection_pool.get().await?;
        let subscribers: u32 = conn.publish(channel, message).await?;
        Ok(subscribers)
    }
    
    pub async fn subscribe(&self, channels: &[String]) -> Result<redis::aio::PubSub> {
        let mut conn = self.connection_pool.get().await?;
        let pubsub = conn.subscribe(channels).await?;
        Ok(pubsub)
    }
    
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection_pool.get().await?;
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }
    
    pub async fn expire(&self, key: &str, seconds: usize) -> Result<()> {
        let mut conn = self.connection_pool.get().await?;
        conn.expire(key, seconds).await?;
        Ok(())
    }
    
    pub async fn sadd(&self, key: &str, member: &str) -> Result<u32> {
        let mut conn = self.connection_pool.get().await?;
        let count: u32 = conn.sadd(key, member).await?;
        Ok(count)
    }
    
    pub async fn srem(&self, key: &str, member: &str) -> Result<u32> {
        let mut conn = self.connection_pool.get().await?;
        let count: u32 = conn.srem(key, member).await?;
        Ok(count)
    }
    
    pub async fn smembers(&self, key: &str) -> Result<HashSet<String>> {
        let mut conn = self.connection_pool.get().await?;
        let members: HashSet<String> = conn.smembers(key).await?;
        Ok(members)
    }
}
```

### Pub/Sub消息处理

```rust
pub struct RedisPubSubHandler {
    redis_client: Arc<RedisClient>,
    websocket_manager: Arc<dyn WebSocketManager>,
    event_bus: Arc<dyn EventBus>,
}

impl RedisPubSubHandler {
    pub fn new(redis_client: Arc<RedisClient>, websocket_manager: Arc<dyn WebSocketManager>) -> Self {
        Self {
            redis_client,
            websocket_manager,
            event_bus: Arc::new(InMemoryEventBus::new()),
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        let channels = vec![
            "chatroom:messages".to_string(),
            "chatroom:presence".to_string(),
            "chatroom:notifications".to_string(),
            "chatroom:system".to_string(),
        ];
        
        let mut pubsub = self.redis_client.subscribe(&channels).await?;
        
        loop {
            let msg = pubsub.on_message().await;
            match self.handle_pubsub_message(msg).await {
                Ok(_) => continue,
                Err(e) => {
                    tracing::error!("Failed to handle pubsub message: {:?}", e);
                }
            }
        }
    }
    
    async fn handle_pubsub_message(&self, msg: redis::aio::Msg) -> Result<()> {
        let channel = msg.get_channel_name();
        let payload = msg.get_payload::<String>();
        
        match channel {
            "chatroom:messages" => {
                if let Ok(message_json) = payload {
                    let message: Message = serde_json::from_str(&message_json)?;
                    self.websocket_manager.broadcast_to_room(message.room_id, message).await?;
                }
            }
            "chatroom:presence" => {
                if let Ok(presence_json) = payload {
                    let presence: UserPresence = serde_json::from_str(&presence_json)?;
                    self.handle_presence_update(presence).await?;
                }
            }
            "chatroom:notifications" => {
                if let Ok(notification_json) = payload {
                    let notification: Notification = serde_json::from_str(&notification_json)?;
                    self.handle_notification(notification).await?;
                }
            }
            "chatroom:system" => {
                if let Ok(system_json) = payload {
                    let system_event: SystemEvent = serde_json::from_str(&system_json)?;
                    self.handle_system_event(system_event).await?;
                }
            }
            _ => {
                tracing::warn!("Unknown pubsub channel: {}", channel);
            }
        }
        
        Ok(())
    }
    
    async fn handle_presence_update(&self, presence: UserPresence) -> Result<()> {
        // 更新用户在线状态
        let key = format!("user:presence:{}", presence.user_id);
        let presence_json = serde_json::to_string(&presence)?;
        self.redis_client.setex(&key, &presence_json, 300).await?; // 5分钟过期
        
        // 通知相关房间
        self.websocket_manager.broadcast_user_status(presence.user_id, presence.status).await?;
        
        Ok(())
    }
    
    async fn handle_notification(&self, notification: Notification) -> Result<()> {
        // 发送通知到指定用户
        self.websocket_manager.send_notification(notification.user_id, notification).await?;
        
        Ok(())
    }
    
    async fn handle_system_event(&self, event: SystemEvent) -> Result<()> {
        // 广播系统事件到所有连接的客户端
        self.websocket_manager.broadcast_system_event(event.message).await?;
        
        Ok(())
    }
    
    pub async fn publish_message(&self, room_id: Uuid, message: Message) -> Result<()> {
        let channel = format!("chatroom:messages:{}", room_id);
        let message_json = serde_json::to_string(&message)?;
        self.redis_client.publish(&channel, &message_json).await?;
        Ok(())
    }
    
    pub async fn publish_presence(&self, presence: UserPresence) -> Result<()> {
        let channel = "chatroom:presence";
        let presence_json = serde_json::to_string(&presence)?;
        self.redis_client.publish(channel, &presence_json).await?;
        Ok(())
    }
    
    pub async fn publish_notification(&self, notification: Notification) -> Result<()> {
        let channel = format!("chatroom:notifications:{}", notification.user_id);
        let notification_json = serde_json::to_string(&notification)?;
        self.redis_client.publish(&channel, &notification_json).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPresence {
    pub user_id: Uuid,
    pub status: UserStatus,
    pub last_seen: DateTime<Utc>,
    pub current_room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub message: String,
    pub notification_type: NotificationType,
    pub created_at: DateTime<Utc>,
    pub read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationType {
    NewMessage,
    UserJoinedRoom,
    UserLeftRoom,
    RoomInvitation,
    SystemAlert,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    pub id: Uuid,
    pub event_type: SystemEventType,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEventType {
    Maintenance,
    SystemUpdate,
    SecurityAlert,
    FeatureUpdate,
}
```

## 🔌 WebSocket连接管理

### WebSocket配置

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct WebSocketConfig {
    pub max_connections: usize,
    pub heartbeat_interval_seconds: u64,
    pub message_size_limit: usize,
    pub connection_timeout_seconds: u64,
    pub enable_compression: bool,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_connections: 10000,
            heartbeat_interval_seconds: 30,
            message_size_limit: 65536, // 64KB
            connection_timeout_seconds: 60,
            enable_compression: true,
        }
    }
}
```

### WebSocket连接实现

```rust
pub struct WebSocketConnection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub sender: SplitSink<WebSocket, Message>,
    pub receiver: SplitStream<WebSocket>,
    pub connected_at: DateTime<Utc>,
    pub last_ping: DateTime<Utc>,
    pub current_rooms: HashSet<Uuid>,
}

pub struct WebSocketManager {
    connections: Arc<RwLock<HashMap<Uuid, WebSocketConnection>>>,
    room_connections: Arc<RwLock<HashMap<Uuid, HashSet<Uuid>>>>,
    redis_client: Arc<RedisClient>,
    config: WebSocketConfig,
}

impl WebSocketManager {
    pub fn new(redis_client: Arc<RedisClient>, config: WebSocketConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            room_connections: Arc::new(RwLock::new(HashMap::new())),
            redis_client,
            config,
        }
    }
    
    pub async fn handle_connection(&self, websocket: WebSocket, user_id: Uuid) -> Result<()> {
        let connection_id = Uuid::new_v4();
        let (sender, receiver) = websocket.split();
        
        let connection = WebSocketConnection {
            id: connection_id,
            user_id,
            sender,
            receiver,
            connected_at: Utc::now(),
            last_ping: Utc::now(),
            current_rooms: HashSet::new(),
        };
        
        // 添加连接管理
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id, connection);
        }
        
        // 更新用户在线状态
        self.update_user_presence(user_id, UserStatus::Online).await?;
        
        // 启动消息处理任务
        let manager = Arc::new(self);
        let connection_id_clone = connection_id;
        let manager_clone = manager.clone();
        
        tokio::spawn(async move {
            if let Err(e) = manager_clone.handle_connection_messages(connection_id_clone).await {
                tracing::error!("Error handling connection {}: {:?}", connection_id_clone, e);
            }
        });
        
        // 启动心跳任务
        let manager_clone = manager.clone();
        let connection_id_clone = connection_id;
        tokio::spawn(async move {
            if let Err(e) = manager_clone.handle_heartbeat(connection_id_clone).await {
                tracing::error!("Error handling heartbeat for {}: {:?}", connection_id_clone, e);
            }
        });
        
        Ok(())
    }
    
    async fn handle_connection_messages(&self, connection_id: Uuid) -> Result<()> {
        let mut receiver = {
            let mut connections = self.connections.write().await;
            connections.get_mut(&connection_id)
                .ok_or(InfrastructureError::ConnectionNotFound)?
                .receiver
                .take()
                .ok_or(InfrastructureError::ReceiverAlreadyTaken)?
        };
        
        while let Some(message_result) = receiver.next().await {
            match message_result {
                Ok(message) => {
                    self.handle_websocket_message(connection_id, message).await?;
                }
                Err(e) => {
                    tracing::warn!("WebSocket error for connection {}: {:?}", connection_id, e);
                    break;
                }
            }
        }
        
        // 连接关闭，清理资源
        self.remove_connection(connection_id).await?;
        Ok(())
    }
    
    async fn handle_websocket_message(&self, connection_id: Uuid, message: Message) -> Result<()> {
        match message {
            Message::Text(text) => {
                let client_message: ClientMessage = serde_json::from_str(&text)?;
                self.handle_client_message(connection_id, client_message).await?;
            }
            Message::Binary(data) => {
                // 处理二进制消息（如文件传输）
                tracing::warn!("Binary message received from connection {}", connection_id);
            }
            Message::Ping(data) => {
                self.handle_ping(connection_id, data).await?;
            }
            Message::Pong(data) => {
                self.handle_pong(connection_id, data).await?;
            }
            Message::Close(_) => {
                self.remove_connection(connection_id).await?;
            }
            Message::Frame(_) => {
                // 原始帧，通常不需要处理
            }
        }
        
        Ok(())
    }
    
    async fn handle_client_message(&self, connection_id: Uuid, message: ClientMessage) -> Result<()> {
        let user_id = {
            let connections = self.connections.read().await;
            connections.get(&connection_id)
                .ok_or(InfrastructureError::ConnectionNotFound)?
                .user_id
        };
        
        match message {
            ClientMessage::SendMessage { room_id, content, message_type, reply_to_message_id } => {
                // 创建消息并发布到Kafka
                let command = SendMessageCommand {
                    room_id,
                    user_id,
                    content,
                    message_type,
                    reply_to_message_id,
                };
                
                // 这里应该通过命令总线处理
                // self.command_bus.dispatch(command).await?;
                
                // 暂时直接发布到Redis
                let message = Message::new(
                    Uuid::new_v4(),
                    room_id,
                    user_id,
                    content,
                    message_type,
                    reply_to_message_id,
                )?;
                
                self.publish_message_to_room(room_id, message).await?;
            }
            ClientMessage::JoinRoom { room_id, password } => {
                // 加入房间
                let command = JoinChatRoomCommand {
                    room_id,
                    user_id,
                    password,
                };
                
                // 这里应该通过命令总线处理
                // self.command_bus.dispatch(command).await?;
                
                // 添加到房间连接
                self.add_connection_to_room(connection_id, room_id).await?;
                
                // 发布加入事件
                let event = ChatEvent::UserJoinedRoom {
                    user_id,
                    room_id,
                    joined_at: Utc::now(),
                };
                
                self.publish_chat_event(event).await?;
            }
            ClientMessage::LeaveRoom { room_id } => {
                // 离开房间
                let command = LeaveChatRoomCommand {
                    room_id,
                    user_id,
                };
                
                // 这里应该通过命令总线处理
                // self.command_bus.dispatch(command).await?;
                
                // 从房间连接移除
                self.remove_connection_from_room(connection_id, room_id).await?;
                
                // 发布离开事件
                let event = ChatEvent::UserLeftRoom {
                    user_id,
                    room_id,
                    left_at: Utc::now(),
                };
                
                self.publish_chat_event(event).await?;
            }
            ClientMessage::Heartbeat => {
                // 更新心跳时间
                self.update_connection_heartbeat(connection_id).await?;
            }
            ClientMessage::Ping => {
                // 回复Pong
                self.send_pong(connection_id).await?;
            }
        }
        
        Ok(())
    }
    
    async fn broadcast_to_room(&self, room_id: Uuid, message: Message) -> Result<()> {
        let message_json = serde_json::to_string(&message)?;
        let server_message = ServerMessage::Message(message);
        let message_text = serde_json::to_string(&server_message)?;
        
        // 发布到Redis Pub/Sub，让其他实例也能收到
        self.redis_client.publish(&format!("chatroom:messages:{}", room_id), &message_json).await?;
        
        // 发送给当前实例的连接
        let connection_ids = {
            let room_connections = self.room_connections.read().await;
            room_connections.get(&room_id)
                .map(|connections| connections.clone())
                .unwrap_or_default()
        };
        
        let connections = self.connections.read().await;
        for connection_id in connection_ids {
            if let Some(connection) = connections.get(&connection_id) {
                if let Err(e) = self.send_message_to_connection(connection, &message_text).await {
                    tracing::error!("Failed to send message to connection {}: {:?}", connection_id, e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn send_message_to_connection(&self, connection: &WebSocketConnection, message: &str) -> Result<()> {
        let mut sender = connection.sender.clone();
        sender.send(Message::Text(message.to_string())).await?;
        Ok(())
    }
    
    async fn add_connection_to_room(&self, connection_id: Uuid, room_id: Uuid) -> Result<()> {
        // 添加到房间连接映射
        {
            let mut room_connections = self.room_connections.write().await;
            room_connections.entry(room_id)
                .or_insert_with(HashSet::new)
                .insert(connection_id);
        }
        
        // 更新连接的当前房间
        {
            let mut connections = self.connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.current_rooms.insert(room_id);
            }
        }
        
        Ok(())
    }
    
    async fn remove_connection_from_room(&self, connection_id: Uuid, room_id: Uuid) -> Result<()> {
        // 从房间连接映射移除
        {
            let mut room_connections = self.room_connections.write().await;
            if let Some(connections) = room_connections.get_mut(&room_id) {
                connections.remove(&connection_id);
                if connections.is_empty() {
                    room_connections.remove(&room_id);
                }
            }
        }
        
        // 更新连接的当前房间
        {
            let mut connections = self.connections.write().await;
            if let Some(connection) = connections.get_mut(&connection_id) {
                connection.current_rooms.remove(&room_id);
            }
        }
        
        Ok(())
    }
    
    async fn remove_connection(&self, connection_id: Uuid) -> Result<()> {
        let user_id = {
            let mut connections = self.connections.write().await;
            connections.remove(&connection_id)
                .map(|conn| {
                    // 从所有房间移除连接
                    for room_id in &conn.current_rooms {
                        drop(connections);
                        let _ = self.remove_connection_from_room(connection_id, *room_id).await;
                        connections = self.connections.write().await;
                    }
                    conn.user_id
                })
        };
        
        if let Some(user_id) = user_id {
            // 更新用户离线状态
            self.update_user_presence(user_id, UserStatus::Offline).await?;
        }
        
        Ok(())
    }
    
    async fn handle_heartbeat(&self, connection_id: Uuid) -> Result<()> {
        let interval = Duration::from_secs(self.config.heartbeat_interval_seconds);
        let mut ticker = tokio::time::interval(interval);
        
        loop {
            ticker.tick().await;
            
            // 检查连接是否存在
            let exists = {
                let connections = self.connections.read().await;
                connections.contains_key(&connection_id)
            };
            
            if !exists {
                break;
            }
            
            // 检查最后心跳时间
            let should_disconnect = {
                let connections = self.connections.read().await;
                if let Some(connection) = connections.get(&connection_id) {
                    let timeout_duration = Duration::from_secs(self.config.connection_timeout_seconds);
                    Utc::now().signed_duration_since(connection.last_ping) > timeout_duration
                } else {
                    true
                }
            };
            
            if should_disconnect {
                tracing::warn!("Connection {} heartbeat timeout, disconnecting", connection_id);
                self.remove_connection(connection_id).await?;
                break;
            }
            
            // 发送心跳
            self.send_ping(connection_id).await?;
        }
        
        Ok(())
    }
    
    async fn send_ping(&self, connection_id: Uuid) -> Result<()> {
        let message = ServerMessage::Ping;
        let message_text = serde_json::to_string(&message)?;
        
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(&connection_id) {
            self.send_message_to_connection(connection, &message_text).await?;
            connection.last_ping = Utc::now();
        }
        
        Ok(())
    }
    
    async fn send_pong(&self, connection_id: Uuid) -> Result<()> {
        let message = ServerMessage::Pong;
        let message_text = serde_json::to_string(&message)?;
        
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&connection_id) {
            self.send_message_to_connection(connection, &message_text).await?;
        }
        
        Ok(())
    }
    
    async fn update_connection_heartbeat(&self, connection_id: Uuid) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(&connection_id) {
            connection.last_ping = Utc::now();
        }
        
        Ok(())
    }
    
    async fn update_user_presence(&self, user_id: Uuid, status: UserStatus) -> Result<()> {
        let presence = UserPresence {
            user_id,
            status,
            last_seen: Utc::now(),
            current_room_id: None,
        };
        
        self.redis_client.publish("chatroom:presence", &serde_json::to_string(&presence)?).await?;
        
        Ok(())
    }
    
    async fn publish_message_to_room(&self, room_id: Uuid, message: Message) -> Result<()> {
        let message_json = serde_json::to_string(&message)?;
        self.redis_client.publish(&format!("chatroom:messages:{}", room_id), &message_json).await?;
        Ok(())
    }
    
    async fn publish_chat_event(&self, event: ChatEvent) -> Result<()> {
        let event_json = serde_json::to_string(&event)?;
        self.redis_client.publish("chatroom:events", &event_json).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    SendMessage {
        room_id: Uuid,
        content: String,
        message_type: MessageType,
        reply_to_message_id: Option<Uuid>,
    },
    JoinRoom {
        room_id: Uuid,
        password: Option<String>,
    },
    LeaveRoom {
        room_id: Uuid,
    },
    Heartbeat,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    Message(Message),
    UserJoined {
        user_id: Uuid,
        username: String,
        room_id: Uuid,
    },
    UserLeft {
        user_id: Uuid,
        username: String,
        room_id: Uuid,
    },
    RoomUpdated {
        room_id: Uuid,
        name: String,
        description: Option<String>,
    },
    Notification(Notification),
    SystemEvent(SystemEvent),
    Ping,
    Pong,
    Error {
        code: String,
        message: String,
    },
}
```

## 🗄️ 数据持久化实现

### PostgreSQL配置

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub max_lifetime_seconds: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost:5432/chatroom".to_string(),
            max_connections: 10,
            min_connections: 2,
            connect_timeout_seconds: 30,
            idle_timeout_seconds: 600,
            max_lifetime_seconds: 3600,
        }
    }
}
```

### 数据库连接池

```rust
pub struct DatabasePool {
    pool: sqlx::PgPool,
}

impl DatabasePool {
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .connect_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .idle_timeout(Duration::from_secs(config.idle_timeout_seconds))
            .max_lifetime(Duration::from_secs(config.max_lifetime_seconds))
            .connect(&config.url)
            .await?;
        
        Ok(Self { pool })
    }
    
    pub fn get_pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
    
    pub async fn execute(&self, query: &str, params: sqlx::postgres::PgArguments) -> Result<u64> {
        let result = sqlx::query_with(query, params)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
    
    pub async fn fetch_one<T>(&self, query: &str, params: sqlx::postgres::PgArguments) -> Result<T>
    where
        T: sqlx::FromRow<sqlx::postgres::PgRow> + Send + Unpin,
    {
        let result = sqlx::query_with(query, params)
            .fetch_one(&self.pool)
            .await?;
        Ok(result)
    }
    
    pub async fn fetch_optional<T>(&self, query: &str, params: sqlx::postgres::PgArguments) -> Result<Option<T>>
    where
        T: sqlx::FromRow<sqlx::postgres::PgRow> + Send + Unpin,
    {
        let result = sqlx::query_with(query, params)
            .fetch_optional(&self.pool)
            .await?;
        Ok(result)
    }
    
    pub async fn fetch_all<T>(&self, query: &str, params: sqlx::postgres::PgArguments) -> Result<Vec<T>>
    where
        T: sqlx::FromRow<sqlx::postgres::PgRow> + Send + Unpin,
    {
        let result = sqlx::query_with(query, params)
            .fetch_all(&self.pool)
            .await?;
        Ok(result)
    }
    
    pub async fn begin_transaction(&self) -> Result<sqlx::Transaction<sqlx::Postgres>> {
        let transaction = self.pool.begin().await?;
        Ok(transaction)
    }
}
```

### 仓储实现

```rust
pub struct PostgresUserRepository {
    pool: Arc<DatabasePool>,
}

impl PostgresUserRepository {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn save(&self, user: &User) -> Result<User> {
        let query = r#"
            INSERT INTO users (id, username, email, avatar_url, status, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                username = EXCLUDED.username,
                email = EXCLUDED.email,
                avatar_url = EXCLUDED.avatar_url,
                status = EXCLUDED.status,
                password_hash = EXCLUDED.password_hash,
                updated_at = EXCLUDED.updated_at
            RETURNING id, username, email, avatar_url, status, password_hash, created_at, updated_at
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(user.id)
            .bind(&user.username)
            .bind(&user.email)
            .bind(&user.avatar_url)
            .bind(&user.status)
            .bind(&user.password_hash)
            .bind(user.created_at)
            .bind(user.updated_at);
        
        let saved_user = self.pool.fetch_one(query, params).await?;
        Ok(saved_user)
    }
    
    async fn find_by_id(&self, user_id: Uuid) -> Result<Option<User>> {
        let query = r#"
            SELECT id, username, email, avatar_url, status, password_hash, created_at, updated_at
            FROM users 
            WHERE id = $1
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(user_id);
        
        let user = self.pool.fetch_optional(query, params).await?;
        Ok(user)
    }
    
    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let query = r#"
            SELECT id, username, email, avatar_url, status, password_hash, created_at, updated_at
            FROM users 
            WHERE username = $1
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(username);
        
        let user = self.pool.fetch_optional(query, params).await?;
        Ok(user)
    }
    
    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let query = r#"
            SELECT id, username, email, avatar_url, status, password_hash, created_at, updated_at
            FROM users 
            WHERE email = $1
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(email);
        
        let user = self.pool.fetch_optional(query, params).await?;
        Ok(user)
    }
    
    async fn delete(&self, user_id: Uuid) -> Result<()> {
        let query = "DELETE FROM users WHERE id = $1";
        let params = sqlx::postgres::PgArguments::new()
            .bind(user_id);
        
        self.pool.execute(query, params).await?;
        Ok(())
    }
    
    async fn find_by_ids(&self, user_ids: &[Uuid]) -> Result<Vec<User>> {
        let query = r#"
            SELECT id, username, email, avatar_url, status, password_hash, created_at, updated_at
            FROM users 
            WHERE id = ANY($1)
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(user_ids);
        
        let users = self.pool.fetch_all(query, params).await?;
        Ok(users)
    }
    
    async fn find_by_room_id(&self, room_id: Uuid, limit: u32, offset: u32) -> Result<Vec<User>> {
        let query = r#"
            SELECT u.id, u.username, u.email, u.avatar_url, u.status, u.password_hash, u.created_at, u.updated_at
            FROM users u
            JOIN room_members rm ON u.id = rm.user_id
            WHERE rm.room_id = $1
            ORDER BY rm.joined_at
            LIMIT $2 OFFSET $3
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(room_id)
            .bind(limit as i64)
            .bind(offset as i64);
        
        let users = self.pool.fetch_all(query, params).await?;
        Ok(users)
    }
    
    async fn search_users(&self, keyword: &str, limit: u32, offset: u32) -> Result<Vec<User>> {
        let query = r#"
            SELECT id, username, email, avatar_url, status, password_hash, created_at, updated_at
            FROM users 
            WHERE username ILIKE $1 OR email ILIKE $1
            ORDER BY username
            LIMIT $2 OFFSET $3
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(format!("%{}%", keyword))
            .bind(limit as i64)
            .bind(offset as i64);
        
        let users = self.pool.fetch_all(query, params).await?;
        Ok(users)
    }
    
    async fn count_by_room_id(&self, room_id: Uuid) -> Result<u32> {
        let query = r#"
            SELECT COUNT(*) as count
            FROM room_members rm
            JOIN users u ON rm.user_id = u.id
            WHERE rm.room_id = $1 AND u.status = 'active'
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(room_id);
        
        let count: (i64,) = self.pool.fetch_one(query, params).await?;
        Ok(count.0 as u32)
    }
    
    async fn update_status(&self, user_id: Uuid, status: UserStatus) -> Result<()> {
        let query = r#"
            UPDATE users 
            SET status = $1, updated_at = NOW()
            WHERE id = $2
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(&status)
            .bind(user_id);
        
        self.pool.execute(query, params).await?;
        Ok(())
    }
    
    async fn update_last_active(&self, user_id: Uuid) -> Result<()> {
        let query = r#"
            UPDATE users 
            SET last_active_at = NOW(), updated_at = NOW()
            WHERE id = $1
        "#;
        
        let params = sqlx::postgres::PgArguments::new()
            .bind(user_id);
        
        self.pool.execute(query, params).await?;
        Ok(())
    }
}
```

## 🔧 基础设施层容器

```rust
pub struct InfrastructureContainer {
    pub database_pool: Arc<DatabasePool>,
    pub redis_client: Arc<RedisClient>,
    pub kafka_producer: Arc<ChatEventProducer>,
    pub kafka_consumer: Arc<ChatEventConsumer>,
    pub websocket_manager: Arc<WebSocketManager>,
    pub user_repository: Arc<dyn UserRepository>,
    pub room_repository: Arc<dyn ChatRoomRepository>,
    pub message_repository: Arc<dyn MessageRepository>,
    pub organization_repository: Arc<dyn OrganizationRepository>,
    pub role_repository: Arc<dyn RoleRepository>,
    pub department_repository: Arc<dyn DepartmentRepository>,
    pub user_role_repository: Arc<dyn UserRoleRepository>,
    pub position_repository: Arc<dyn PositionRepository>,
    pub user_proxy_repository: Arc<dyn UserProxyRepository>,
    pub online_time_repository: Arc<dyn OnlineTimeRepository>,
}

impl InfrastructureContainer {
    pub async fn new(
        database_config: DatabaseConfig,
        redis_config: RedisConfig,
        kafka_config: KafkaConfig,
        websocket_config: WebSocketConfig,
    ) -> Result<Self> {
        // 初始化数据库连接池
        let database_pool = Arc::new(DatabasePool::new(&database_config).await?);
        
        // 初始化Redis客户端
        let redis_client = Arc::new(RedisClient::new(&redis_config).await?);
        
        // 初始化Kafka生产者
        let kafka_producer = Arc::new(ChatEventProducer::new(&kafka_config)?);
        
        // 初始化WebSocket管理器
        let websocket_manager = Arc::new(WebSocketManager::new(redis_client.clone(), websocket_config));
        
        // 初始化Kafka消费者
        let kafka_consumer = Arc::new(ChatEventConsumer::new(&kafka_config, websocket_manager.clone())?);
        
        // 初始化仓储实现
        let user_repository: Arc<dyn UserRepository> = Arc::new(PostgresUserRepository::new(database_pool.clone()));
        let room_repository: Arc<dyn ChatRoomRepository> = Arc::new(PostgresChatRoomRepository::new(database_pool.clone()));
        let message_repository: Arc<dyn MessageRepository> = Arc::new(PostgresMessageRepository::new(database_pool.clone()));
        let organization_repository: Arc<dyn OrganizationRepository> = Arc::new(PostgresOrganizationRepository::new(database_pool.clone()));
        let role_repository: Arc<dyn RoleRepository> = Arc::new(PostgresRoleRepository::new(database_pool.clone()));
        let department_repository: Arc<dyn DepartmentRepository> = Arc::new(PostgresDepartmentRepository::new(database_pool.clone()));
        let user_role_repository: Arc<dyn UserRoleRepository> = Arc::new(PostgresUserRoleRepository::new(database_pool.clone()));
        let position_repository: Arc<dyn PositionRepository> = Arc::new(PostgresPositionRepository::new(database_pool.clone()));
        let user_proxy_repository: Arc<dyn UserProxyRepository> = Arc::new(PostgresUserProxyRepository::new(database_pool.clone()));
        let online_time_repository: Arc<dyn OnlineTimeRepository> = Arc::new(PostgresOnlineTimeRepository::new(database_pool.clone()));
        
        Ok(Self {
            database_pool,
            redis_client,
            kafka_producer,
            kafka_consumer,
            websocket_manager,
            user_repository,
            room_repository,
            message_repository,
            organization_repository,
            role_repository,
            department_repository,
            user_role_repository,
            position_repository,
            user_proxy_repository,
            online_time_repository,
        })
    }
    
    pub async fn start_background_services(&self) -> Result<()> {
        // 启动Kafka消费者
        let kafka_consumer = self.kafka_consumer.clone();
        tokio::spawn(async move {
            if let Err(e) = kafka_consumer.start().await {
                tracing::error!("Kafka consumer error: {:?}", e);
            }
        });
        
        // 启动Redis Pub/Sub处理器
        let redis_pubsub_handler = RedisPubSubHandler::new(
            self.redis_client.clone(),
            self.websocket_manager.clone(),
        );
        
        tokio::spawn(async move {
            if let Err(e) = redis_pubsub_handler.start().await {
                tracing::error!("Redis Pub/Sub handler error: {:?}", e);
            }
        });
        
        Ok(())
    }
}
```

---

**下一步**: 阅读[05-web-api-layer-design.md](./05-web-api-layer-design.md)了解Web API层的详细设计。
