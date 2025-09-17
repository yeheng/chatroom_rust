//! 用户实体定义
//!
//! 包含用户的核心信息和相关操作。

use crate::errors::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// 用户状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    /// 活跃状态
    Active,
    /// 未激活
    Inactive,
    /// 暂停
    Suspended,
    /// 已删除
    Deleted,
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserStatus::Active => write!(f, "active"),
            UserStatus::Inactive => write!(f, "inactive"),
            UserStatus::Suspended => write!(f, "suspended"),
            UserStatus::Deleted => write!(f, "deleted"),
        }
    }
}

impl Default for UserStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// 用户实体
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// 用户唯一ID
    pub id: Uuid,
    /// 用户名（唯一）
    pub username: String,
    /// 邮箱（唯一）
    pub email: String,
    /// 密码哈希（敏感信息，不在序列化中包含）
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    /// 头像URL（可选）
    pub avatar_url: Option<String>,
    /// 显示名称（可选）
    pub display_name: Option<String>,
    /// 用户状态
    pub status: UserStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 最后活跃时间（与数据库字段名保持一致）
    pub last_active_at: Option<DateTime<Utc>>,
}

impl User {
    /// 创建新用户
    pub fn new(username: impl Into<String>, email: impl Into<String>) -> DomainResult<Self> {
        let username = username.into();
        let email = email.into();

        // 验证用户名
        Self::validate_username(&username)?;

        // 验证邮箱
        Self::validate_email(&email)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            username,
            email,
            avatar_url: None,
            display_name: None,
            status: UserStatus::Active,
            created_at: now,
            updated_at: now,
            last_activity_at: Some(now),
        })
    }

    /// 创建新用户（完整版本）
    pub fn new_with_details(
        username: impl Into<String>,
        email: impl Into<String>,
        _password_hash: String,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> DomainResult<Self> {
        let username = username.into();
        let email = email.into();

        // 验证用户名
        Self::validate_username(&username)?;

        // 验证邮箱
        Self::validate_email(&email)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            username,
            email,
            avatar_url,
            display_name,
            status: UserStatus::Active,
            created_at: now,
            updated_at: now,
            last_activity_at: Some(now),
        })
    }

    /// 创建具有指定ID的用户（用于从数据库加载）
    pub fn with_id(
        id: Uuid,
        username: impl Into<String>,
        email: impl Into<String>,
        avatar_url: Option<String>,
        display_name: Option<String>,
        status: UserStatus,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        last_activity_at: Option<DateTime<Utc>>,
    ) -> DomainResult<Self> {
        let username = username.into();
        let email = email.into();

        // 验证用户名
        Self::validate_username(&username)?;

        // 验证邮箱
        Self::validate_email(&email)?;

        Ok(Self {
            id,
            username,
            email,
            avatar_url,
            display_name,
            status,
            created_at,
            updated_at,
            last_activity_at,
        })
    }

    /// 更新用户名
    pub fn update_username(&mut self, new_username: impl Into<String>) -> DomainResult<()> {
        let new_username = new_username.into();
        Self::validate_username(&new_username)?;

        self.username = new_username;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 更新邮箱
    pub fn update_email(&mut self, new_email: impl Into<String>) -> DomainResult<()> {
        let new_email = new_email.into();
        Self::validate_email(&new_email)?;

        self.email = new_email;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 设置头像URL
    pub fn set_avatar_url(&mut self, avatar_url: Option<String>) {
        self.avatar_url = avatar_url;
        self.updated_at = Utc::now();
    }

    /// 更新用户状态
    pub fn update_status(&mut self, new_status: UserStatus) {
        self.status = new_status;
        self.updated_at = Utc::now();
    }

    /// 记录最后活跃时间
    pub fn mark_active(&mut self) {
        self.last_activity_at = Some(Utc::now());
        if matches!(self.status, UserStatus::Inactive) {
            self.status = UserStatus::Active;
        }
        self.updated_at = Utc::now();
    }

    /// 设置用户为未激活状态  
    pub fn mark_inactive(&mut self) {
        self.status = UserStatus::Inactive;
        self.updated_at = Utc::now();
    }

    /// 暂停用户
    pub fn suspend(&mut self) {
        self.status = UserStatus::Suspended;
        self.updated_at = Utc::now();
    }

    /// 激活用户
    pub fn activate(&mut self) {
        self.status = UserStatus::Active;
        self.updated_at = Utc::now();
    }

    /// 软删除用户
    pub fn soft_delete(&mut self) {
        self.status = UserStatus::Deleted;
        self.updated_at = Utc::now();
    }

    /// 检查用户是否活跃
    pub fn is_active(&self) -> bool {
        matches!(self.status, UserStatus::Active)
    }

    /// 检查用户是否未激活
    pub fn is_inactive(&self) -> bool {
        matches!(self.status, UserStatus::Inactive)
    }

    /// 检查用户是否被暂停
    pub fn is_suspended(&self) -> bool {
        matches!(self.status, UserStatus::Suspended)
    }

    /// 检查用户是否被删除
    pub fn is_deleted(&self) -> bool {
        matches!(self.status, UserStatus::Deleted)
    }

    /// 验证用户名格式
    fn validate_username(username: &str) -> DomainResult<()> {
        if username.is_empty() {
            return Err(DomainError::validation_error("username", "用户名不能为空"));
        }

        if username.len() < 2 {
            return Err(DomainError::validation_error(
                "username",
                "用户名长度至少2个字符",
            ));
        }

        if username.len() > 50 {
            return Err(DomainError::validation_error(
                "username",
                "用户名长度不能超过50个字符",
            ));
        }

        // 检查用户名格式（只允许字母、数字、下划线和连字符）
        if !username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(DomainError::validation_error(
                "username",
                "用户名只能包含字母、数字、下划线和连字符",
            ));
        }

        Ok(())
    }

    /// 验证邮箱格式
    fn validate_email(email: &str) -> DomainResult<()> {
        if email.is_empty() {
            return Err(DomainError::validation_error("email", "邮箱不能为空"));
        }

        // 简单的邮箱格式验证
        if !email.contains('@') || !email.contains('.') {
            return Err(DomainError::validation_error("email", "邮箱格式不正确"));
        }

        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(DomainError::validation_error("email", "邮箱格式不正确"));
        }

        if email.len() > 255 {
            return Err(DomainError::validation_error(
                "email",
                "邮箱长度不能超过255个字符",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_creation() {
        let user = User::new("testuser", "test@example.com").unwrap();
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.status, UserStatus::Active);
        assert!(user.is_active());
        assert!(user.last_activity_at.is_some());
    }

    #[test]
    fn test_username_validation() {
        // 有效用户名
        assert!(User::new("user123", "test@example.com").is_ok());
        assert!(User::new("user_name", "test@example.com").is_ok());
        assert!(User::new("user-name", "test@example.com").is_ok());

        // 无效用户名
        assert!(User::new("", "test@example.com").is_err());
        assert!(User::new("u", "test@example.com").is_err());
        assert!(User::new("user@name", "test@example.com").is_err());
        assert!(User::new("a".repeat(51), "test@example.com").is_err());
    }

    #[test]
    fn test_email_validation() {
        // 有效邮箱
        assert!(User::new("test", "test@example.com").is_ok());
        assert!(User::new("test", "user.name@domain.co.uk").is_ok());

        // 无效邮箱
        assert!(User::new("test", "").is_err());
        assert!(User::new("test", "invalid-email").is_err());
        assert!(User::new("test", "@example.com").is_err());
        assert!(User::new("test", "test@").is_err());
        assert!(User::new("test", &"a".repeat(256)).is_err());
    }

    #[test]
    fn test_user_status_operations() {
        let mut user = User::new("testuser", "test@example.com").unwrap();

        // 测试标记未激活
        user.mark_inactive();
        assert_eq!(user.status, UserStatus::Inactive);
        assert!(!user.is_active());

        // 测试标记活跃
        user.mark_active();
        assert_eq!(user.status, UserStatus::Active);
        assert!(user.is_active());

        // 测试暂停
        user.suspend();
        assert!(user.is_suspended());

        // 测试激活
        user.activate();
        assert!(user.is_active());

        // 测试软删除
        user.soft_delete();
        assert!(user.is_deleted());
    }

    #[test]
    fn test_user_updates() {
        let mut user = User::new("testuser", "test@example.com").unwrap();
        let original_updated_at = user.updated_at;

        // 等待一小段时间确保时间戳不同
        std::thread::sleep(std::time::Duration::from_millis(1));

        // 测试更新用户名
        user.update_username("newuser").unwrap();
        assert_eq!(user.username, "newuser");
        assert!(user.updated_at > original_updated_at);

        // 测试更新邮箱
        let before_email_update = user.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        user.update_email("new@example.com").unwrap();
        assert_eq!(user.email, "new@example.com");
        assert!(user.updated_at > before_email_update);

        // 测试设置头像
        user.set_avatar_url(Some("https://example.com/avatar.jpg".to_string()));
        assert_eq!(
            user.avatar_url,
            Some("https://example.com/avatar.jpg".to_string())
        );
    }

    #[test]
    fn test_user_serialization() {
        let user = User::new("testuser", "test@example.com").unwrap();

        // 测试序列化
        let json = serde_json::to_string(&user).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: User = serde_json::from_str(&json).unwrap();
        assert_eq!(user, deserialized);
    }
}
