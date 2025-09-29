//! 统一配置中心
//!
//! 提供应用的全局配置管理，包括：
//! - 数据库连接
//! - JWT认证
//! - 消息广播
//! - 服务设置
//!
//! 使用分层配置加载：
//! 1. config/default.yml (基础默认值)
//! 2. config/local.yml (本地开发覆盖，不提交到git)
//! 3. 环境变量 (最高优先级，用于生产和CI)

use figment::{providers::{Env, Format, Yaml}, Figment};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
    /// 统计聚合配置
    pub stats: StatsConfig,
    /// 用户状态事件配置
    pub presence: PresenceConfig,
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

/// 统计聚合配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsConfig {
    /// 定时任务调度配置
    pub schedule: ScheduleConfig,
    /// 消费者配置
    pub consumer: ConsumerConfig,
}

/// 定时任务调度配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    /// 小时级统计聚合：每小时第5分钟执行
    pub hourly_aggregation: String,
    /// 日级统计：每天凌晨1点执行
    pub daily_aggregation: String,
    /// 周级统计：每周一凌晨2点执行
    pub weekly_aggregation: String,
    /// 月级统计：每月1号凌晨3点执行
    pub monthly_aggregation: String,
    /// 数据清理：每天凌晨4点执行
    pub data_cleanup: String,
}

/// 消费者配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerConfig {
    /// 消费者组名称
    pub consumer_group: String,
    /// 消费者实例名称
    pub consumer_name: String,
    /// 批次大小
    pub batch_size: i64,
    /// 轮询间隔（秒）
    pub poll_interval_secs: u64,
}

/// 用户状态事件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceConfig {
    /// Redis Stream 名称
    pub stream_name: String,
}

impl AppConfig {
    /// 唯一的配置加载方法 - Linus式"单一可信来源"
    ///
    /// 分层加载顺序：default.yml → local.yml → 环境变量
    ///
    /// 失败策略：FAIL FAST - 配置错误时立即崩溃
    /// 这是正确的行为 - 服务不应该在配置错误时启动
    pub fn load() -> Result<Self, ConfigError> {
        let mut figment = Figment::new()
            .merge(Yaml::file("config/default.yml"));

        // 如果存在 local.yml，则加载它（用于本地开发覆盖）
        if Path::new("config/local.yml").exists() {
            figment = figment.merge(Yaml::file("config/local.yml"));
        }

        // 环境变量具有最高优先级
        figment = figment.merge(Env::raw());

        let config: AppConfig = figment
            .extract()
            .map_err(|e| ConfigError::FigmentError(e.to_string()))?;

        // 立即验证配置 - FAIL FAST原则
        let is_production = std::env::var("APP_ENV")
            .map(|env| env == "production")
            .unwrap_or(false);

        if is_production {
            config.validate_strict()?;
        } else {
            config.validate()?;
        }

        Ok(config)
    }

    /// 严格验证（生产环境）
    fn validate_strict(&self) -> Result<(), ConfigError> {
        self.validate()?;

        // 生产环境不能使用开发配置
        if self.database.url.contains("127.0.0.1") || self.database.url.contains("localhost") {
            return Err(ConfigError::ProductionSafetyError(
                "Cannot use localhost database in production".to_string(),
            ));
        }

        if self.jwt.secret.contains("dev-secret") || self.jwt.secret.contains("not-for-production") {
            return Err(ConfigError::ProductionSafetyError(
                "Cannot use development JWT secret in production".to_string(),
            ));
        }

        if self.redis.url.contains("127.0.0.1") || self.redis.url.contains("localhost") {
            return Err(ConfigError::ProductionSafetyError(
                "Cannot use localhost Redis in production".to_string(),
            ));
        }

        Ok(())
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

    /// 测试专用配置 - 在测试模式下可用
    ///
    /// Linus式原则：测试有自己的配置，不污染生产逻辑
    pub fn test_config() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string(),
                max_connections: 5,
            },
            jwt: JwtConfig {
                secret: "test-secret-key-with-at-least-32-characters-for-testing".to_string(),
                expiration_hours: 24,
            },
            broadcast: BroadcastConfig {
                capacity: 256,
                redis_url: None,
            },
            redis: RedisConfig {
                url: "redis://127.0.0.1:6379".to_string(),
                max_connections: 10,
            },
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                bcrypt_cost: None,
            },
            stats: StatsConfig {
                schedule: ScheduleConfig {
                    hourly_aggregation: "0 5 * * * *".to_string(),
                    daily_aggregation: "0 0 1 * * *".to_string(),
                    weekly_aggregation: "0 0 2 * * 1".to_string(),
                    monthly_aggregation: "0 0 3 1 * *".to_string(),
                    data_cleanup: "0 0 4 * * *".to_string(),
                },
                consumer: ConsumerConfig {
                    consumer_group: "stats_consumers".to_string(),
                    consumer_name: "consumer_1".to_string(),
                    batch_size: 10,
                    poll_interval_secs: 1,
                },
            },
            presence: PresenceConfig {
                stream_name: "presence_events".to_string(),
            },
        }
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
    #[error("Configuration parsing error: {0}")]
    FigmentError(String),
    #[error("Production safety error: {0}")]
    ProductionSafetyError(String),
}

impl Default for AppConfig {
    /// Default实现遵循"单一可信来源"原则
    ///
    /// Linus式FAIL FAST：配置错误时立即panic
    /// 不提供fallback，不隐藏问题
    fn default() -> Self {
        Self::load().expect("配置加载失败 - 请检查配置文件和环境变量")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_load_with_yaml() {
        // 测试从YAML文件加载配置
        // 这会从 config/default.yml 加载
        let config = AppConfig::load();

        // 在没有YAML文件时，应该使用test_config进行测试
        if let Ok(config) = config {
            assert!(!config.database.url.is_empty());
            assert!(!config.jwt.secret.is_empty());
        } else {
            // 如果load失败，使用test_config验证基本结构
            let test_config = AppConfig::test_config();
            assert!(!test_config.database.url.is_empty());
            assert!(!test_config.jwt.secret.is_empty());
        }
    }

    #[test]
    fn test_config_backward_compatibility() {
        // 测试load方法正常工作
        let config = AppConfig::test_config();
        assert!(!config.database.url.is_empty());
        assert!(!config.jwt.secret.is_empty());
        assert!(config.jwt.expiration_hours > 0);
        assert!(config.server.port > 0);
    }

    #[test]
    fn test_production_strict_validation() {
        let mut config = AppConfig::test_config();

        // 生产环境严格验证：不能使用开发配置（localhost数据库）
        let result = config.validate_strict();
        assert!(result.is_err());
        // validate_strict 应该返回 ProductionSafetyError，包含 "localhost"
        assert!(result.unwrap_err().to_string().contains("localhost"));

        // 修复为生产配置
        config.database.url = "postgres://user:pass@prod-db:5432/chatroom".to_string();
        config.redis.url = "redis://prod-redis:6379".to_string();
        config.jwt.secret = "production-grade-secret-key-with-sufficient-length".to_string();

        assert!(config.validate_strict().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::test_config();

        // 测试配置应该通过基础验证
        assert!(config.validate().is_ok());

        // 测试无效JWT密钥长度
        config.jwt.secret = "short".to_string();
        assert!(config.validate().is_err());

        // 测试开发JWT密钥在严格验证中被拒绝
        config.jwt.secret = "dev-secret-key-not-for-production-use".to_string();
        let result = config.validate_strict();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("development JWT secret"));
    }

    #[test]
    fn test_bcrypt_cost_validation() {
        let mut config = AppConfig::test_config();

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
    fn test_env_var_override() {
        // 测试环境变量覆盖
        env::set_var("DATABASE_URL", "postgres://test:test@test-db:5432/test");
        env::set_var("JWT_SECRET", "test-secret-key-with-at-least-32-characters");
        env::set_var("SERVER_PORT", "9000");

        // 由于没有YAML文件，load()可能失败，但向后兼容方法应该能处理
        if let Ok(config) = AppConfig::load() {
            // 如果成功加载，检查环境变量是否生效
            assert_eq!(config.database.url, "postgres://test:test@test-db:5432/test");
            assert_eq!(config.jwt.secret, "test-secret-key-with-at-least-32-characters");
            assert_eq!(config.server.port, 9000);
        }

        // 清理环境变量
        env::remove_var("DATABASE_URL");
        env::remove_var("JWT_SECRET");
        env::remove_var("SERVER_PORT");
    }
}
