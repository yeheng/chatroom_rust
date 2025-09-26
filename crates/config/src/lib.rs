//! 统一配置中心
//!
//! 提供应用的全局配置管理，包括：
//! - 数据库连接
//! - JWT认证
//! - 消息广播
//! - 服务设置

use serde::{Deserialize, Serialize};
use std::env;

/// 全局应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 数据库配置
    pub database: DatabaseConfig,
    /// JWT认证配置
    pub jwt: JwtConfig,
    /// 广播器配置
    pub broadcast: BroadcastConfig,
    /// 服务配置
    pub server: ServerConfig,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

/// JWT配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub expiration_hours: i64,
}

/// 广播器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastConfig {
    pub capacity: usize,
    pub redis_url: Option<String>,
}

/// 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub bcrypt_cost: Option<u32>,
}

impl AppConfig {
    /// 从环境变量加载配置
    pub fn from_env() -> Self {
        Self {
            database: DatabaseConfig {
                url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                    "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string()
                }),
                max_connections: env::var("DB_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5),
            },
            jwt: JwtConfig {
                secret: env::var("JWT_SECRET").unwrap_or_else(|_| {
                    "your-256-bit-secret-key-here-please-change-in-production".to_string()
                }),
                expiration_hours: env::var("JWT_EXPIRATION_HOURS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(24),
            },
            broadcast: BroadcastConfig {
                capacity: env::var("BROADCAST_CAPACITY")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(256),
                redis_url: env::var("REDIS_URL").ok(),
            },
            server: ServerConfig {
                host: env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("SERVER_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(8080),
                bcrypt_cost: env::var("BCRYPT_COST").ok().and_then(|s| s.parse().ok()),
            },
        }
    }

    /// 验证配置有效性
    pub fn validate(&self) -> Result<(), ConfigError> {
        // 验证数据库URL
        if self.database.url.is_empty() {
            return Err(ConfigError::InvalidDatabaseUrl(
                "Database URL cannot be empty".to_string(),
            ));
        }

        // 验证JWT密钥长度（至少256位/32字节）
        if self.jwt.secret.len() < 32 {
            return Err(ConfigError::InvalidJwtSecret(
                "JWT secret must be at least 32 characters long".to_string(),
            ));
        }

        // 验证端口范围
        // if self.server.port > 65535 {
        //     return Err(ConfigError::InvalidServerPort(
        //         "Port must be between 0-65535".to_string(),
        //     ));
        // }

        // 验证连接数
        if self.database.max_connections == 0 {
            return Err(ConfigError::InvalidDatabaseConfig(
                "Max connections must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// 配置错误类型
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid database URL: {0}")]
    InvalidDatabaseUrl(String),
    #[error("Invalid JWT secret: {0}")]
    InvalidJwtSecret(String),
    #[error("Invalid server port: {0}")]
    InvalidServerPort(String),
    #[error("Invalid database configuration: {0}")]
    InvalidDatabaseConfig(String),
    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        let config = AppConfig::from_env();
        assert!(!config.database.url.is_empty());
        assert!(!config.jwt.secret.is_empty());
        assert!(config.jwt.expiration_hours > 0);
        assert!(config.server.port > 0);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::from_env();

        // 有效配置应该通过验证
        assert!(config.validate().is_ok());

        // 测试无效JWT密钥
        config.jwt.secret = "short".to_string();
        assert!(config.validate().is_err());

        // 测试无效端口需要修改验证逻辑，因为port是u16类型
        // 这里我们暂时跳过这个测试，因为u16类型本身限制了范围
        // 在实际应用中，这种验证应该在配置加载时进行
    }
}
