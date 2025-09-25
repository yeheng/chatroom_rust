use std::sync::Arc;

use application::PasswordHasher;
use thiserror::Error;

use crate::{
    broadcast::{LocalMessageBroadcaster, RedisMessageBroadcaster},
    migrations::MIGRATOR,
    password::BcryptPasswordHasher,
    repository::{create_pg_pool, PgStorage},
};

#[derive(Debug, Clone)]
pub struct InfrastructureConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub bcrypt_cost: Option<u32>,
    pub broadcast_capacity: usize,
    pub redis_url: Option<String>, // Redis 配置，None 表示使用本地广播器
}

impl Default for InfrastructureConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://postgres:postgres@127.0.0.1:5432/postgres".to_string(),
            max_connections: 5,
            bcrypt_cost: None,
            broadcast_capacity: 256,
            redis_url: None, // 默认使用本地广播器
        }
    }
}

#[derive(Debug, Error)]
pub enum InfrastructureError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
}

use crate::broadcast::BroadcasterType;

#[derive(Clone)]
pub struct Infrastructure {
    pub storage: Arc<PgStorage>,
    pub password_hasher: Arc<BcryptPasswordHasher>,
    pub broadcaster: BroadcasterType, // 使用枚举支持多种广播器
}

impl Infrastructure {
    pub async fn connect(config: InfrastructureConfig) -> Result<Self, InfrastructureError> {
        let pool = create_pg_pool(&config.database_url, config.max_connections).await?;
        MIGRATOR.run(&pool).await?;

        let storage = Arc::new(PgStorage::new(pool));
        let password_hasher = Arc::new(BcryptPasswordHasher::new(config.bcrypt_cost));

        // 根据配置选择广播器类型
        let broadcaster = match config.redis_url {
            Some(redis_url) => {
                let client = redis::Client::open(redis_url)?;
                BroadcasterType::Redis(Arc::new(RedisMessageBroadcaster::new(client)))
            }
            None => BroadcasterType::Local(Arc::new(LocalMessageBroadcaster::new(
                config.broadcast_capacity,
            ))),
        };

        Ok(Self {
            storage,
            password_hasher,
            broadcaster,
        })
    }
}

impl Infrastructure {
    pub fn password_hasher_trait(&self) -> Arc<dyn PasswordHasher> {
        self.password_hasher.clone()
    }
}
