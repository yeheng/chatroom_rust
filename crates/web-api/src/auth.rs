//! JWT 认证和授权模块
//!
//! 提供 JWT token 生成、验证

use axum::http::HeaderMap;
use config::JwtConfig;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

/// JWT Claims 结构
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub user_id: Uuid,
    pub exp: i64, // 过期时间 (Unix timestamp)
}

/// JWT Token 服务
#[derive(Clone)]
pub struct JwtService {
    config: JwtConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtService {
    pub fn new(config: JwtConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.secret.as_ref());
        let decoding_key = DecodingKey::from_secret(config.secret.as_ref());

        Self {
            config,
            encoding_key,
            decoding_key,
        }
    }

    /// 生成 JWT token
    pub fn generate_token(&self, user_id: Uuid) -> Result<String, ApiError> {
        let now = chrono::Utc::now();
        let exp = now + chrono::Duration::hours(self.config.expiration_hours);

        let claims = Claims {
            user_id,
            exp: exp.timestamp(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|err| ApiError::unauthorized(format!("Token generation failed: {}", err)))
    }

    /// 验证并解析 JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims, ApiError> {
        decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map(|token_data| token_data.claims)
            .map_err(|err| ApiError::unauthorized(format!("Invalid token: {}", err)))
    }

    /// 从 headers 中提取和验证 token
    pub fn extract_user_from_headers(&self, headers: &HeaderMap) -> Result<Uuid, ApiError> {
        let auth_header = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("Missing authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::unauthorized("Invalid authorization header format"))?;

        let claims = self.verify_token(token)?;
        Ok(claims.user_id)
    }
}

/// 登录响应结构
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub user: domain::User,
    pub token: String,
}
