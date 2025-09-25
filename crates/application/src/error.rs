use domain::{DomainError, RepositoryError};
use thiserror::Error;

use crate::password::PasswordHasherError;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("domain error: {0}")]
    Domain(#[from] DomainError),
    #[error("repository error: {0:?}")]
    Repository(RepositoryError),
    #[error("password error: {0}")]
    Password(#[from] PasswordHasherError),
    #[error("broadcast error: {0}")]
    Broadcast(#[from] crate::broadcaster::BroadcastError),
    #[error("infrastructure error: {0}")]
    Infrastructure(String), // 新增：基础设施错误
    #[error("authentication failed")]
    Authentication,
    #[error("authorization failed")]
    Authorization,
}

impl ApplicationError {
    /// 创建基础设施错误
    pub fn infrastructure(message: String) -> Self {
        ApplicationError::Infrastructure(message)
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(value: RepositoryError) -> Self {
        ApplicationError::Repository(value)
    }
}
