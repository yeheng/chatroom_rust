use std::error::Error as StdError;

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
    #[error("infrastructure error: {message}")]
    Infrastructure {
        message: String,
        #[source]
        source: Option<Box<dyn StdError + Send + Sync>>,
    },
    #[error("authentication failed")]
    Authentication,
    #[error("authorization failed")]
    Authorization,
}

impl ApplicationError {
    /// 创建基础设施错误
    pub fn infrastructure(message: impl Into<String>) -> Self {
        ApplicationError::Infrastructure {
            message: message.into(),
            source: None,
        }
    }

    pub fn infrastructure_with_source(
        message: impl Into<String>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        ApplicationError::Infrastructure {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(value: RepositoryError) -> Self {
        ApplicationError::Repository(value)
    }
}
