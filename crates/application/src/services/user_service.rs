//! 用户管理服务
//!
//! 实现用户的核心业务逻辑，包括用户创建、状态管理、搜索、扩展信息管理等功能。
//! 支持用户状态查询的批量操作，头像上传验证，用户搜索分页等功能。

use crate::errors::{ApplicationResult, UserError};
use domain::user::{User, UserStatus};
use infrastructure::events::ChatEvent;
use infrastructure::kafka::producer::KafkaMessageProducer;
use infrastructure::KafkaConfig;
use redis::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use uuid::Uuid;

/// 创建用户请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    /// 用户名
    pub username: String,
    /// 电子邮箱
    pub email: String,
    /// 密码（明文，将被哈希处理）
    pub password: String,
    /// 头像URL（可选）
    pub avatar_url: Option<String>,
    /// 显示名称（可选）
    pub display_name: Option<String>,
}

/// 更新用户请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    /// 新的用户名（可选）
    pub username: Option<String>,
    /// 新的邮箱（可选）
    pub email: Option<String>,
    /// 新的显示名称（可选）
    pub display_name: Option<String>,
    /// 新的头像URL（可选）
    pub avatar_url: Option<String>,
}

/// 用户搜索请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSearchRequest {
    /// 搜索查询字符串
    pub query: String,
    /// 页码（从1开始）
    pub page: u32,
    /// 每页大小
    pub page_size: u32,
    /// 状态过滤（可选）
    pub status_filter: Option<UserStatus>,
}

/// 用户搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSearchResponse {
    /// 用户列表
    pub users: Vec<User>,
    /// 总数量
    pub total: u64,
    /// 当前页码
    pub page: u32,
    /// 每页大小
    pub page_size: u32,
    /// 总页数
    pub total_pages: u32,
}

/// 用户统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    /// 总用户数
    pub total_users: u64,
    /// 活跃用户数
    pub active_users: u64,
    /// 在线用户数
    pub online_users: u64,
    /// 忙碌用户数
    pub busy_users: u64,
    /// 离开用户数
    pub away_users: u64,
    /// 今日新增用户数
    pub today_new_users: u64,
}

/// 用户服务接口
#[async_trait::async_trait]
pub trait UserService: Send + Sync {
    /// 创建用户
    async fn create_user(&self, request: CreateUserRequest) -> ApplicationResult<User>;

    /// 根据ID获取用户
    async fn get_user_by_id(&self, user_id: Uuid) -> ApplicationResult<User>;

    /// 根据用户名获取用户
    async fn get_user_by_username(&self, username: &str) -> ApplicationResult<User>;

    /// 根据邮箱获取用户
    async fn get_user_by_email(&self, email: &str) -> ApplicationResult<User>;

    /// 更新用户信息
    async fn update_user(
        &self,
        user_id: Uuid,
        request: UpdateUserRequest,
    ) -> ApplicationResult<User>;

    /// 更新用户状态
    async fn update_user_status(&self, user_id: Uuid, status: UserStatus) -> ApplicationResult<()>;

    /// 批量获取用户状态
    async fn get_users_status(
        &self,
        user_ids: &[Uuid],
    ) -> ApplicationResult<HashMap<Uuid, UserStatus>>;

    /// 搜索用户
    async fn search_users(
        &self,
        request: UserSearchRequest,
    ) -> ApplicationResult<UserSearchResponse>;

    /// 获取用户扩展信息
    async fn get_user_extensions(&self, user_id: Uuid) -> ApplicationResult<JsonValue>;

    /// 更新用户扩展信息
    async fn update_user_extensions(
        &self,
        user_id: Uuid,
        extensions: JsonValue,
    ) -> ApplicationResult<()>;

    /// 删除用户（软删除）
    async fn delete_user(&self, user_id: Uuid) -> ApplicationResult<()>;

    /// 获取用户统计信息
    async fn get_user_stats(&self) -> ApplicationResult<UserStats>;

    /// 验证用户凭据
    async fn verify_credentials(
        &self,
        username_or_email: &str,
        password: &str,
    ) -> ApplicationResult<User>;

    /// 更新用户最后活跃时间
    async fn update_last_activity(&self, user_id: Uuid) -> ApplicationResult<()>;
}

/// 内存中的用户存储（用于测试和简单实现）
pub struct InMemoryUserStore {
    /// 用户存储：用户ID -> 用户信息
    users: Arc<RwLock<HashMap<Uuid, User>>>,
    /// 用户名索引：用户名 -> 用户ID
    username_index: Arc<RwLock<HashMap<String, Uuid>>>,
    /// 邮箱索引：邮箱 -> 用户ID
    email_index: Arc<RwLock<HashMap<String, Uuid>>>,
    /// 用户扩展信息：用户ID -> JSON值
    user_extensions: Arc<RwLock<HashMap<Uuid, JsonValue>>>,
}

impl Default for InMemoryUserStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryUserStore {
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            username_index: Arc::new(RwLock::new(HashMap::new())),
            email_index: Arc::new(RwLock::new(HashMap::new())),
            user_extensions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 存储用户
    pub async fn store_user(&self, user: User) -> ApplicationResult<()> {
        let mut users = self.users.write().await;
        let mut username_index = self.username_index.write().await;
        let mut email_index = self.email_index.write().await;

        // 检查用户名冲突
        if username_index.contains_key(&user.username) {
            return Err(UserError::UsernameConflict(user.username.clone()).into());
        }

        // 检查邮箱冲突
        if email_index.contains_key(&user.email) {
            return Err(UserError::EmailConflict(user.email.clone()).into());
        }

        // 存储用户和索引
        username_index.insert(user.username.clone(), user.id);
        email_index.insert(user.email.clone(), user.id);
        users.insert(user.id, user);

        Ok(())
    }

    /// 获取用户
    pub async fn get_user(&self, user_id: Uuid) -> Option<User> {
        let users = self.users.read().await;
        users.get(&user_id).cloned()
    }

    /// 根据用户名获取用户
    pub async fn get_user_by_username(&self, username: &str) -> Option<User> {
        let username_index = self.username_index.read().await;
        let user_id = username_index.get(username).copied();
        drop(username_index);

        if let Some(user_id) = user_id {
            let users = self.users.read().await;
            users.get(&user_id).cloned()
        } else {
            None
        }
    }

    /// 根据邮箱获取用户
    pub async fn get_user_by_email(&self, email: &str) -> Option<User> {
        let email_index = self.email_index.read().await;
        let user_id = email_index.get(email).copied();
        drop(email_index);

        if let Some(user_id) = user_id {
            let users = self.users.read().await;
            users.get(&user_id).cloned()
        } else {
            None
        }
    }

    /// 更新用户
    pub async fn update_user(&self, user_id: Uuid, user: User) -> ApplicationResult<()> {
        let mut users = self.users.write().await;
        if let Some(stored_user) = users.get_mut(&user_id) {
            *stored_user = user;
            Ok(())
        } else {
            Err(UserError::UserNotFound(user_id).into())
        }
    }

    /// 删除用户
    pub async fn delete_user(&self, user_id: Uuid) -> ApplicationResult<()> {
        let mut users = self.users.write().await;

        if let Some(user) = users.get_mut(&user_id) {
            // 软删除
            user.status = UserStatus::Inactive;
            user.updated_at = chrono::Utc::now();
            Ok(())
        } else {
            Err(UserError::UserNotFound(user_id).into())
        }
    }

    /// 搜索用户
    pub async fn search_users(&self, query: &str, status_filter: Option<UserStatus>) -> Vec<User> {
        let users = self.users.read().await;
        let query_lower = query.to_lowercase();

        users
            .values()
            .filter(|user| {
                // 状态过滤
                if let Some(ref status) = status_filter {
                    if user.status != *status {
                        return false;
                    }
                }

                // 文本搜索（用户名、邮箱本地部分、显示名称）
                // 仅匹配邮箱的本地部分，避免因域名（如 example.com）导致干扰
                {
                    let username_match = user.username.to_lowercase().contains(&query_lower);
                    let local_part = user.email.split('@').next().unwrap_or("").to_lowercase();
                    let email_local_match = local_part.contains(&query_lower);
                    let display_name_match = user
                        .display_name
                        .as_ref()
                        .map(|name| name.to_lowercase().contains(&query_lower))
                        .unwrap_or(false);
                    username_match || email_local_match || display_name_match
                }
            })
            .cloned()
            .collect()
    }

    /// 获取所有用户
    pub async fn get_all_users(&self) -> Vec<User> {
        let users = self.users.read().await;
        users.values().cloned().collect()
    }
}

/// 用户服务实现
pub struct UserServiceImpl {
    /// 用户存储
    user_store: Arc<InMemoryUserStore>,
    /// 密码哈希缓存（用于性能优化）
    password_cache: Arc<RwLock<HashMap<Uuid, String>>>,
    /// Kafka生产者
    kafka_producer: Option<Arc<KafkaMessageProducer>>,
    /// Redis连接（用于状态缓存）
    redis_client: Option<Arc<Client>>,
    /// 用户状态缓存（内存缓存用于快速访问）
    status_cache: Arc<RwLock<HashMap<Uuid, (UserStatus, Instant)>>>,
    /// 搜索缓存（缓存搜索结果）
    search_cache: Arc<RwLock<HashMap<String, (Vec<User>, Instant)>>>,
}

impl Default for UserServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl UserServiceImpl {
    /// 创建新的用户服务
    pub fn new() -> Self {
        Self {
            user_store: Arc::new(InMemoryUserStore::new()),
            password_cache: Arc::new(RwLock::new(HashMap::new())),
            kafka_producer: None,
            redis_client: None,
            status_cache: Arc::new(RwLock::new(HashMap::new())),
            search_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 带完整配置的创建
    pub async fn with_config(
        kafka_config: Option<&KafkaConfig>,
        redis_url: Option<&str>,
    ) -> ApplicationResult<Self> {
        let kafka_producer = if let Some(config) = kafka_config {
            Some(Arc::new(KafkaMessageProducer::new(config).await.map_err(
                |e| {
                    crate::errors::ApplicationError::Infrastructure(format!(
                        "创建Kafka生产者失败: {}",
                        e
                    ))
                },
            )?))
        } else {
            None
        };

        let redis_client = if let Some(url) = redis_url {
            Some(Arc::new(Client::open(url).map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!("连接Redis失败: {}", e))
            })?))
        } else {
            None
        };

        Ok(Self {
            user_store: Arc::new(InMemoryUserStore::new()),
            password_cache: Arc::new(RwLock::new(HashMap::new())),
            kafka_producer,
            redis_client,
            status_cache: Arc::new(RwLock::new(HashMap::new())),
            search_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// 验证用户名格式
    fn validate_username(username: &str) -> ApplicationResult<()> {
        if username.is_empty() {
            return Err(UserError::Validation("用户名不能为空".to_string()).into());
        }

        if username.len() < 3 {
            return Err(UserError::Validation("用户名长度至少3个字符".to_string()).into());
        }

        if username.len() > 50 {
            return Err(UserError::Validation("用户名长度不能超过50个字符".to_string()).into());
        }

        // 检查用户名字符
        if !username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(UserError::Validation(
                "用户名只能包含字母、数字、下划线和连字符".to_string(),
            )
            .into());
        }

        Ok(())
    }

    /// 验证邮箱格式
    fn validate_email(email: &str) -> ApplicationResult<()> {
        if email.is_empty() {
            return Err(UserError::Validation("邮箱不能为空".to_string()).into());
        }

        // 简单的邮箱格式检查
        if !email.contains('@') || !email.contains('.') {
            return Err(UserError::Validation("邮箱格式不正确".to_string()).into());
        }

        if email.len() > 254 {
            return Err(UserError::Validation("邮箱长度过长".to_string()).into());
        }

        Ok(())
    }

    /// 验证密码强度
    fn validate_password(password: &str) -> ApplicationResult<()> {
        if password.is_empty() {
            return Err(UserError::Validation("密码不能为空".to_string()).into());
        }

        if password.len() < 8 {
            return Err(UserError::Validation("密码长度至少8个字符".to_string()).into());
        }

        if password.len() > 128 {
            return Err(UserError::Validation("密码长度不能超过128个字符".to_string()).into());
        }

        // 检查密码复杂度
        let has_lower = password.chars().any(|c| c.is_lowercase());
        let has_upper = password.chars().any(|c| c.is_uppercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_special = password
            .chars()
            .any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c));

        let complexity_count = [has_lower, has_upper, has_digit, has_special]
            .iter()
            .map(|&b| b as u8)
            .sum::<u8>();

        if complexity_count < 3 {
            return Err(UserError::Validation(
                "密码必须包含至少3种字符类型（大写字母、小写字母、数字、特殊字符）".to_string(),
            )
            .into());
        }

        Ok(())
    }

    /// 验证头像URL
    fn validate_avatar_url(url: &str) -> ApplicationResult<()> {
        if url.is_empty() {
            return Ok(()); // 允许空URL
        }

        // 检查URL格式
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(
                UserError::Validation("头像URL必须以http://或https://开头".to_string()).into(),
            );
        }

        if url.len() > 2048 {
            return Err(UserError::Validation("头像URL长度过长".to_string()).into());
        }

        Ok(())
    }

    /// 哈希密码
    async fn hash_password(&self, password: &str) -> ApplicationResult<String> {
        let password = password.to_string();
        tokio::task::spawn_blocking(move || {
            bcrypt::hash(&password, bcrypt::DEFAULT_COST)
                .map_err(|e| UserError::Internal(format!("密码哈希失败: {}", e)))
        })
        .await
        .map_err(|e| {
            crate::errors::ApplicationError::Infrastructure(format!("密码哈希任务失败: {}", e))
        })?
        .map_err(crate::errors::ApplicationError::from)
    }

    /// 验证密码
    async fn verify_password(&self, password: &str, hash: &str) -> ApplicationResult<bool> {
        let password = password.to_string();
        let hash = hash.to_string();
        tokio::task::spawn_blocking(move || {
            bcrypt::verify(&password, &hash)
                .map_err(|e| UserError::Internal(format!("密码验证失败: {}", e)))
        })
        .await
        .map_err(|e| {
            crate::errors::ApplicationError::Infrastructure(format!("密码验证任务失败: {}", e))
        })?
        .map_err(crate::errors::ApplicationError::from)
    }

    /// 缓存用户状态到Redis
    async fn cache_user_status(&self, user_id: Uuid, status: UserStatus) -> ApplicationResult<()> {
        if let Some(redis_client) = &self.redis_client {
            let key = format!("user_status:{}", user_id);
            let value = serde_json::to_string(&status).map_err(|e| {
                crate::errors::ApplicationError::Infrastructure(format!(
                    "序列化用户状态失败: {}",
                    e
                ))
            })?;

            // 异步缓存，不阻塞主流程
            let redis_client = Arc::clone(redis_client);
            tokio::spawn(async move {
                if let Ok(mut conn) = redis_client.get_multiplexed_async_connection().await {
                    let _: Result<String, _> = redis::cmd("SETEX")
                        .arg(&key)
                        .arg(3600) // 1小时过期
                        .arg(&value)
                        .query_async(&mut conn)
                        .await;
                }
            });
        }

        // 同时更新内存缓存
        let mut status_cache = self.status_cache.write().await;
        status_cache.insert(user_id, (status, Instant::now()));
        Ok(())
    }

    /// 发布用户事件
    async fn publish_user_event(&self, event: ChatEvent) -> ApplicationResult<()> {
        if let Some(producer) = &self.kafka_producer {
            tokio::spawn({
                let producer = Arc::clone(producer);
                let event = event.clone();
                async move {
                    if let Err(e) = producer.send_event(event).await {
                        error!("发布用户事件失败: {}", e);
                    }
                }
            });
            debug!("异步发布用户事件: {:?}", event);
        }
        Ok(())
    }

    /// 清理过期缓存
    async fn cleanup_expired_caches(&self) {
        let now = Instant::now();
        let cache_ttl = Duration::from_secs(3600); // 1小时TTL

        // 清理状态缓存
        let mut status_cache = self.status_cache.write().await;
        status_cache.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < cache_ttl);
        drop(status_cache);

        // 清理搜索缓存
        let mut search_cache = self.search_cache.write().await;
        search_cache.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < cache_ttl);
        drop(search_cache);
    }
}

#[async_trait::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(&self, request: CreateUserRequest) -> ApplicationResult<User> {
        info!("创建用户: {}", request.username);

        // 输入验证
        Self::validate_username(&request.username)?;
        Self::validate_email(&request.email)?;
        Self::validate_password(&request.password)?;

        if let Some(ref avatar_url) = request.avatar_url {
            Self::validate_avatar_url(avatar_url)?;
        }

        // 哈希密码
        let password_hash = self.hash_password(&request.password).await?;

        // 创建用户
        let user = User::new_with_details(
            request.username.clone(),
            request.email.clone(),
            password_hash.clone(),
            request.display_name.clone(),
            request.avatar_url.clone(),
        )
        .map_err(crate::errors::ApplicationError::from)?;

        // 存储用户
        self.user_store.store_user(user.clone()).await?;

        // 缓存密码哈希
        let mut password_cache = self.password_cache.write().await;
        password_cache.insert(user.id, password_hash);
        drop(password_cache);

        // 缓存用户状态
        self.cache_user_status(user.id, user.status.clone()).await?;

        // 发布用户创建事件
        let event = ChatEvent::UserCreated {
            user_id: user.id,
            username: user.username.clone(),
            email: user.email.clone(),
            timestamp: chrono::Utc::now(),
        };
        self.publish_user_event(event).await?;

        info!("用户创建成功: {} ({})", user.username, user.id);
        Ok(user)
    }

    async fn get_user_by_id(&self, user_id: Uuid) -> ApplicationResult<User> {
        let user = self
            .user_store
            .get_user(user_id)
            .await
            .ok_or(UserError::UserNotFound(user_id))?;

        // 检查用户是否已删除
        if user.status == UserStatus::Inactive {
            return Err(UserError::UserNotFound(user_id).into());
        }

        Ok(user)
    }

    async fn get_user_by_username(&self, username: &str) -> ApplicationResult<User> {
        let user = self
            .user_store
            .get_user_by_username(username)
            .await
            .ok_or_else(|| UserError::UserNotFoundByUsername(username.to_string()))?;

        // 检查用户是否已删除
        if user.status == UserStatus::Inactive {
            return Err(UserError::UserNotFoundByUsername(username.to_string()).into());
        }

        Ok(user)
    }

    async fn get_user_by_email(&self, email: &str) -> ApplicationResult<User> {
        let user = self
            .user_store
            .get_user_by_email(email)
            .await
            .ok_or_else(|| UserError::UserNotFoundByEmail(email.to_string()))?;

        // 检查用户是否已删除
        if user.status == UserStatus::Inactive {
            return Err(UserError::UserNotFoundByEmail(email.to_string()).into());
        }

        Ok(user)
    }

    async fn update_user(
        &self,
        user_id: Uuid,
        request: UpdateUserRequest,
    ) -> ApplicationResult<User> {
        info!("更新用户信息: {}", user_id);

        let mut user = self.get_user_by_id(user_id).await?;

        // 验证和更新各字段
        if let Some(ref username) = request.username {
            Self::validate_username(username)?;
            user.username = username.clone();
        }

        if let Some(ref email) = request.email {
            Self::validate_email(email)?;
            user.email = email.clone();
        }

        if let Some(ref display_name) = request.display_name {
            user.display_name = Some(display_name.clone());
        }

        if let Some(ref avatar_url) = request.avatar_url {
            Self::validate_avatar_url(avatar_url)?;
            user.avatar_url = Some(avatar_url.clone());
        }

        user.updated_at = chrono::Utc::now();

        // 更新存储
        self.user_store.update_user(user_id, user.clone()).await?;

        // 发布用户更新事件
        let event = ChatEvent::UserUpdated {
            user_id: user.id,
            nickname: user.display_name.clone().unwrap_or(user.username.clone()),
            timestamp: chrono::Utc::now(),
        };
        self.publish_user_event(event).await?;

        info!("用户信息更新成功: {}", user_id);
        Ok(user)
    }

    async fn update_user_status(&self, user_id: Uuid, status: UserStatus) -> ApplicationResult<()> {
        info!("更新用户状态: {} -> {:?}", user_id, status);

        let mut user = self.get_user_by_id(user_id).await?;
        let old_status = user.status.clone();

        user.status = status.clone();
        user.updated_at = chrono::Utc::now();

        // 更新存储
        self.user_store.update_user(user_id, user).await?;

        // 更新缓存
        self.cache_user_status(user_id, status.clone()).await?;

        // 发布状态变更事件
        let event = ChatEvent::UserStatusChanged {
            user_id,
            old_status: format!("{:?}", old_status),
            new_status: format!("{:?}", status),
            timestamp: chrono::Utc::now(),
        };
        self.publish_user_event(event).await?;

        info!("用户状态更新成功: {}", user_id);
        Ok(())
    }

    async fn get_users_status(
        &self,
        user_ids: &[Uuid],
    ) -> ApplicationResult<HashMap<Uuid, UserStatus>> {
        if user_ids.len() > 100 {
            return Err(UserError::Validation("批量查询用户数量不能超过100".to_string()).into());
        }

        let mut result = HashMap::new();
        let status_cache = self.status_cache.read().await;

        for &user_id in user_ids {
            // 先尝试从缓存获取
            if let Some((status, timestamp)) = status_cache.get(&user_id) {
                if timestamp.elapsed() < Duration::from_secs(300) {
                    // 5分钟缓存
                    result.insert(user_id, status.clone());
                    continue;
                }
            }

            // 缓存未命中，从存储获取
            if let Some(user) = self.user_store.get_user(user_id).await {
                result.insert(user_id, user.status);
            }
        }

        Ok(result)
    }

    async fn search_users(
        &self,
        request: UserSearchRequest,
    ) -> ApplicationResult<UserSearchResponse> {
        info!(
            "搜索用户: query={}, page={}, size={}",
            request.query, request.page, request.page_size
        );

        // 验证分页参数
        if request.page == 0 {
            return Err(UserError::Validation("页码必须从1开始".to_string()).into());
        }

        if request.page_size == 0 || request.page_size > 50 {
            return Err(UserError::Validation("每页大小必须在1-50之间".to_string()).into());
        }

        // 检查搜索缓存
        let cache_key = format!(
            "{}:{}:{:?}",
            request.query, request.page_size, request.status_filter
        );
        {
            let search_cache = self.search_cache.read().await;
            if let Some((cached_results, timestamp)) = search_cache.get(&cache_key) {
                if timestamp.elapsed() < Duration::from_secs(300) {
                    // 5分钟缓存
                    let total = cached_results.len() as u64;
                    let total_pages = total.div_ceil(request.page_size as u64);
                    let start = ((request.page - 1) * request.page_size) as usize;
                    let end =
                        std::cmp::min(start + request.page_size as usize, cached_results.len());

                    let users = if start < cached_results.len() {
                        cached_results[start..end].to_vec()
                    } else {
                        Vec::new()
                    };

                    return Ok(UserSearchResponse {
                        users,
                        total,
                        page: request.page,
                        page_size: request.page_size,
                        total_pages: total_pages as u32,
                    });
                }
            }
        }

        // 执行搜索
        let mut all_results = self
            .user_store
            .search_users(&request.query, request.status_filter)
            .await;

        // 按创建时间排序
        all_results.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // 更新搜索缓存
        {
            let mut search_cache = self.search_cache.write().await;
            search_cache.insert(cache_key, (all_results.clone(), Instant::now()));
        }

        // 分页处理
        let total = all_results.len() as u64;
        let total_pages = total.div_ceil(request.page_size as u64);
        let start = ((request.page - 1) * request.page_size) as usize;
        let end = std::cmp::min(start + request.page_size as usize, all_results.len());

        let users = if start < all_results.len() {
            all_results[start..end].to_vec()
        } else {
            Vec::new()
        };

        Ok(UserSearchResponse {
            users,
            total,
            page: request.page,
            page_size: request.page_size,
            total_pages: total_pages as u32,
        })
    }

    async fn get_user_extensions(&self, user_id: Uuid) -> ApplicationResult<JsonValue> {
        let extensions = self.user_store.user_extensions.read().await;
        Ok(extensions
            .get(&user_id)
            .cloned()
            .unwrap_or(JsonValue::Object(serde_json::Map::new())))
    }

    async fn update_user_extensions(
        &self,
        user_id: Uuid,
        extensions: JsonValue,
    ) -> ApplicationResult<()> {
        info!("更新用户扩展信息: {}", user_id);

        // 验证用户存在
        self.get_user_by_id(user_id).await?;

        // 验证JSON格式和大小
        let json_str = serde_json::to_string(&extensions)
            .map_err(|e| UserError::Validation(format!("扩展信息JSON格式无效: {}", e)))?;

        if json_str.len() > 65536 {
            // 64KB限制
            return Err(UserError::Validation("扩展信息过大，最大支持64KB".to_string()).into());
        }

        // 更新扩展信息
        let mut user_extensions = self.user_store.user_extensions.write().await;
        user_extensions.insert(user_id, extensions);

        info!("用户扩展信息更新成功: {}", user_id);
        Ok(())
    }

    async fn delete_user(&self, user_id: Uuid) -> ApplicationResult<()> {
        info!("删除用户: {}", user_id);

        // 软删除用户
        self.user_store.delete_user(user_id).await?;

        // 清理缓存
        {
            let mut status_cache = self.status_cache.write().await;
            status_cache.remove(&user_id);
        }

        {
            let mut password_cache = self.password_cache.write().await;
            password_cache.remove(&user_id);
        }

        // 发布用户删除事件
        let event = ChatEvent::UserDeleted {
            user_id,
            deleted_by: user_id, // 用户自己删除自己
            timestamp: chrono::Utc::now(),
        };
        self.publish_user_event(event).await?;

        info!("用户删除成功: {}", user_id);
        Ok(())
    }

    async fn get_user_stats(&self) -> ApplicationResult<UserStats> {
        let all_users = self.user_store.get_all_users().await;

        let mut total_users = 0u64;
        let mut active_users = 0u64;
        let mut online_users = 0u64;
        let mut busy_users = 0u64;
        let mut away_users = 0u64;
        let mut today_new_users = 0u64;

        let now = chrono::Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

        for user in &all_users {
            if user.status != UserStatus::Inactive {
                total_users += 1;

                match user.status {
                    UserStatus::Active => {
                        active_users += 1;
                        online_users += 1;
                    }
                    UserStatus::Deleted => {
                        active_users += 1;
                        busy_users += 1;
                    }
                    UserStatus::Suspended => {
                        away_users += 1;
                    }
                    _ => {}
                }

                // 统计今日新增用户
                if user.created_at >= today_start {
                    today_new_users += 1;
                }
            }
        }

        Ok(UserStats {
            total_users,
            active_users,
            online_users,
            busy_users,
            away_users,
            today_new_users,
        })
    }

    async fn verify_credentials(
        &self,
        username_or_email: &str,
        password: &str,
    ) -> ApplicationResult<User> {
        // 尝试通过用户名或邮箱查找用户
        let user = if username_or_email.contains('@') {
            self.user_store.get_user_by_email(username_or_email).await
        } else {
            self.user_store
                .get_user_by_username(username_or_email)
                .await
        };

        let user = user.ok_or(UserError::InvalidCredentials)?;

        // 检查用户状态
        if user.status == UserStatus::Inactive {
            return Err(UserError::UserInactive.into());
        }

        // 获取密码哈希
        let password_hash = {
            let password_cache = self.password_cache.read().await;
            password_cache.get(&user.id).cloned()
        };

        let password_hash = password_hash.ok_or(UserError::InvalidCredentials)?;

        // 验证密码
        let is_valid = self.verify_password(password, &password_hash).await?;
        if !is_valid {
            return Err(UserError::InvalidCredentials.into());
        }

        Ok(user)
    }

    async fn update_last_activity(&self, user_id: Uuid) -> ApplicationResult<()> {
        let mut user = self.get_user_by_id(user_id).await?;
        user.last_active_at = Some(chrono::Utc::now());
        user.updated_at = chrono::Utc::now();

        self.user_store.update_user(user_id, user).await?;
        Ok(())
    }
}

// 用于测试的辅助方法
impl UserServiceImpl {
    /// 获取用户总数（用于测试）
    pub async fn get_user_count(&self) -> usize {
        let users = self.user_store.users.read().await;
        users.len()
    }

    /// 清理所有缓存（用于测试）
    pub async fn clear_all_caches(&self) {
        let mut status_cache = self.status_cache.write().await;
        status_cache.clear();

        let mut search_cache = self.search_cache.write().await;
        search_cache.clear();

        let mut password_cache = self.password_cache.write().await;
        password_cache.clear();
    }
}

// 添加Validation错误变体
impl From<String> for UserError {
    fn from(msg: String) -> Self {
        UserError::Validation(msg)
    }
}
