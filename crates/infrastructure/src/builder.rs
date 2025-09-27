use std::sync::Arc;

use application::{MessageBroadcaster, PasswordHasher};
use config::AppConfig;
use thiserror::Error;

use crate::{
    broadcast::{LocalMessageBroadcaster, RedisMessageBroadcaster},
    migrations::MIGRATOR,
    password::BcryptPasswordHasher,
    repository::{create_pg_pool, PgStorage},
};

#[derive(Debug, Error)]
pub enum InfrastructureError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("configuration error: {0}")]
    Config(String),
}

#[derive(Clone)]
pub struct Infrastructure {
    pub storage: Arc<PgStorage>,
    pub password_hasher: Arc<BcryptPasswordHasher>,
    pub broadcaster: Arc<dyn MessageBroadcaster>,
}

impl Infrastructure {
    pub async fn connect(config: &AppConfig) -> Result<Self, InfrastructureError> {
        let pool = create_pg_pool(&config.database.url, config.database.max_connections).await?;
        MIGRATOR.run(&pool).await?;

        let storage = Arc::new(PgStorage::new(pool));
        let password_hasher = Arc::new(BcryptPasswordHasher::new(config.server.bcrypt_cost));

        // 根据配置选择广播器类型
        let broadcaster: Arc<dyn MessageBroadcaster> = match &config.broadcast.redis_url {
            Some(redis_url) => {
                let client =
                    redis::Client::open(redis_url.clone()).map_err(InfrastructureError::Redis)?;
                Arc::new(RedisMessageBroadcaster::new(client))
            }
            None => Arc::new(LocalMessageBroadcaster::new(config.broadcast.capacity)),
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
