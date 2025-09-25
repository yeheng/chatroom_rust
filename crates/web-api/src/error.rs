use application::ApplicationError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            body: ErrorBody {
                code,
                message: message.into(),
            },
        }
    }

    // 添加便利方法
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", message)
    }

    pub fn internal_server_error(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", message)
    }
}

impl From<ApplicationError> for ApiError {
    fn from(error: ApplicationError) -> Self {
        use application::ApplicationError as AppErr;
        use domain::DomainError;

        match error {
            AppErr::Domain(DomainError::InvalidArgument { field, reason }) => ApiError::new(
                StatusCode::BAD_REQUEST,
                "INVALID_ARGUMENT",
                format!("{}: {}", field, reason),
            ),
            AppErr::Domain(DomainError::UserAlreadyExists) => {
                ApiError::new(StatusCode::CONFLICT, "USER_EXISTS", "user already exists")
            }
            AppErr::Domain(DomainError::UserNotFound) => {
                ApiError::new(StatusCode::NOT_FOUND, "USER_NOT_FOUND", "user not found")
            }
            AppErr::Domain(DomainError::RoomNotFound) => {
                ApiError::new(StatusCode::NOT_FOUND, "ROOM_NOT_FOUND", "room not found")
            }
            AppErr::Domain(DomainError::MessageNotFound) => ApiError::new(
                StatusCode::NOT_FOUND,
                "MESSAGE_NOT_FOUND",
                "message not found",
            ),
            AppErr::Domain(DomainError::UserAlreadyInRoom) => ApiError::new(
                StatusCode::CONFLICT,
                "MEMBERSHIP_EXISTS",
                "user already joined room",
            ),
            AppErr::Domain(DomainError::UserNotInRoom) => {
                ApiError::new(StatusCode::FORBIDDEN, "NOT_ROOM_MEMBER", "user not in room")
            }
            AppErr::Domain(DomainError::RoomIsPrivate) => ApiError::new(
                StatusCode::FORBIDDEN,
                "ROOM_PRIVATE",
                "room requires password",
            ),
            AppErr::Domain(DomainError::RoomClosed) => {
                ApiError::new(StatusCode::FORBIDDEN, "ROOM_CLOSED", "room is closed")
            }
            AppErr::Domain(DomainError::OperationNotAllowed) => ApiError::new(
                StatusCode::FORBIDDEN,
                "OPERATION_NOT_ALLOWED",
                "operation not allowed",
            ),
            AppErr::Repository(_) | AppErr::Password(_) | AppErr::Broadcast(_) => ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal server error",
            ),
            AppErr::Authentication => ApiError::new(
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_FAILED",
                "authentication failed",
            ),
            AppErr::Authorization => ApiError::new(
                StatusCode::FORBIDDEN,
                "AUTHORIZATION_FAILED",
                "authorization failed",
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
