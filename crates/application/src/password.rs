use async_trait::async_trait;
use domain::PasswordHash;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasswordHasherError {
    #[error("hash error: {0}")]
    Hash(String),
    #[error("verify error: {0}")]
    Verify(String),
}

impl PasswordHasherError {
    pub fn hash_error(message: impl Into<String>) -> Self {
        Self::Hash(message.into())
    }

    pub fn verify_error(message: impl Into<String>) -> Self {
        Self::Verify(message.into())
    }
}

#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash(&self, plaintext: &str) -> Result<PasswordHash, PasswordHasherError>;
    async fn verify(
        &self,
        plaintext: &str,
        hashed: &PasswordHash,
    ) -> Result<bool, PasswordHasherError>;
}
