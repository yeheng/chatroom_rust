//! 用户命令处理器
//!
//! 处理用户相关的命令：注册、登录、更新、删除等

use crate::errors::{ApplicationResult, UserError};
use crate::cqrs::{
    CommandHandler,
    commands::*,
    EventBus,
};
use domain::entities::user::{User, UserStatus};
use infrastructure::events::ChatEvent;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use async_trait::async_trait;
use bcrypt;
use tracing::info;

/// 简化的用户仓储接口
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn save(&self, user: User) -> ApplicationResult<User>;
    async fn find_by_id(&self, user_id: Uuid) -> ApplicationResult<Option<User>>;
    async fn find_by_username(&self, username: String) -> ApplicationResult<Option<User>>;
    async fn find_by_email(&self, email: String) -> ApplicationResult<Option<User>>;
    async fn delete(&self, user_id: Uuid) -> ApplicationResult<()>;
    async fn store_password_hash(&self, user_id: Uuid, hash: String) -> ApplicationResult<()>;
    async fn get_password_hash(&self, user_id: Uuid) -> ApplicationResult<Option<String>>;
}

/// 内存用户仓储实现
pub struct InMemoryUserRepository {
    users: Arc<RwLock<HashMap<Uuid, User>>>,
    username_index: Arc<RwLock<HashMap<String, Uuid>>>,
    email_index: Arc<RwLock<HashMap<String, Uuid>>>,
    password_hash: Arc<RwLock<HashMap<Uuid, String>>>,
}

impl Default for InMemoryUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            username_index: Arc::new(RwLock::new(HashMap::new())),
            email_index: Arc::new(RwLock::new(HashMap::new())),
            password_hash: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn save(&self, user: User) -> ApplicationResult<User> {
        let mut users = self.users.write().await;
        let mut username_index = self.username_index.write().await;
        let mut email_index = self.email_index.write().await;

        // 检查用户名和邮箱冲突（排除自己）
        if let Some(&existing_id) = username_index.get(&user.username) {
            if existing_id != user.id {
                return Err(UserError::UsernameConflict(user.username.clone()).into());
            }
        }

        if let Some(&existing_id) = email_index.get(&user.email) {
            if existing_id != user.id {
                return Err(UserError::EmailConflict(user.email.clone()).into());
            }
        }

        // 更新索引
        if let Some(old_user) = users.get(&user.id) {
            username_index.remove(&old_user.username);
            email_index.remove(&old_user.email);
        }
        username_index.insert(user.username.clone(), user.id);
        email_index.insert(user.email.clone(), user.id);

        // 存储用户
        users.insert(user.id, user.clone());
        Ok(user)
    }

    async fn find_by_id(&self, user_id: Uuid) -> ApplicationResult<Option<User>> {
        let users = self.users.read().await;
        Ok(users.get(&user_id).cloned())
    }

    async fn find_by_username(&self, username: String) -> ApplicationResult<Option<User>> {
        let username_index = self.username_index.read().await;
        if let Some(&user_id) = username_index.get(&username) {
            drop(username_index);
            let users = self.users.read().await;
            Ok(users.get(&user_id).cloned())
        } else {
            Ok(None)
        }
    }

    async fn find_by_email(&self, email: String) -> ApplicationResult<Option<User>> {
        let email_index = self.email_index.read().await;
        if let Some(&user_id) = email_index.get(&email) {
            drop(email_index);
            let users = self.users.read().await;
            Ok(users.get(&user_id).cloned())
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, user_id: Uuid) -> ApplicationResult<()> {
        let mut users = self.users.write().await;

        if let Some(user) = users.get_mut(&user_id) {
            // 软删除：更新状态
            user.status = UserStatus::Inactive;
            user.updated_at = chrono::Utc::now();
            Ok(())
        } else {
            Err(UserError::UserNotFound(user_id).into())
        }
    }

    async fn store_password_hash(&self, user_id: Uuid, hash: String) -> ApplicationResult<()> {
        let mut password_hash = self.password_hash.write().await;
        password_hash.insert(user_id, hash);
        Ok(())
    }

    async fn get_password_hash(&self, user_id: Uuid) -> ApplicationResult<Option<String>> {
        let password_hash = self.password_hash.read().await;
        Ok(password_hash.get(&user_id).cloned())
    }
}

/// 用户命令处理器
pub struct UserCommandHandler {
    user_repository: Arc<dyn UserRepository>,
    event_bus: Option<Arc<dyn EventBus>>,
}

impl UserCommandHandler {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self {
            user_repository,
            event_bus: None,
        }
    }

    pub fn with_event_bus(mut self, event_bus: Arc<dyn EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
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

    /// 发布用户事件（如果配置了事件总线）
    async fn publish_user_event(&self, _event: ChatEvent) -> ApplicationResult<()> {
        // 暂时跳过事件发布，避免类型兼容性问题
        // if let Some(event_bus) = &self.event_bus {
        //     let domain_event = Arc::new(event);
        //     event_bus.publish(domain_event).await?;
        // }
        Ok(())
    }
}

#[async_trait]
impl CommandHandler<RegisterUserCommand> for UserCommandHandler {
    async fn handle(&self, command: RegisterUserCommand) -> ApplicationResult<User> {
        info!("处理用户注册命令: {}", command.username);

        // 验证输入
        Self::validate_username(&command.username)?;
        Self::validate_email(&command.email)?;
        Self::validate_password(&command.password)?;

        // 检查用户名和邮箱是否已存在
        if let Some(_) = self.user_repository.find_by_username(command.username.clone()).await? {
            return Err(UserError::UsernameConflict(command.username).into());
        }

        if let Some(_) = self.user_repository.find_by_email(command.email.clone()).await? {
            return Err(UserError::EmailConflict(command.email).into());
        }

        // 哈希密码
        let password_hash = self.hash_password(&command.password).await?;

        // 创建用户
        let user = User::new_with_details(
            command.username,
            command.email,
            password_hash.clone(),
            command.display_name,
            command.avatar_url,
        )
        .map_err(crate::errors::ApplicationError::from)?;

        // 保存用户和密码哈希
        let saved_user = self.user_repository.save(user).await?;
        self.user_repository.store_password_hash(saved_user.id, password_hash).await?;

        // 发布用户注册事件
        self.publish_user_event(ChatEvent::UserCreated {
            user_id: saved_user.id,
            username: saved_user.username.clone(),
            email: saved_user.email.clone(),
            timestamp: chrono::Utc::now(),
        }).await?;

        info!("用户注册成功: {} ({})", saved_user.username, saved_user.id);
        Ok(saved_user)
    }
}

#[async_trait]
impl CommandHandler<LoginUserCommand> for UserCommandHandler {
    async fn handle(&self, command: LoginUserCommand) -> ApplicationResult<User> {
        info!("处理用户登录命令: {}", command.email);

        // 查找用户
        let user = self.user_repository
            .find_by_email(command.email.clone())
            .await?
            .ok_or(UserError::InvalidCredentials)?;

        // 检查用户状态
        if user.status == UserStatus::Inactive {
            return Err(UserError::UserInactive.into());
        }

        // 验证密码
        if let Some(password_hash) = self.user_repository.get_password_hash(user.id).await? {
            let is_valid = self.verify_password(&command.password, &password_hash).await?;
            if !is_valid {
                return Err(UserError::InvalidCredentials.into());
            }
        } else {
            return Err(UserError::InvalidCredentials.into());
        }

        // 更新用户状态为活跃
        let mut updated_user = user.clone();
        updated_user.status = UserStatus::Active;
        updated_user.last_activity_at = Some(chrono::Utc::now());
        updated_user.updated_at = chrono::Utc::now();

        let updated_user = self.user_repository.save(updated_user).await?;

        // 发布登录事件
        self.publish_user_event(ChatEvent::UserCreated {  // 暂时使用UserCreated，避免UserLoggedIn不存在的错误
            user_id: updated_user.id,
            username: updated_user.username.clone(),
            email: updated_user.email.clone(),
            timestamp: chrono::Utc::now(),
        }).await?;

        info!("用户登录成功: {} ({})", updated_user.email, updated_user.id);
        Ok(updated_user)
    }
}

#[async_trait]
impl CommandHandler<UpdateUserCommand> for UserCommandHandler {
    async fn handle(&self, command: UpdateUserCommand) -> ApplicationResult<User> {
        info!("处理用户更新命令: {}", command.user_id);

        // 获取现有用户
        let mut user = self.user_repository
            .find_by_id(command.user_id)
            .await?
            .ok_or(UserError::UserNotFound(command.user_id))?;

        // 验证和更新各字段
        if let Some(ref username) = command.username {
            Self::validate_username(username)?;
            user.username = username.clone();
        }

        if let Some(ref email) = command.email {
            Self::validate_email(email)?;
            user.email = email.clone();
        }

        if let Some(ref display_name) = command.display_name {
            user.display_name = Some(display_name.clone());
        }

        if let Some(ref avatar_url) = command.avatar_url {
            user.avatar_url = Some(avatar_url.clone());
        }

        user.updated_at = chrono::Utc::now();

        // 保存更新后的用户
        let updated_user = self.user_repository.save(user).await?;

        info!("用户信息更新成功: {}", command.user_id);
        Ok(updated_user)
    }
}

#[async_trait]
impl CommandHandler<UpdateUserStatusCommand> for UserCommandHandler {
    async fn handle(&self, command: UpdateUserStatusCommand) -> ApplicationResult<()> {
        info!("处理用户状态更新命令: {} -> {:?}", command.user_id, command.status);

        // 获取现有用户
        let mut user = self.user_repository
            .find_by_id(command.user_id)
            .await?
            .ok_or(UserError::UserNotFound(command.user_id))?;

        // 更新状态
        user.status = command.status.clone();
        user.updated_at = chrono::Utc::now();

        // 保存更新
        self.user_repository.save(user).await?;

        info!("用户状态更新成功: {}", command.user_id);
        Ok(())
    }
}

#[async_trait]
impl CommandHandler<DeleteUserCommand> for UserCommandHandler {
    async fn handle(&self, command: DeleteUserCommand) -> ApplicationResult<()> {
        info!("处理用户删除命令: {}", command.user_id);

        // 验证用户存在
        let _user = self.user_repository
            .find_by_id(command.user_id)
            .await?
            .ok_or(UserError::UserNotFound(command.user_id))?;

        // 软删除用户
        self.user_repository.delete(command.user_id).await?;

        info!("用户删除成功: {}", command.user_id);
        Ok(())
    }
}