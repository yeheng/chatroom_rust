use std::sync::Arc;

use application::PasswordHasher;
use thiserror::Error;

use crate::{
    broadcast::LocalMessageBroadcaster,
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
}

impl Default for InfrastructureConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://postgres:postgres@127.0.0.1:5432/postgres".to_string(),
            max_connections: 5,
            bcrypt_cost: None,
            broadcast_capacity: 256,
        }
    }
}

#[derive(Debug, Error)]
pub enum InfrastructureError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

#[derive(Clone)]
pub struct Infrastructure {
    pub storage: Arc<PgStorage>,
    pub password_hasher: Arc<BcryptPasswordHasher>,
    pub broadcaster: Arc<LocalMessageBroadcaster>,
}

impl Infrastructure {
    pub async fn connect(config: InfrastructureConfig) -> Result<Self, InfrastructureError> {
        let pool = create_pg_pool(&config.database_url, config.max_connections).await?;
        MIGRATOR.run(&pool).await?;

        let storage = Arc::new(PgStorage::new(pool));
        let password_hasher = Arc::new(BcryptPasswordHasher::new(config.bcrypt_cost));
        let broadcaster = Arc::new(LocalMessageBroadcaster::new(config.broadcast_capacity));

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
