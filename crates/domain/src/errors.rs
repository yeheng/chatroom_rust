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
    #[error("operation not allowed")]
    OperationNotAllowed,
}

impl DomainError {
    pub fn invalid_argument(field: &'static str, reason: &'static str) -> Self {
        Self::InvalidArgument { field, reason }
    }
}

/// 仓储错误。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    #[error("entity not found")]
    NotFound,
    #[error("entity already exists")]
    Conflict,
    #[error("storage error: {message}")]
    Storage { message: String },
}

impl RepositoryError {
    pub fn storage(message: impl Into<String>) -> Self {
        RepositoryError::Storage {
            message: message.into(),
        }
    }
}

impl From<DomainError> for RepositoryError {
    fn from(value: DomainError) -> Self {
        match value {
            DomainError::UserNotFound
            | DomainError::RoomNotFound
            | DomainError::MessageNotFound
            | DomainError::UserNotInRoom => RepositoryError::NotFound,
            DomainError::UserAlreadyExists | DomainError::UserAlreadyInRoom => RepositoryError::Conflict,
            other => RepositoryError::storage(other.to_string()),
        }
    }
}
