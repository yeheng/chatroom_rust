//! JWT认证基础设施实现
//!
//! 实现JWT认证服务，包括令牌生成、验证和撤销功能。

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use domain::entities::auth::*;
use domain::services::auth_service::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::config::RedisConfig;

/// Redis令牌黑名单服务
pub struct RedisTokenBlacklistService {
    /// Redis客户端
    redis_client: Arc<redis::Client>,
    /// 黑名单键前缀
    blacklist_prefix: String,
}

impl RedisTokenBlacklistService {
    /// 创建新的Redis令牌黑名单服务
    pub fn new(redis_config: &RedisConfig) -> Result<Self, String> {
        let client = redis::Client::open(redis_config.url.clone())
            .map_err(|e| format!("Failed to create Redis client: {}", e))?;

        Ok(Self {
            redis_client: Arc::new(client),
            blacklist_prefix: "token_blacklist:".to_string(),
        })
    }

    /// 生成黑名单键
    fn blacklist_key(&self, jti: &str) -> String {
        format!("{}{}", self.blacklist_prefix, jti)
    }

    /// 获取Redis连接
    async fn get_connection(&self) -> Result<redis::aio::ConnectionManager, String> {
        self.redis_client
            .get_connection_manager()
            .await
            .map_err(|e| format!("Failed to get Redis connection: {}", e))
    }
}

#[async_trait]
impl TokenBlacklistService for RedisTokenBlacklistService {
    async fn add_to_blacklist(
        &self,
        jti: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        let mut conn = self
            .get_connection()
            .await
            .map_err(|e| AuthError::InternalError(e))?;

        let key = self.blacklist_key(jti);
        let ttl = (expires_at - Utc::now()).num_seconds();

        if ttl > 0 {
            let _: () = redis::cmd("SETEX")
                .arg(&key)
                .arg(ttl)
                .arg("1")
                .query_async(&mut conn)
                .await
                .map_err(|e| {
                    AuthError::InternalError(format!("Failed to add token to blacklist: {}", e))
                })?;

            info!("Added token {} to blacklist, TTL: {}s", jti, ttl);
        } else {
            warn!("Token {} already expired, skipping blacklist", jti);
        }

        Ok(())
    }

    async fn is_blacklisted(&self, jti: &str) -> Result<bool, AuthError> {
        let mut conn = self
            .get_connection()
            .await
            .map_err(|e| AuthError::InternalError(e))?;

        let key = self.blacklist_key(jti);
        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AuthError::InternalError(format!("Failed to check blacklist: {}", e)))?;

        Ok(exists)
    }

    async fn remove_from_blacklist(&self, jti: &str) -> Result<(), AuthError> {
        let mut conn = self
            .get_connection()
            .await
            .map_err(|e| AuthError::InternalError(e))?;

        let key = self.blacklist_key(jti);
        let _: () = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                AuthError::InternalError(format!("Failed to remove from blacklist: {}", e))
            })?;

        debug!("Removed token {} from blacklist", jti);
        Ok(())
    }

    async fn cleanup_expired_tokens(&self) -> Result<usize, AuthError> {
        // Redis的SETEX命令会自动清理过期键，这里主要做日志记录
        info!("Token blacklist cleanup completed (Redis handles expiration automatically)");
        Ok(0)
    }
}

/// 内存中的令牌黑名单服务（用于测试）
pub struct InMemoryTokenBlacklistService {
    /// 黑名单存储
    blacklist: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl InMemoryTokenBlacklistService {
    /// 创建新的内存令牌黑名单服务
    pub fn new() -> Self {
        Self {
            blacklist: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 清理过期令牌
    async fn cleanup(&self) {
        let mut blacklist = self.blacklist.write().await;
        let now = Utc::now();

        blacklist.retain(|_, expires_at| *expires_at > now);
    }
}

#[async_trait]
impl TokenBlacklistService for InMemoryTokenBlacklistService {
    async fn add_to_blacklist(
        &self,
        jti: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        self.cleanup().await;

        let mut blacklist = self.blacklist.write().await;
        blacklist.insert(jti.to_string(), expires_at);

        debug!("Added token {} to in-memory blacklist", jti);
        Ok(())
    }

    async fn is_blacklisted(&self, jti: &str) -> Result<bool, AuthError> {
        self.cleanup().await;

        let blacklist = self.blacklist.read().await;
        Ok(blacklist.contains_key(jti))
    }

    async fn remove_from_blacklist(&self, jti: &str) -> Result<(), AuthError> {
        let mut blacklist = self.blacklist.write().await;
        blacklist.remove(jti);

        debug!("Removed token {} from in-memory blacklist", jti);
        Ok(())
    }

    async fn cleanup_expired_tokens(&self) -> Result<usize, AuthError> {
        let before_len = {
            let blacklist = self.blacklist.read().await;
            blacklist.len()
        };

        self.cleanup().await;

        let after_len = {
            let blacklist = self.blacklist.read().await;
            blacklist.len()
        };

        Ok(before_len - after_len)
    }
}

/// JWT认证服务实现
pub struct JwtAuthServiceImpl {
    /// JWT编码器
    jwt_encoder: JwtEncoder,
    /// 令牌黑名单服务
    token_blacklist: Arc<dyn TokenBlacklistService>,
    /// 用户服务（需要从application层引入）
    user_service: Arc<dyn UserAuthService>,
}

impl JwtAuthServiceImpl {
    /// 创建新的JWT认证服务
    pub fn new(
        jwt_encoder: JwtEncoder,
        token_blacklist: Arc<dyn TokenBlacklistService>,
        user_service: Arc<dyn UserAuthService>,
    ) -> Self {
        Self {
            jwt_encoder,
            token_blacklist,
            user_service,
        }
    }

    /// 验证密码（使用恒定时间比较）
    async fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
        bcrypt::verify(password, hash)
            .map_err(|e| AuthError::InternalError(format!("Password verification failed: {}", e)))
    }

    /// 生成令牌对
    async fn generate_token_pair(
        &self,
        user_id: Uuid,
        username: String,
        email: Option<String>,
        role: UserRole,
        permissions: Vec<Permission>,
    ) -> Result<TokenPair, AuthError> {
        let claims = self.jwt_encoder.generate_claims(
            user_id,
            username.clone(),
            email.clone(),
            role.clone(),
            permissions.clone(),
        );

        let access_token = self.jwt_encoder.generate_access_token(&claims)?;
        let refresh_token = self.jwt_encoder.generate_refresh_token(user_id)?;

        let now = Utc::now();
        let access_token_expires_at =
            now + Duration::minutes(self.jwt_encoder.access_token_expire_minutes());
        let refresh_token_expires_at =
            now + Duration::days(self.jwt_encoder.refresh_token_expire_days());

        Ok(TokenPair {
            access_token,
            refresh_token,
            access_token_expires_at,
            refresh_token_expires_at,
        })
    }
}

#[async_trait]
impl AuthService for JwtAuthServiceImpl {
    async fn login(&self, credentials: LoginCredentials) -> Result<LoginResponse, AuthError> {
        // 验证用户凭证
        let user_info = self
            .user_service
            .authenticate_user(&credentials.username, &credentials.password)
            .await?;

        // 检查用户状态
        if !user_info.is_active {
            return Err(AuthError::UserDisabled);
        }

        // 生成令牌
        let token_pair = self
            .generate_token_pair(
                user_info.id,
                user_info.username.clone(),
                user_info.email.clone(),
                user_info.role.clone(),
                user_info.permissions.clone(),
            )
            .await?;

        // 更新用户最后登录时间
        self.user_service.update_last_login(user_info.id).await?;

        info!("User {} logged in successfully", credentials.username);

        Ok(LoginResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_encoder.access_token_expire_minutes() * 60,
            user: UserInfo {
                id: user_info.id,
                username: user_info.username,
                email: user_info.email,
                role: user_info.role,
                permissions: user_info.permissions,
                created_at: user_info.created_at,
                last_login: Some(Utc::now()),
            },
        })
    }

    async fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        // 解码令牌
        let claims = self.jwt_encoder.decode_token(token)?;

        // 检查令牌是否被撤销
        if self.token_blacklist.is_blacklisted(&claims.jti).await? {
            return Err(AuthError::TokenRevoked);
        }

        // 检查用户是否仍然有效
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidToken)?;

        if !self.user_service.is_user_active(user_id).await? {
            return Err(AuthError::UserDisabled);
        }

        debug!("Token validated successfully for user {}", claims.sub);
        Ok(claims)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<LoginResponse, AuthError> {
        // 验证刷新令牌
        let user_id = self
            .user_service
            .validate_refresh_token(refresh_token)
            .await?;

        // 获取用户信息
        let user_info = self.user_service.get_user_by_id(user_id).await?;

        if !user_info.is_active {
            return Err(AuthError::UserDisabled);
        }

        // 生成新的令牌对
        let token_pair = self
            .generate_token_pair(
                user_info.id,
                user_info.username.clone(),
                user_info.email.clone(),
                user_info.role.clone(),
                user_info.permissions.clone(),
            )
            .await?;

        // 撤销旧的刷新令牌
        self.user_service
            .revoke_refresh_token(refresh_token)
            .await?;

        info!(
            "Refresh token used successfully for user {}",
            user_info.username
        );

        Ok(LoginResponse {
            access_token: token_pair.access_token,
            refresh_token: token_pair.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.jwt_encoder.access_token_expire_minutes() * 60,
            user: UserInfo {
                id: user_info.id,
                username: user_info.username,
                email: user_info.email,
                role: user_info.role,
                permissions: user_info.permissions,
                created_at: user_info.created_at,
                last_login: user_info.last_login,
            },
        })
    }

    async fn logout(&self, token: &str) -> Result<(), AuthError> {
        // 解码令牌以获取JTI
        let claims = self.jwt_encoder.decode_token(token)?;

        // 添加到黑名单
        let expires_at = DateTime::from_timestamp(claims.exp, 0)
            .ok_or_else(|| AuthError::InternalError("Invalid expiration timestamp".to_string()))?;

        self.token_blacklist
            .add_to_blacklist(&claims.jti, expires_at)
            .await?;

        // 撤销用户的所有刷新令牌
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidToken)?;

        self.user_service.revoke_all_refresh_tokens(user_id).await?;

        info!("User {} logged out successfully", claims.sub);
        Ok(())
    }

    async fn has_permission(
        &self,
        user_id: Uuid,
        permission: Permission,
    ) -> Result<bool, AuthError> {
        let user_info = self.user_service.get_user_by_id(user_id).await?;
        Ok(PermissionChecker::check_permission(
            &user_info.permissions,
            permission,
        ))
    }

    async fn has_all_permissions(
        &self,
        user_id: Uuid,
        permissions: &[Permission],
    ) -> Result<bool, AuthError> {
        let user_info = self.user_service.get_user_by_id(user_id).await?;
        Ok(PermissionChecker::check_all_permissions(
            &user_info.permissions,
            permissions,
        ))
    }

    async fn has_any_permission(
        &self,
        user_id: Uuid,
        permissions: &[Permission],
    ) -> Result<bool, AuthError> {
        let user_info = self.user_service.get_user_by_id(user_id).await?;
        Ok(PermissionChecker::check_any_permission(
            &user_info.permissions,
            permissions,
        ))
    }

    async fn get_user_info(&self, user_id: Uuid) -> Result<UserInfo, AuthError> {
        let user_info = self.user_service.get_user_by_id(user_id).await?;

        Ok(UserInfo {
            id: user_info.id,
            username: user_info.username,
            email: user_info.email,
            role: user_info.role,
            permissions: user_info.permissions,
            created_at: user_info.created_at,
            last_login: user_info.last_login,
        })
    }

    async fn revoke_user_tokens(&self, user_id: Uuid) -> Result<(), AuthError> {
        self.user_service.revoke_all_refresh_tokens(user_id).await?;
        info!("All tokens revoked for user {}", user_id);
        Ok(())
    }

    async fn is_token_revoked(&self, jti: &str) -> Result<bool, AuthError> {
        self.token_blacklist.is_blacklisted(jti).await
    }
}

/// 用户认证服务接口（需要application层实现）
#[async_trait]
pub trait UserAuthService: Send + Sync {
    /// 验证用户凭证
    async fn authenticate_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<UserAuthInfo, AuthError>;

    /// 验证刷新令牌
    async fn validate_refresh_token(&self, refresh_token: &str) -> Result<Uuid, AuthError>;

    /// 撤销刷新令牌
    async fn revoke_refresh_token(&self, refresh_token: &str) -> Result<(), AuthError>;

    /// 撤销用户的所有刷新令牌
    async fn revoke_all_refresh_tokens(&self, user_id: Uuid) -> Result<(), AuthError>;

    /// 获取用户信息
    async fn get_user_by_id(&self, user_id: Uuid) -> Result<UserAuthInfo, AuthError>;

    /// 检查用户是否活跃
    async fn is_user_active(&self, user_id: Uuid) -> Result<bool, AuthError>;

    /// 更新用户最后登录时间
    async fn update_last_login(&self, user_id: Uuid) -> Result<(), AuthError>;
}

/// 用户认证信息
#[derive(Clone)]
pub struct UserAuthInfo {
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
    /// 是否活跃
    pub is_active: bool,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后登录时间
    pub last_login: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_blacklist() {
        let blacklist = InMemoryTokenBlacklistService::new();
        let jti = "test_token_123";

        // 测试添加到黑名单
        let expires_at = Utc::now() + chrono::Duration::hours(1);
        blacklist.add_to_blacklist(jti, expires_at).await.unwrap();

        // 测试检查黑名单
        assert!(blacklist.is_blacklisted(jti).await.unwrap());

        // 测试从黑名单移除
        blacklist.remove_from_blacklist(jti).await.unwrap();
        assert!(!blacklist.is_blacklisted(jti).await.unwrap());
    }

    #[tokio::test]
    async fn test_blacklist_cleanup() {
        let blacklist = InMemoryTokenBlacklistService::new();

        // 直接访问黑名单来添加令牌，避免触发自动清理
        {
            let mut blacklist_data = blacklist.blacklist.write().await;

            // 添加已过期的令牌
            let expired_jti = "expired_token";
            let past_time = Utc::now() - chrono::Duration::hours(1);
            blacklist_data.insert(expired_jti.to_string(), past_time);

            // 添加未过期的令牌
            let valid_jti = "valid_token";
            let future_time = Utc::now() + chrono::Duration::hours(1);
            blacklist_data.insert(valid_jti.to_string(), future_time);
        }

        // 清理过期令牌
        let cleaned_count = blacklist.cleanup_expired_tokens().await.unwrap();
        assert_eq!(cleaned_count, 1);

        // 验证结果
        assert!(!blacklist.is_blacklisted("expired_token").await.unwrap());
        assert!(blacklist.is_blacklisted("valid_token").await.unwrap());
    }
}
