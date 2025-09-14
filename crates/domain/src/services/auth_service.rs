//! JWT认证服务
//!
//! 定义JWT认证服务的核心接口和实现。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use uuid::Uuid;

use crate::entities::auth::*;

/// JWT认证服务接口
#[async_trait]
pub trait AuthService: Send + Sync {
    /// 用户登录
    async fn login(&self, credentials: LoginCredentials) -> Result<LoginResponse, AuthError>;

    /// 验证访问令牌
    async fn validate_token(&self, token: &str) -> Result<Claims, AuthError>;

    /// 刷新访问令牌
    async fn refresh_token(&self, refresh_token: &str) -> Result<LoginResponse, AuthError>;

    /// 用户登出（撤销令牌）
    async fn logout(&self, token: &str) -> Result<(), AuthError>;

    /// 检查用户是否具有指定权限
    async fn has_permission(
        &self,
        user_id: Uuid,
        permission: Permission,
    ) -> Result<bool, AuthError>;

    /// 检查用户是否具有所有指定权限
    async fn has_all_permissions(
        &self,
        user_id: Uuid,
        permissions: &[Permission],
    ) -> Result<bool, AuthError>;

    /// 检查用户是否具有任一指定权限
    async fn has_any_permission(
        &self,
        user_id: Uuid,
        permissions: &[Permission],
    ) -> Result<bool, AuthError>;

    /// 获取用户信息
    async fn get_user_info(&self, user_id: Uuid) -> Result<UserInfo, AuthError>;

    /// 撤销用户的所有令牌
    async fn revoke_user_tokens(&self, user_id: Uuid) -> Result<(), AuthError>;

    /// 检查令牌是否被撤销
    async fn is_token_revoked(&self, jti: &str) -> Result<bool, AuthError>;
}

/// 令牌黑名单服务接口
#[async_trait]
pub trait TokenBlacklistService: Send + Sync {
    /// 添加令牌到黑名单
    async fn add_to_blacklist(&self, jti: &str, expires_at: DateTime<Utc>)
        -> Result<(), AuthError>;

    /// 检查令牌是否在黑名单中
    async fn is_blacklisted(&self, jti: &str) -> Result<bool, AuthError>;

    /// 从黑名单中移除令牌
    async fn remove_from_blacklist(&self, jti: &str) -> Result<(), AuthError>;

    /// 清理过期的黑名单令牌
    async fn cleanup_expired_tokens(&self) -> Result<usize, AuthError>;
}

/// JWT编码器/解码器
pub struct JwtEncoder {
    /// 私钥（用于签名）
    private_key: Vec<u8>,
    /// 公钥（用于验证）
    public_key: Vec<u8>,
    /// 访问令牌过期时间（分钟）
    access_token_expire_minutes: i64,
    /// 刷新令牌过期时间（天）
    refresh_token_expire_days: i64,
    /// 签发者
    issuer: String,
    /// 受众
    audience: String,
}

impl JwtEncoder {
    /// 创建新的JWT编码器
    pub fn new(
        private_key: Vec<u8>,
        public_key: Vec<u8>,
        access_token_expire_minutes: i64,
        refresh_token_expire_days: i64,
        issuer: String,
        audience: String,
    ) -> Self {
        Self {
            private_key,
            public_key,
            access_token_expire_minutes,
            refresh_token_expire_days,
            issuer,
            audience,
        }
    }

    /// 生成访问令牌
    pub fn generate_access_token(&self, claims: &Claims) -> Result<String, AuthError> {
        // 首先尝试使用 RS256（RSA DER/PEM）
        let header_rs = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let try_rs = jsonwebtoken::encode(&header_rs, claims, &self.encoding_key());
        if let Ok(tok) = try_rs {
            return Ok(tok);
        }

        // 兼容性回退：使用 HS256（对称密钥）
        let mut header_hs = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        header_hs.alg = jsonwebtoken::Algorithm::HS256;
        let enc = jsonwebtoken::EncodingKey::from_secret(&self.private_key);
        jsonwebtoken::encode(&header_hs, claims, &enc).map_err(|e| {
            AuthError::InternalError(format!(
                "Failed to encode access token (HS256 fallback): {}",
                e
            ))
        })
    }

    /// 生成刷新令牌
    pub fn generate_refresh_token(&self, _user_id: Uuid) -> Result<String, AuthError> {
        use uuid::Uuid;

        // 使用UUID v4生成随机的刷新令牌
        let refresh_token = Uuid::new_v4();
        Ok(refresh_token.to_string())
    }

    /// 解码令牌
    pub fn decode_token(&self, token: &str) -> Result<Claims, AuthError> {
        // 先尝试 RS256
        let try_rs = {
            let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
            validation.set_issuer(&[&self.issuer]);
            validation.set_audience(&[&self.audience]);
            validation.validate_exp = true;
            validation.validate_nbf = true;
            jsonwebtoken::decode::<Claims>(token, &self.decoding_key(), &validation)
        };
        if let Ok(data) = try_rs {
            return Ok(data.claims);
        }

        // 回退 HS256
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        // 使用 private_key 作为共享密钥
        let dec = jsonwebtoken::DecodingKey::from_secret(&self.private_key);
        let data = jsonwebtoken::decode::<Claims>(token, &dec, &validation)
            .map_err(|_| AuthError::InvalidToken)?;
        Ok(data.claims)
    }

    /// 生成JWT声明
    pub fn generate_claims(
        &self,
        user_id: Uuid,
        username: String,
        email: Option<String>,
        role: UserRole,
        permissions: Vec<Permission>,
    ) -> Claims {
        use chrono::Duration;

        let now = Utc::now();
        let access_token_expires = now + Duration::minutes(self.access_token_expire_minutes);

        // 生成令牌唯一标识符
        let jti = Uuid::new_v4().to_string();

        Claims {
            sub: user_id.to_string(),
            username,
            email,
            role,
            permissions,
            jti,
            iat: now.timestamp(),
            exp: access_token_expires.timestamp(),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
        }
    }

    /// 获取编码密钥
    fn encoding_key(&self) -> jsonwebtoken::EncodingKey {
        // 同时兼容 DER 和 PEM
        if self.private_key.starts_with(b"-----BEGIN") {
            if let Ok(k) = jsonwebtoken::EncodingKey::from_rsa_pem(&self.private_key) {
                return k;
            }
        }
        jsonwebtoken::EncodingKey::from_rsa_der(&self.private_key)
    }

    /// 获取解码密钥
    fn decoding_key(&self) -> jsonwebtoken::DecodingKey {
        // 同时兼容 DER 和 PEM
        if self.public_key.starts_with(b"-----BEGIN") {
            if let Ok(k) = jsonwebtoken::DecodingKey::from_rsa_pem(&self.public_key) {
                return k;
            }
        }
        jsonwebtoken::DecodingKey::from_rsa_der(&self.public_key)
    }

    /// 获取访问令牌过期时间（分钟）
    pub fn access_token_expire_minutes(&self) -> i64 {
        self.access_token_expire_minutes
    }

    /// 获取刷新令牌过期时间（天）
    pub fn refresh_token_expire_days(&self) -> i64 {
        self.refresh_token_expire_days
    }
}

/// 权限检查器
pub struct PermissionChecker;

impl PermissionChecker {
    /// 检查权限
    pub fn check_permission(
        user_permissions: &[Permission],
        required_permission: Permission,
    ) -> bool {
        user_permissions.contains(&required_permission)
    }

    /// 检查是否具有所有权限
    pub fn check_all_permissions(
        user_permissions: &[Permission],
        required_permissions: &[Permission],
    ) -> bool {
        let user_perm_set: HashSet<_> = user_permissions.iter().collect();
        let required_perm_set: HashSet<_> = required_permissions.iter().collect();

        user_perm_set.is_superset(&required_perm_set)
    }

    /// 检查是否具有任一权限
    pub fn check_any_permission(
        user_permissions: &[Permission],
        required_permissions: &[Permission],
    ) -> bool {
        let user_perm_set: HashSet<_> = user_permissions.iter().collect();

        required_permissions
            .iter()
            .any(|p| user_perm_set.contains(p))
    }

    /// 根据角色获取权限
    pub fn get_permissions_by_role(role: UserRole) -> Vec<Permission> {
        role.default_permissions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_checker() {
        let user_perms = vec![Permission::SendMessage, Permission::JoinRoom];

        assert!(PermissionChecker::check_permission(
            &user_perms,
            Permission::SendMessage
        ));
        assert!(!PermissionChecker::check_permission(
            &user_perms,
            Permission::ManageSystem
        ));

        let required_all = vec![Permission::SendMessage, Permission::JoinRoom];
        assert!(PermissionChecker::check_all_permissions(
            &user_perms,
            &required_all
        ));

        let required_any = vec![Permission::SendMessage, Permission::ManageSystem];
        assert!(PermissionChecker::check_any_permission(
            &user_perms,
            &required_any
        ));
    }

    #[test]
    fn test_role_permissions() {
        let admin_perms = PermissionChecker::get_permissions_by_role(UserRole::Admin);
        assert!(admin_perms.contains(&Permission::ManageSystem));

        let user_perms = PermissionChecker::get_permissions_by_role(UserRole::User);
        assert!(!user_perms.contains(&Permission::ManageSystem));

        let guest_perms = PermissionChecker::get_permissions_by_role(UserRole::Guest);
        assert!(!guest_perms.contains(&Permission::EditMessage));
    }
}
