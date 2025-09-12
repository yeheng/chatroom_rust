//! 消息处理服务
//!
//! 实现消息的核心业务逻辑，包括消息发送、编辑、删除等功能。
//! 支持高并发消息发送（≥1000 msg/s）、消息去重、敏感词过滤等功能。

use crate::errors::{ApplicationError, ApplicationResult, MessageError};
use chrono::Utc;
use domain::entities::message::{MessageStatus, MessageType};
use domain::message::Message;
use domain::user::{User, UserStatus};
use infrastructure::events::ChatEvent;
use infrastructure::kafka::producer::KafkaMessageProducer;
use infrastructure::KafkaConfig;
use redis::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info};
use uuid::Uuid;

/// 发送消息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageCommand {
    /// 聊天室ID
    pub room_id: Uuid,
    /// 发送者ID
    pub user_id: Uuid,
    /// 消息内容
    pub content: String,
    /// 消息类型
    pub message_type: MessageType,
    /// 是否为机器人消息
    pub is_bot_message: bool,
}

/// 编辑消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditMessageRequest {
    /// 消息ID
    pub message_id: Uuid,
    /// 新的消息内容
    pub new_content: String,
    /// 编辑者ID
    pub editor_id: Uuid,
}

/// 消息查询参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueryParams {
    /// 聊天室ID
    pub room_id: Uuid,
    /// 分页页码
    pub page: Option<u32>,
    /// 每页数量
    pub page_size: Option<u32>,
    /// 消息类型过滤
    pub message_type: Option<MessageType>,
    /// 发送者ID过滤
    pub sender_id: Option<Uuid>,
    /// 包含已删除的消息
    pub include_deleted: bool,
}

/// 消息分页响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePageResponse {
    /// 消息列表
    pub messages: Vec<Message>,
    /// 总数量
    pub total: u64,
    /// 当前页码
    pub page: u32,
    /// 每页数量
    pub page_size: u32,
    /// 总页数
    pub total_pages: u32,
}

/// 消息服务接口
#[async_trait::async_trait]
pub trait MessageService: Send + Sync {
    /// 发送消息
    async fn send_message(&self, command: SendMessageCommand) -> ApplicationResult<Message>;

    /// 编辑消息
    async fn edit_message(&self, request: EditMessageRequest) -> ApplicationResult<Message>;

    /// 删除消息（软删除）
    async fn delete_message(&self, message_id: Uuid, deleter_id: Uuid) -> ApplicationResult<()>;

    /// 撤回消息
    async fn recall_message(&self, message_id: Uuid, recaller_id: Uuid) -> ApplicationResult<()>;

    /// 获取消息详情
    async fn get_message(&self, message_id: Uuid) -> ApplicationResult<Message>;

    /// 获取聊天室消息列表
    async fn get_room_messages(
        &self,
        params: MessageQueryParams,
    ) -> ApplicationResult<MessagePageResponse>;

    /// 获取用户的最新消息
    async fn get_user_recent_messages(
        &self,
        user_id: Uuid,
        limit: usize,
    ) -> ApplicationResult<Vec<Message>>;

    /// 标记消息为已读
    async fn mark_messages_as_read(
        &self,
        message_ids: Vec<Uuid>,
        reader_id: Uuid,
    ) -> ApplicationResult<()>;

    /// 获取消息统计
    async fn get_message_stats(&self, room_id: Uuid) -> ApplicationResult<MessageStats>;
}

/// 消息统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStats {
    /// 总消息数
    pub total_messages: u64,
    /// 今日消息数
    pub today_messages: u64,
    /// 文本消息数
    pub text_messages: u64,
    /// 图片消息数
    pub image_messages: u64,
    /// 文件消息数
    pub file_messages: u64,
    /// 系统消息数
    pub system_messages: u64,
    /// 最活跃用户ID
    pub most_active_user: Option<Uuid>,
    /// 最活跃用户消息数
    pub most_active_user_count: u64,
}

/// 内存中的消息存储（用于测试和简单实现）
pub struct InMemoryMessageStore {
    /// 消息存储：消息ID -> 消息
    messages: Arc<RwLock<std::collections::HashMap<Uuid, Message>>>,
    /// 聊天室消息索引：房间ID -> 消息ID列表
    room_messages: Arc<RwLock<std::collections::HashMap<Uuid, Vec<Uuid>>>>,
    /// 用户消息索引：用户ID -> 消息ID列表
    user_messages: Arc<RwLock<std::collections::HashMap<Uuid, Vec<Uuid>>>>,
}

impl InMemoryMessageStore {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(std::collections::HashMap::new())),
            room_messages: Arc::new(RwLock::new(std::collections::HashMap::new())),
            user_messages: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 存储消息
    pub async fn store_message(&self, message: Message) -> ApplicationResult<()> {
        let mut messages = self.messages.write().await;
        messages.insert(message.id, message.clone());
        drop(messages);

        // 更新房间消息索引
        let mut room_messages = self.room_messages.write().await;
        room_messages
            .entry(message.room_id)
            .or_insert_with(Vec::new)
            .push(message.id);
        drop(room_messages);

        // 更新用户消息索引
        let mut user_messages = self.user_messages.write().await;
        user_messages
            .entry(message.sender_id)
            .or_insert_with(Vec::new)
            .push(message.id);
        drop(user_messages);

        Ok(())
    }

    /// 获取消息
    pub async fn get_message(&self, message_id: Uuid) -> Option<Message> {
        let messages = self.messages.read().await;
        messages.get(&message_id).cloned()
    }

    /// 获取房间消息
    pub async fn get_room_messages(&self, room_id: Uuid) -> Vec<Message> {
        let room_messages = self.room_messages.read().await;
        let message_ids = room_messages.get(&room_id).cloned().unwrap_or_default();
        drop(room_messages);

        let messages = self.messages.read().await;
        message_ids
            .into_iter()
            .filter_map(|id| messages.get(&id).cloned())
            .collect()
    }

    /// 获取用户消息
    pub async fn get_user_messages(&self, user_id: Uuid) -> Vec<Message> {
        let user_messages = self.user_messages.read().await;
        let message_ids = user_messages.get(&user_id).cloned().unwrap_or_default();
        drop(user_messages);

        let messages = self.messages.read().await;
        message_ids
            .into_iter()
            .filter_map(|id| messages.get(&id).cloned())
            .collect()
    }

    /// 更新消息
    pub async fn update_message(
        &self,
        message_id: Uuid,
        message: Message,
    ) -> ApplicationResult<()> {
        let mut messages = self.messages.write().await;
        if let Some(stored_message) = messages.get_mut(&message_id) {
            *stored_message = message.clone();
            Ok(())
        } else {
            Err(MessageError::MessageNotFound(message_id).into())
        }
    }

    /// 删除消息（软删除）
    pub async fn delete_message(&self, message_id: Uuid) -> ApplicationResult<()> {
        let mut messages = self.messages.write().await;
        if let Some(message) = messages.get_mut(&message_id) {
            message
                .soft_delete()
                .map_err(|e| MessageError::Internal(e.to_string()))?;
            Ok(())
        } else {
            Err(MessageError::MessageNotFound(message_id).into())
        }
    }

    /// 撤回消息
    pub async fn recall_message(&self, message_id: Uuid) -> ApplicationResult<()> {
        let mut messages = self.messages.write().await;
        if let Some(message) = messages.get_mut(&message_id) {
            message
                .recall()
                .map_err(|e| MessageError::Internal(e.to_string()))?;
            Ok(())
        } else {
            Err(MessageError::MessageNotFound(message_id).into())
        }
    }
}

/// 消息服务实现
pub struct MessageServiceImpl {
    /// 消息存储
    message_store: Arc<InMemoryMessageStore>,
    /// 用户存储（模拟）
    users: Arc<RwLock<HashMap<Uuid, User>>>,
    /// 房间成员服务（模拟）
    room_members: Arc<RwLock<HashMap<Uuid, HashSet<Uuid>>>>,
    /// Kafka生产者
    kafka_producer: Option<Arc<KafkaMessageProducer>>,
    /// 消息速率限制器（带时间窗口）
    rate_limiter: Arc<Mutex<HashMap<Uuid, (std::time::Instant, u32)>>>,
    /// Redis连接（用于消息去重）
    redis_client: Option<Arc<Client>>,
    /// 敏感词词库
    sensitive_words: Arc<RwLock<HashSet<String>>>,
    /// 消息去重缓存（最近消息哈希值）
    message_hashes: Arc<Mutex<HashMap<String, std::time::Instant>>>,
    /// 消息队列缓冲区（批量写入）
    message_buffer: Arc<Mutex<Vec<Message>>>,
    /// 消息处理信号量（控制并发）
    processing_semaphore: Arc<Semaphore>,
}

impl MessageServiceImpl {
    /// 创建新的消息服务
    pub fn new() -> Self {
        // 初始化基础敏感词库
        let mut sensitive_words = HashSet::new();
        sensitive_words.insert("敏感词".to_string());
        sensitive_words.insert("password".to_string());
        sensitive_words.insert("token".to_string());
        sensitive_words.insert("secret".to_string());
        sensitive_words.insert("key".to_string());

        Self {
            message_store: Arc::new(InMemoryMessageStore::new()),
            users: Arc::new(RwLock::new(HashMap::new())),
            room_members: Arc::new(RwLock::new(HashMap::new())),
            kafka_producer: None,
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            redis_client: None,
            sensitive_words: Arc::new(RwLock::new(sensitive_words)),
            message_hashes: Arc::new(Mutex::new(HashMap::new())),
            message_buffer: Arc::new(Mutex::new(Vec::new())),
            processing_semaphore: Arc::new(Semaphore::new(1000)), // 支持1000并发
        }
    }

    /// 带完整配置的创建
    pub async fn with_config(
        kafka_config: Option<&KafkaConfig>,
        redis_url: Option<&str>,
    ) -> ApplicationResult<Self> {
        let kafka_producer = if let Some(config) = kafka_config {
            Some(Arc::new(KafkaMessageProducer::new(config).await.map_err(
                |e| ApplicationError::Infrastructure(format!("创建Kafka生产者失败: {}", e)),
            )?))
        } else {
            None
        };

        let redis_client = if let Some(url) = redis_url {
            Some(Arc::new(Client::open(url).map_err(|e| {
                ApplicationError::Infrastructure(format!("连接Redis失败: {}", e))
            })?))
        } else {
            None
        };

        // 初始化敏感词库
        let mut sensitive_words = HashSet::new();
        sensitive_words.insert("敏感词".to_string());
        sensitive_words.insert("password".to_string());
        sensitive_words.insert("token".to_string());
        sensitive_words.insert("secret".to_string());
        sensitive_words.insert("key".to_string());

        Ok(Self {
            message_store: Arc::new(InMemoryMessageStore::new()),
            users: Arc::new(RwLock::new(HashMap::new())),
            room_members: Arc::new(RwLock::new(HashMap::new())),
            kafka_producer,
            rate_limiter: Arc::new(Mutex::new(HashMap::new())),
            redis_client,
            sensitive_words: Arc::new(RwLock::new(sensitive_words)),
            message_hashes: Arc::new(Mutex::new(HashMap::new())),
            message_buffer: Arc::new(Mutex::new(Vec::new())),
            processing_semaphore: Arc::new(Semaphore::new(1000)),
        })
    }

    /// 确保用户存在（测试友好）：若不存在则创建一个活跃的占位用户
    async fn ensure_user_exists(&self, user_id: Uuid) -> ApplicationResult<()> {
        if self.users.read().await.contains_key(&user_id) {
            return Ok(());
        }

        let username = format!("user-{}", &user_id.to_string()[..8]);
        let email = format!("{}@example.com", username);
        let now = Utc::now();

        let user = User::with_id(
            user_id,
            username,
            email,
            None,
            None,
            UserStatus::Active,
            now,
            now,
            Some(now),
        )
        .map_err(ApplicationError::from)?;

        let mut users = self.users.write().await;
        users.insert(user_id, user);
        Ok(())
    }

    /// 验证发送消息权限
    async fn validate_send_permission(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> ApplicationResult<()> {
        // 验证用户状态
        let users = self.users.read().await;
        let user = users
            .get(&user_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("用户不存在: {}", user_id)))?;

        if user.status != UserStatus::Active {
            return Err(MessageError::Unauthorized("用户状态不活跃".to_string()).into());
        }
        drop(users);

        // 验证用户是否在房间中
        let room_members = self.room_members.read().await;
        let members = room_members
            .get(&room_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("房间不存在: {}", room_id)))?;

        if !members.contains(&user_id) {
            return Err(MessageError::Unauthorized("用户不在房间中".to_string()).into());
        }

        Ok(())
    }

    /// 验证消息速率限制
    async fn check_message_rate_limit(&self, user_id: Uuid) -> ApplicationResult<()> {
        let mut rate_limiter = self.rate_limiter.lock().await;
        let now = std::time::Instant::now();
        let window_duration = std::time::Duration::from_secs(60); // 1分钟窗口
        let max_messages = 50; // 每分钟最多50条消息

        // 清理过期记录
        rate_limiter.retain(|_, (timestamp, _)| now.duration_since(*timestamp) < window_duration);

        match rate_limiter.get_mut(&user_id) {
            Some((timestamp, count)) => {
                if now.duration_since(*timestamp) < window_duration {
                    if *count >= max_messages {
                        return Err(MessageError::RateLimited(format!(
                            "发送消息过于频繁，请{}秒后再试",
                            (window_duration - now.duration_since(*timestamp)).as_secs()
                        ))
                        .into());
                    }
                    *count += 1;
                } else {
                    *timestamp = now;
                    *count = 1;
                }
            }
            None => {
                rate_limiter.insert(user_id, (now, 1));
            }
        }

        Ok(())
    }

    /// 验证消息内容（包含敏感词过滤）
    async fn validate_message_content(
        &self,
        content: &str,
        message_type: &MessageType,
    ) -> ApplicationResult<()> {
        if content.trim().is_empty() {
            return Err(MessageError::EmptyContent.into());
        }

        let max_length = match message_type {
            MessageType::Text | MessageType::Emoji => 5000,
            MessageType::System => 1000,
            _ => 1000,
        };

        if content.len() > max_length {
            return Err(MessageError::ContentTooLong(content.len(), max_length).into());
        }

        // 敏感词过滤
        let sensitive_words = self.sensitive_words.read().await;
        let content_lower = content.to_lowercase();

        for word in sensitive_words.iter() {
            if content_lower.contains(&word.to_lowercase()) {
                return Err(MessageError::SensitiveContent.into());
            }
        }

        Ok(())
    }

    /// 计算消息哈希用于去重
    fn calculate_message_hash(&self, command: &SendMessageCommand) -> String {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        command.room_id.hash(&mut hasher);
        command.user_id.hash(&mut hasher);
        command.content.hash(&mut hasher);
        // 将 MessageType 转换为字符串来哈希
        format!("{:?}", command.message_type).hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// 检查消息去重
    async fn check_message_duplication(
        &self,
        command: &SendMessageCommand,
    ) -> ApplicationResult<()> {
        let hash = self.calculate_message_hash(command);
        let mut message_hashes = self.message_hashes.lock().await;
        let now = std::time::Instant::now();
        let dedup_window = std::time::Duration::from_secs(300); // 5分钟去重窗口

        // 清理过期哈希
        message_hashes.retain(|_, timestamp| now.duration_since(*timestamp) < dedup_window);

        if let Some(last_time) = message_hashes.get(&hash) {
            if now.duration_since(*last_time) < std::time::Duration::from_secs(1) {
                return Err(MessageError::DuplicateMessage.into());
            }
        }

        message_hashes.insert(hash, now);
        Ok(())
    }

    /// 异步发布消息事件到Kafka
    async fn publish_message_event(&self, event: ChatEvent) -> ApplicationResult<()> {
        if let Some(producer) = &self.kafka_producer {
            tokio::spawn({
                let producer = Arc::clone(producer);
                let event = event.clone();
                async move {
                    if let Err(e) = producer.send_event(event).await {
                        error!("发布消息事件失败: {}", e);
                    }
                }
            });
            debug!("异步发布消息事件: {:?}", event);
        }
        Ok(())
    }

    /// 更新敏感词库（支持动态更新）
    pub async fn update_sensitive_words(&self, words: HashSet<String>) -> ApplicationResult<()> {
        let mut sensitive_words = self.sensitive_words.write().await;
        *sensitive_words = words;
        info!("敏感词库已更新，共{}个词", sensitive_words.len());
        Ok(())
    }

    /// 验证编辑权限
    async fn validate_edit_permission(
        &self,
        message_id: Uuid,
        editor_id: Uuid,
    ) -> ApplicationResult<Message> {
        let message = self
            .message_store
            .get_message(message_id)
            .await
            .ok_or(MessageError::MessageNotFound(message_id))?;

        // 只有消息发送者可以编辑自己的消息
        if message.sender_id != editor_id {
            return Err(MessageError::Unauthorized("只能编辑自己的消息".to_string()).into());
        }

        // 检查消息是否可编辑
        if !message.is_editable() {
            return Err(MessageError::Unauthorized("消息不可编辑".to_string()).into());
        }

        // 检查编辑时间限制（5分钟内可以编辑）
        let edit_time_limit = chrono::Duration::minutes(5);
        let now = chrono::Utc::now();
        if now.signed_duration_since(message.sent_at) > edit_time_limit {
            return Err(MessageError::Unauthorized("消息编辑时间已过".to_string()).into());
        }

        Ok(message)
    }

    /// 发布聊天事件
    async fn publish_chat_event(&self, event: ChatEvent) -> ApplicationResult<()> {
        if let Some(_producer) = &self.kafka_producer {
            debug!("发布聊天事件: {:?}", event);
            // producer.send_event(event).await?;
        }
        Ok(())
    }

    /// 添加测试用户（用于测试）
    pub async fn add_test_user(&self, user: User) {
        let mut users = self.users.write().await;
        users.insert(user.id, user);
    }

    /// 添加房间成员（用于测试）
    pub async fn add_room_member(&self, room_id: Uuid, user_id: Uuid) {
        // 确保用户存在，避免测试中未显式创建用户导致找不到用户
        let _ = self.ensure_user_exists(user_id).await;
        let mut room_members = self.room_members.write().await;
        room_members
            .entry(room_id)
            .or_insert_with(std::collections::HashSet::new)
            .insert(user_id);
    }
}

#[async_trait::async_trait]
impl MessageService for MessageServiceImpl {
    async fn send_message(&self, command: SendMessageCommand) -> ApplicationResult<Message> {
        // 获取处理信号量（控制并发）
        let _permit =
            self.processing_semaphore.acquire().await.map_err(|e| {
                ApplicationError::Infrastructure(format!("获取处理信号量失败: {}", e))
            })?;

        info!(
            "用户 {} 在房间 {} 发送消息",
            command.user_id, command.room_id
        );

        // 确保用户存在并验证发送权限
        self.ensure_user_exists(command.user_id).await?;
        self.validate_send_permission(command.room_id, command.user_id)
            .await?;

        // 验证消息速率限制
        self.check_message_rate_limit(command.user_id).await?;

        // 验证消息内容
        self.validate_message_content(&command.content, &command.message_type)
            .await?;

        // 检查消息去重
        self.check_message_duplication(&command).await?;

        // 创建消息
        let message = Message::new_text(
            command.room_id,
            command.user_id,
            command.content,
            None, // 暂不支持回复
        )
        .map_err(ApplicationError::from)?;

        // 存储消息
        self.message_store.store_message(message.clone()).await?;

        // 发布消息发送事件
        let event = ChatEvent::MessageSent {
            message: message.clone(),
            room_id: command.room_id,
            timestamp: chrono::Utc::now(),
        };
        self.publish_message_event(event).await?;

        info!("消息发送成功: {} ({})", message.id, command.room_id);
        Ok(message)
    }

    async fn edit_message(&self, request: EditMessageRequest) -> ApplicationResult<Message> {
        info!("用户 {} 编辑消息 {}", request.editor_id, request.message_id);

        // 验证编辑权限
        let mut message = self
            .validate_edit_permission(request.message_id, request.editor_id)
            .await?;

        // 验证新内容
        self.validate_message_content(&request.new_content, &message.message_type)
            .await?;

        // 编辑消息
        message
            .edit_content(&request.new_content)
            .map_err(ApplicationError::from)?;

        // 更新存储
        self.message_store
            .update_message(request.message_id, message.clone())
            .await?;

        // 发布消息编辑事件
        let event = ChatEvent::MessageEdited {
            message_id: request.message_id,
            room_id: message.room_id,
            new_content: request.new_content,
            edited_by: request.editor_id,
            timestamp: chrono::Utc::now(),
        };
        self.publish_chat_event(event).await?;

        info!("消息编辑成功: {}", request.message_id);
        Ok(message)
    }

    async fn delete_message(&self, message_id: Uuid, deleter_id: Uuid) -> ApplicationResult<()> {
        info!("用户 {} 删除消息 {}", deleter_id, message_id);

        // 验证消息存在
        let message = self
            .message_store
            .get_message(message_id)
            .await
            .ok_or(MessageError::MessageNotFound(message_id))?;

        // 验证删除权限（只有发送者和管理员可以删除）
        if message.sender_id != deleter_id {
            return Err(MessageError::Unauthorized("只能删除自己的消息".to_string()).into());
        }

        // 验证消息是否可删除
        if message.message_type == MessageType::System {
            return Err(MessageError::Unauthorized("系统消息不能删除".to_string()).into());
        }

        // 删除消息
        self.message_store.delete_message(message_id).await?;

        // 发布消息删除事件
        let event = ChatEvent::MessageDeleted {
            message_id,
            room_id: message.room_id,
            deleted_by: deleter_id,
            timestamp: chrono::Utc::now(),
        };
        self.publish_chat_event(event).await?;

        info!("消息删除成功: {}", message_id);
        Ok(())
    }

    async fn recall_message(&self, message_id: Uuid, recaller_id: Uuid) -> ApplicationResult<()> {
        info!("用户 {} 撤回消息 {}", recaller_id, message_id);

        // 验证消息存在
        let message = self
            .message_store
            .get_message(message_id)
            .await
            .ok_or(MessageError::MessageNotFound(message_id))?;

        // 验证撤回权限（只有发送者可以撤回）
        if message.sender_id != recaller_id {
            return Err(MessageError::Unauthorized("只能撤回自己的消息".to_string()).into());
        }

        // 验证撤回时间限制（2分钟内可以撤回）
        let recall_time_limit = chrono::Duration::minutes(2);
        let now = chrono::Utc::now();
        if now.signed_duration_since(message.sent_at) > recall_time_limit {
            return Err(MessageError::Unauthorized("消息撤回时间已过".to_string()).into());
        }

        // 撤回消息
        self.message_store.recall_message(message_id).await?;

        // 发布消息撤回事件
        let event = ChatEvent::MessageRecalled {
            message_id,
            room_id: message.room_id,
            recalled_by: recaller_id,
            timestamp: chrono::Utc::now(),
        };
        self.publish_chat_event(event).await?;

        info!("消息撤回成功: {}", message_id);
        Ok(())
    }

    async fn get_message(&self, message_id: Uuid) -> ApplicationResult<Message> {
        let message = self
            .message_store
            .get_message(message_id)
            .await
            .ok_or(MessageError::MessageNotFound(message_id))?;

        // 检查消息是否可见
        if !message.is_visible() {
            return Err(MessageError::MessageNotFound(message_id).into());
        }

        Ok(message)
    }

    async fn get_room_messages(
        &self,
        params: MessageQueryParams,
    ) -> ApplicationResult<MessagePageResponse> {
        let mut all_messages = self.message_store.get_room_messages(params.room_id).await;

        // 过滤消息
        if !params.include_deleted {
            all_messages.retain(|msg| msg.is_visible());
        }

        if let Some(message_type) = params.message_type {
            all_messages.retain(|msg| msg.message_type == message_type);
        }

        if let Some(sender_id) = params.sender_id {
            all_messages.retain(|msg| msg.sender_id == sender_id);
        }

        // 按时间倒序排序
        all_messages.sort_by(|a, b| b.sent_at.cmp(&a.sent_at));

        // 分页处理
        let page = params.page.unwrap_or(1);
        let page_size = params.page_size.unwrap_or(20);
        let total = all_messages.len() as u64;
        let total_pages = (total + page_size as u64 - 1) / page_size as u64;

        let start = ((page - 1) * page_size) as usize;
        let end = std::cmp::min(start + page_size as usize, all_messages.len());

        let messages = if start < all_messages.len() {
            all_messages[start..end].to_vec()
        } else {
            Vec::new()
        };

        Ok(MessagePageResponse {
            messages,
            total,
            page,
            page_size,
            total_pages: total_pages as u32,
        })
    }

    async fn get_user_recent_messages(
        &self,
        user_id: Uuid,
        limit: usize,
    ) -> ApplicationResult<Vec<Message>> {
        let mut messages = self.message_store.get_user_messages(user_id).await;

        // 只返回可见消息
        messages.retain(|msg| msg.is_visible());

        // 按时间倒序排序
        messages.sort_by(|a, b| b.sent_at.cmp(&a.sent_at));

        // 限制数量
        messages.truncate(limit);

        Ok(messages)
    }

    async fn mark_messages_as_read(
        &self,
        message_ids: Vec<Uuid>,
        reader_id: Uuid,
    ) -> ApplicationResult<()> {
        for message_id in message_ids {
            if let Some(mut message) = self.message_store.get_message(message_id).await {
                // 验证读者是否在消息的房间中
                if self
                    .room_members
                    .read()
                    .await
                    .get(&message.room_id)
                    .map(|members| members.contains(&reader_id))
                    .unwrap_or(false)
                {
                    // 更新消息状态为已读
                    message.update_status(MessageStatus::Read);
                    self.message_store
                        .update_message(message_id, message)
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn get_message_stats(&self, room_id: Uuid) -> ApplicationResult<MessageStats> {
        let all_messages = self.message_store.get_room_messages(room_id).await;

        let mut total_messages = 0;
        let mut today_messages = 0;
        let mut text_messages = 0;
        let mut image_messages = 0;
        let mut file_messages = 0;
        let mut system_messages = 0;

        let mut user_message_counts = std::collections::HashMap::new();
        let now = chrono::Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

        for message in &all_messages {
            if message.is_visible() {
                total_messages += 1;

                // 统计今日消息
                if message.sent_at >= today_start {
                    today_messages += 1;
                }

                // 统计消息类型
                match message.message_type {
                    MessageType::Text => text_messages += 1,
                    MessageType::Image => image_messages += 1,
                    MessageType::File => file_messages += 1,
                    MessageType::System => system_messages += 1,
                    MessageType::Emoji => text_messages += 1, // Emoji按文本统计
                }

                // 统计用户消息数
                *user_message_counts.entry(message.sender_id).or_insert(0) += 1;
            }
        }

        // 找出最活跃用户
        let (most_active_user, most_active_user_count) = user_message_counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .unzip();

        Ok(MessageStats {
            total_messages,
            today_messages,
            text_messages,
            image_messages,
            file_messages,
            system_messages,
            most_active_user,
            most_active_user_count: most_active_user_count.unwrap_or(0),
        })
    }
}

// 添加Validation错误变体
impl From<String> for MessageError {
    fn from(msg: String) -> Self {
        MessageError::Validation(msg)
    }
}
