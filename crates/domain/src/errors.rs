use std::{error::Error as StdError, fmt, sync::Arc};

use thiserror::Error;

/// 领域错误定义。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid {field}: {reason}")]
    InvalidArgument {
        field: &'static str,
        reason: &'static str,
    },
    #[error("user already exists")]
    UserAlreadyExists,
    #[error("user not found")]
    UserNotFound,
    #[error("room not found")]
    RoomNotFound,
    #[error("message not found")]
    MessageNotFound,
    #[error("user already in room")]
    UserAlreadyInRoom,
    #[error("user not in room")]
    UserNotInRoom,
    #[error("room is private")]
    RoomIsPrivate,
    #[error("room is closed")]
    RoomClosed,
    #[error("insufficient permissions")]
    InsufficientPermissions,
    #[error("operation not allowed")]
    OperationNotAllowed,
}

impl DomainError {
    pub fn invalid_argument(field: &'static str, reason: &'static str) -> Self {
        Self::InvalidArgument { field, reason }
    }
}

#[derive(Debug, Error, Clone)]
pub enum RepositoryError {
    #[error("entity not found")]
    NotFound,
    #[error("entity already exists")]
    Conflict,
    #[error("storage error: {message}")]
    Storage {
        message: String,
        #[source]
        source: Option<RepositoryErrorSource>,
    },
}

impl RepositoryError {
    pub fn storage(message: impl Into<String>) -> Self {
        RepositoryError::Storage {
            message: message.into(),
            source: None,
        }
    }

    pub fn storage_with_source<E>(message: impl Into<String>, source: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        RepositoryError::Storage {
            message: message.into(),
            source: Some(RepositoryErrorSource::new(source)),
        }
    }
}

#[derive(Clone)]
pub struct RepositoryErrorSource(Arc<dyn StdError + Send + Sync>);

impl RepositoryErrorSource {
    pub fn new<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self(Arc::new(error))
    }
}

impl fmt::Debug for RepositoryErrorSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Display for RepositoryErrorSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl StdError for RepositoryErrorSource {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

impl From<DomainError> for RepositoryError {
    fn from(value: DomainError) -> Self {
        match value {
            DomainError::UserNotFound
            | DomainError::RoomNotFound
            | DomainError::MessageNotFound
            | DomainError::UserNotInRoom => RepositoryError::NotFound,
            DomainError::UserAlreadyExists | DomainError::UserAlreadyInRoom => {
                RepositoryError::Conflict
            }
            other => RepositoryError::storage(other.to_string()),
        }
    }
}
