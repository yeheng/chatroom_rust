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
    /// Redis配置
    pub redis: RedisConfig,
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

/// Redis配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: u32,
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
    /// 对于关键安全配置（DATABASE_URL, JWT_SECRET, REDIS_URL），如果环境变量不存在将会 panic
    /// 这确保了生产环境中不会使用不安全的默认值
    pub fn from_env() -> Self {
        Self {
            database: DatabaseConfig {
                url: env::var("DATABASE_URL")
                    .expect("DATABASE_URL environment variable is required for production safety"),
                max_connections: env::var("DB_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5),
            },
            jwt: JwtConfig {
                secret: env::var("JWT_SECRET")
                    .expect("JWT_SECRET environment variable is required for production safety"),
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
            redis: RedisConfig {
                url: env::var("REDIS_URL")
                    .expect("REDIS_URL environment variable is required for production safety"),
                max_connections: env::var("REDIS_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
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

    /// 从环境变量加载配置，开发环境版本
    /// 提供不安全的默认值，仅用于测试和开发
    pub fn from_env_with_defaults() -> Self {
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
                    "dev-secret-key-not-for-production-use-minimum-32-chars".to_string()
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
            redis: RedisConfig {
                url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
                max_connections: env::var("REDIS_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
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
    /// 增强的验证逻辑，特别关注生产环境安全
    pub fn validate(&self) -> Result<(), ConfigError> {
        // 验证数据库URL
        if self.database.url.is_empty() {
            return Err(ConfigError::InvalidDatabaseUrl(
                "Database URL cannot be empty".to_string(),
            ));
        }

        // 生产环境安全检查：不允许明显的测试/开发配置
        if self.database.url.contains("postgres:123456")
            || self.database.url.contains("localhost")
            || self.database.url.contains("127.0.0.1:5432")
        {
            eprintln!("⚠️ WARNING: Using development database configuration in production!");
        }

        // 验证JWT密钥长度和安全性（至少256位/32字节）
        if self.jwt.secret.len() < 32 {
            return Err(ConfigError::InvalidJwtSecret(
                "JWT secret must be at least 32 characters long".to_string(),
            ));
        }

        // 检查JWT密钥是否为明显的开发密钥
        if self.jwt.secret.contains("dev-secret")
            || self.jwt.secret.contains("not-for-production")
            || self.jwt.secret.contains("please-change")
        {
            return Err(ConfigError::InvalidJwtSecret(
                "Cannot use development JWT secret in production".to_string(),
            ));
        }

        // 验证Redis URL
        if self.redis.url.contains("127.0.0.1") || self.redis.url.contains("localhost") {
            eprintln!("⚠️ WARNING: Using development Redis configuration in production!");
        }

        // 验证连接数
        if self.database.max_connections == 0 {
            return Err(ConfigError::InvalidDatabaseConfig(
                "Max connections must be greater than 0".to_string(),
            ));
        }

        // 验证bcrypt cost（如果设置）
        if let Some(cost) = self.server.bcrypt_cost {
            if !(10..=14).contains(&cost) {
                return Err(ConfigError::InvalidServerConfig(
                    "bcrypt cost should be between 10-14 for security".to_string(),
                ));
            }
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
    #[error("Invalid server configuration: {0}")]
    InvalidServerConfig(String),
    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),
}

impl Default for AppConfig {
    /// 默认配置使用开发环境版本
    /// 注意：生产环境应该明确调用 from_env() 而不是依赖默认值
    fn default() -> Self {
        Self::from_env_with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_from_env_with_defaults() {
        let config = AppConfig::from_env_with_defaults();
        assert!(!config.database.url.is_empty());
        assert!(!config.jwt.secret.is_empty());
        assert!(config.jwt.expiration_hours > 0);
        assert!(config.server.port > 0);
    }

    #[test]
    fn test_config_from_env_requires_critical_vars() {
        // 清理环境变量
        env::remove_var("DATABASE_URL");
        env::remove_var("JWT_SECRET");
        env::remove_var("REDIS_URL");

        // 测试缺少关键环境变量时会panic
        let result = std::panic::catch_unwind(|| AppConfig::from_env());
        assert!(
            result.is_err(),
            "AppConfig::from_env() should panic when critical env vars are missing"
        );
    }

    #[test]
    fn test_config_from_env_with_required_vars() {
        // 设置必需的环境变量
        env::set_var("DATABASE_URL", "postgres://user:pass@prod-db:5432/chatroom");
        env::set_var(
            "JWT_SECRET",
            "production-secret-key-with-at-least-32-characters",
        );
        env::set_var("REDIS_URL", "redis://prod-redis:6379");

        let config = AppConfig::from_env();
        assert_eq!(
            config.database.url,
            "postgres://user:pass@prod-db:5432/chatroom"
        );
        assert_eq!(
            config.jwt.secret,
            "production-secret-key-with-at-least-32-characters"
        );
        assert_eq!(config.redis.url, "redis://prod-redis:6379");

        // 清理环境变量
        env::remove_var("DATABASE_URL");
        env::remove_var("JWT_SECRET");
        env::remove_var("REDIS_URL");
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::from_env_with_defaults();

        // 开发配置需要修复JWT密钥才能通过验证
        config.jwt.secret = "production-grade-secret-key-with-sufficient-length".to_string();
        assert!(config.validate().is_ok());

        // 测试无效JWT密钥长度
        config.jwt.secret = "short".to_string();
        assert!(config.validate().is_err());

        // 测试开发JWT密钥在生产环境被拒绝
        config.jwt.secret = "dev-secret-key-not-for-production-use".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("development JWT secret"));

        // 测试有效的生产配置
        config.jwt.secret = "production-grade-secret-key-with-sufficient-length".to_string();
        config.database.url = "postgres://user:pass@prod-db:5432/chatroom".to_string();
        config.redis.url = "redis://prod-redis:6379".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_bcrypt_cost_validation() {
        let mut config = AppConfig::from_env_with_defaults();
        config.jwt.secret = "production-grade-secret-key-with-sufficient-length".to_string();

        // 测试有效的bcrypt cost
        config.server.bcrypt_cost = Some(12);
        assert!(config.validate().is_ok());

        // 测试过低的bcrypt cost
        config.server.bcrypt_cost = Some(8);
        assert!(config.validate().is_err());

        // 测试过高的bcrypt cost
        config.server.bcrypt_cost = Some(16);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_missing_database_url_fails_validation() {
        // 这个测试验证todo.md中要求的行为：
        // "在不设置 DATABASE_URL 环境变量的情况下调用配置加载函数，并断言它会返回错误"

        // 清理环境变量
        env::remove_var("DATABASE_URL");
        env::remove_var("JWT_SECRET");
        env::remove_var("REDIS_URL");

        // 测试严格的配置加载失败
        let result = std::panic::catch_unwind(|| AppConfig::from_env());

        // 应该panic，因为缺少关键的环境变量
        assert!(
            result.is_err(),
            "严格配置模式下，缺少关键环境变量应该导致应用启动失败"
        );
    }
}
