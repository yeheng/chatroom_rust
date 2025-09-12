//! JWT认证相关实体
//!
//! 定义JWT令牌、用户认证状态等认证相关实体。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// JWT令牌声明
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    /// 主题（用户ID）
    pub sub: String,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: Option<String>,
    /// 用户角色
    pub role: UserRole,
    /// 权限列表
    pub permissions: Vec<Permission>,
    /// 令牌唯一标识符（用于撤销）
    pub jti: String,
    /// 签发时间
    pub iat: i64,
    /// 过期时间
    pub exp: i64,
    /// 签发者
    pub iss: String,
    /// 受众
    pub aud: String,
}

/// 用户角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserRole {
    /// 管理员
    Admin,
    /// 普通用户
    User,
    /// 访客
    Guest,
}

/// 用户权限
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    /// 创建聊天室
    CreateRoom,
    /// 加入聊天室
    JoinRoom,
    /// 发送消息
    SendMessage,
    /// 编辑消息
    EditMessage,
    /// 删除消息
    DeleteMessage,
    /// 管理用户
    ManageUsers,
    /// 管理系统
    ManageSystem,
}

/// 登录凭证
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
    /// 用户名或邮箱
    pub username: String,
    /// 密码
    pub password: String,
}

/// 登录响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: String,
    /// 令牌类型
    pub token_type: String,
    /// 过期时间（秒）
    pub expires_in: i64,
    /// 用户信息
    pub user: UserInfo,
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// 用户ID
    pub id: Uuid,
    /// 用户名
    pub username: String,
    /// 邮箱
    pub email: Option<String>,
    /// 用户角色
    pub role: UserRole,
    /// 权限列表
    pub permissions: Vec<Permission>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后登录时间
    pub last_login: Option<DateTime<Utc>>,
}

/// 刷新令牌请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    /// 刷新令牌
    pub refresh_token: String,
}

/// 令牌对
#[derive(Debug, Clone)]
pub struct TokenPair {
    /// 访问令牌
    pub access_token: String,
    /// 刷新令牌
    pub refresh_token: String,
    /// 访问令牌过期时间
    pub access_token_expires_at: DateTime<Utc>,
    /// 刷新令牌过期时间
    pub refresh_token_expires_at: DateTime<Utc>,
}

/// 认证错误
#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthError {
    /// 令牌无效
    #[error("Invalid token")]
    InvalidToken,
    /// 令牌已过期
    #[error("Token expired")]
    TokenExpired,
    /// 令牌已被撤销
    #[error("Token revoked")]
    TokenRevoked,
    /// 签名验证失败
    #[error("Invalid signature")]
    InvalidSignature,
    /// 无效的凭证
    #[error("Invalid credentials")]
    InvalidCredentials,
    /// 用户不存在
    #[error("User not found")]
    UserNotFound,
    /// 用户已禁用
    #[error("User disabled")]
    UserDisabled,
    /// 密码错误
    #[error("Invalid password")]
    InvalidPassword,
    /// 刷新令牌无效
    #[error("Invalid refresh token")]
    InvalidRefreshToken,
    /// 权限不足
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    /// 内部错误
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::Admin => write!(f, "admin"),
            UserRole::User => write!(f, "user"),
            UserRole::Guest => write!(f, "guest"),
        }
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::CreateRoom => write!(f, "create_room"),
            Permission::JoinRoom => write!(f, "join_room"),
            Permission::SendMessage => write!(f, "send_message"),
            Permission::EditMessage => write!(f, "edit_message"),
            Permission::DeleteMessage => write!(f, "delete_message"),
            Permission::ManageUsers => write!(f, "manage_users"),
            Permission::ManageSystem => write!(f, "manage_system"),
        }
    }
}

/// 默认权限配置
impl Default for UserRole {
    fn default() -> Self {
        UserRole::User
    }
}

impl UserRole {
    /// 获取角色的默认权限
    pub fn default_permissions(&self) -> Vec<Permission> {
        match self {
            UserRole::Admin => vec![
                Permission::CreateRoom,
                Permission::JoinRoom,
                Permission::SendMessage,
                Permission::EditMessage,
                Permission::DeleteMessage,
                Permission::ManageUsers,
                Permission::ManageSystem,
            ],
            UserRole::User => vec![
                Permission::JoinRoom,
                Permission::SendMessage,
                Permission::EditMessage,
                Permission::DeleteMessage,
            ],
            UserRole::Guest => vec![Permission::JoinRoom, Permission::SendMessage],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_role_permissions() {
        let admin_perms = UserRole::Admin.default_permissions();
        assert!(admin_perms.contains(&Permission::ManageSystem));

        let user_perms = UserRole::User.default_permissions();
        assert!(!user_perms.contains(&Permission::ManageSystem));
        assert!(user_perms.contains(&Permission::SendMessage));

        let guest_perms = UserRole::Guest.default_permissions();
        assert!(!guest_perms.contains(&Permission::EditMessage));
        assert!(guest_perms.contains(&Permission::JoinRoom));
    }

    #[test]
    fn test_token_serialization() {
        let claims = Claims {
            sub: "user123".to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            role: UserRole::User,
            permissions: vec![Permission::SendMessage],
            jti: "token123".to_string(),
            iat: 1640995200,
            exp: 1641081600,
            iss: "chatroom".to_string(),
            aud: "users".to_string(),
        };

        let json = serde_json::to_string(&claims).unwrap();
        let deserialized: Claims = serde_json::from_str(&json).unwrap();

        assert_eq!(claims, deserialized);
    }

    #[test]
    fn test_login_credentials_validation() {
        let credentials = LoginCredentials {
            username: "testuser".to_string(),
            password: "password123".to_string(),
        };

        assert!(!credentials.username.is_empty());
        assert!(!credentials.password.is_empty());
    }
}
