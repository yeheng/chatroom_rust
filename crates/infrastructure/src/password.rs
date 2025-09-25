use application::{password::PasswordHasherError, PasswordHasher};
use async_trait::async_trait;
use bcrypt::{hash, verify, DEFAULT_COST};
use domain::PasswordHash;

#[derive(Clone)]
pub struct BcryptPasswordHasher {
    cost: u32,
}

impl BcryptPasswordHasher {
    pub fn new(cost: Option<u32>) -> Self {
        Self {
            cost: cost.unwrap_or(DEFAULT_COST),
        }
    }
}

#[async_trait]
impl PasswordHasher for BcryptPasswordHasher {
    async fn hash(&self, plaintext: &str) -> Result<PasswordHash, PasswordHasherError> {
        let cost = self.cost;
        let plaintext = plaintext.to_owned();
        let hashed = tokio::task::spawn_blocking(move || hash(plaintext, cost))
            .await
            .map_err(|err| PasswordHasherError::hash_error(err.to_string()))
            .and_then(|res| res.map_err(|err| PasswordHasherError::hash_error(err.to_string())))?;

        PasswordHash::new(hashed).map_err(|err| PasswordHasherError::hash_error(err.to_string()))
    }

    async fn verify(
        &self,
        plaintext: &str,
        hashed: &PasswordHash,
    ) -> Result<bool, PasswordHasherError> {
        let plaintext = plaintext.to_owned();
        let hashed = hashed.as_str().to_owned();
        tokio::task::spawn_blocking(move || verify(plaintext, &hashed))
            .await
            .map_err(|err| PasswordHasherError::verify_error(err.to_string()))
            .and_then(|res| res.map_err(|err| PasswordHasherError::verify_error(err.to_string())))
    }
}

impl Default for BcryptPasswordHasher {
    fn default() -> Self {
        Self::new(Some(DEFAULT_COST))
    }
}
