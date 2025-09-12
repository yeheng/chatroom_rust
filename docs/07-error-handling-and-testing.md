# 错误处理和测试策略

本节详细说明系统的错误处理机制和测试策略，包括错误类型定义、错误处理策略、单元测试、集成测试、性能测试等内容。

## 🚨 错误类型定义

### 系统错误层次结构

```rust
// 基础错误类型
#[derive(Debug, thiserror::Error)]
pub enum ChatRoomError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),
    
    #[error("Infrastructure error: {0}")]
    Infrastructure(#[from] InfrastructureError),
    
    #[error("Application error: {0}")]
    Application(#[from] ApplicationError),
    
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
    
    #[error("Authentication error: {0}")]
    Authentication(#[from] AuthError),
    
    #[error("External service error: {0}")]
    External(#[from] ExternalServiceError),
    
    #[error("Unknown error")]
    Unknown,
}

impl From<ChatRoomError> for axum::response::ErrorResponse {
    fn from(err: ChatRoomError) -> Self {
        let status = match &err {
            ChatRoomError::Domain(_) => StatusCode::BAD_REQUEST,
            ChatRoomError::Infrastructure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ChatRoomError::Application(_) => StatusCode::BAD_REQUEST,
            ChatRoomError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ChatRoomError::Authentication(_) => StatusCode::UNAUTHORIZED,
            ChatRoomError::External(_) => StatusCode::SERVICE_UNAVAILABLE,
            ChatRoomError::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
        };
        
        Self::new(status, err.to_string())
    }
}
```

### 领域错误 (DomainError)

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum DomainError {
    #[error("User not found")]
    UserNotFound,
    
    #[error("User already exists")]
    UserAlreadyExists,
    
    #[error("Chat room not found")]
    RoomNotFound,
    
    #[error("User already in room")]
    UserAlreadyInRoom,
    
    #[error("User not in room")]
    UserNotInRoom,
    
    #[error("Room is full")]
    RoomIsFull,
    
    #[error("Room is private")]
    RoomIsPrivate,
    
    #[error("Invalid password")]
    InvalidPassword,
    
    #[error("Message not found")]
    MessageNotFound,
    
    #[error("Organization not found")]
    OrganizationNotFound,
    
    #[error("Role not found")]
    RoleNotFound,
    
    #[error("Department not found")]
    DepartmentNotFound,
    
    #[error("Position not found")]
    PositionNotFound,
    
    #[error("User already has role in organization")]
    UserAlreadyHasRole,
    
    #[error("Invalid credentials")]
    InvalidCredentials,
    
    #[error("User not active")]
    UserNotActive,
    
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),
    
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Permission denied")]
    PermissionDenied,
    
    #[error("Operation not allowed")]
    OperationNotAllowed,
    
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Quota exceeded")]
    QuotaExceeded,
    
    #[error("Maintenance mode")]
    MaintenanceMode,
}

impl DomainError {
    pub fn error_code(&self) -> String {
        match self {
            DomainError::UserNotFound => "USER_NOT_FOUND".to_string(),
            DomainError::UserAlreadyExists => "USER_ALREADY_EXISTS".to_string(),
            DomainError::RoomNotFound => "ROOM_NOT_FOUND".to_string(),
            DomainError::UserAlreadyInRoom => "USER_ALREADY_IN_ROOM".to_string(),
            DomainError::UserNotInRoom => "USER_NOT_IN_ROOM".to_string(),
            DomainError::RoomIsFull => "ROOM_IS_FULL".to_string(),
            DomainError::RoomIsPrivate => "ROOM_IS_PRIVATE".to_string(),
            DomainError::InvalidPassword => "INVALID_PASSWORD".to_string(),
            DomainError::MessageNotFound => "MESSAGE_NOT_FOUND".to_string(),
            DomainError::OrganizationNotFound => "ORGANIZATION_NOT_FOUND".to_string(),
            DomainError::RoleNotFound => "ROLE_NOT_FOUND".to_string(),
            DomainError::DepartmentNotFound => "DEPARTMENT_NOT_FOUND".to_string(),
            DomainError::PositionNotFound => "POSITION_NOT_FOUND".to_string(),
            DomainError::UserAlreadyHasRole => "USER_ALREADY_HAS_ROLE".to_string(),
            DomainError::InvalidCredentials => "INVALID_CREDENTIALS".to_string(),
            DomainError::UserNotActive => "USER_NOT_ACTIVE".to_string(),
            DomainError::FeatureNotEnabled(feature) => format!("FEATURE_NOT_ENABLED:{}", feature),
            DomainError::ValidationFailed(msg) => format!("VALIDATION_FAILED:{}", msg),
            DomainError::PermissionDenied => "PERMISSION_DENIED".to_string(),
            DomainError::OperationNotAllowed => "OPERATION_NOT_ALLOWED".to_string(),
            DomainError::InsufficientPrivileges => "INSUFFICIENT_PRIVILEGES".to_string(),
            DomainError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED".to_string(),
            DomainError::QuotaExceeded => "QUOTA_EXCEEDED".to_string(),
            DomainError::MaintenanceMode => "MAINTENANCE_MODE".to_string(),
        }
    }
    
    pub fn http_status(&self) -> StatusCode {
        match self {
            DomainError::UserNotFound
            | DomainError::RoomNotFound
            | DomainError::MessageNotFound
            | DomainError::OrganizationNotFound
            | DomainError::RoleNotFound
            | DomainError::DepartmentNotFound
            | DomainError::PositionNotFound => StatusCode::NOT_FOUND,
            
            DomainError::UserAlreadyExists
            | DomainError::UserAlreadyInRoom
            | DomainError::UserAlreadyHasRole
            | DomainError::InvalidCredentials
            | DomainError::InvalidPassword => StatusCode::CONFLICT,
            
            DomainError::UserNotInRoom
            | DomainError::RoomIsFull
            | DomainError::RoomIsPrivate
            | DomainError::UserNotActive
            | DomainError::OperationNotAllowed
            | DomainError::InsufficientPrivileges => StatusCode::FORBIDDEN,
            
            DomainError::PermissionDenied
            | DomainError::FeatureNotEnabled(_)
            | DomainError::RateLimitExceeded
            | DomainError::QuotaExceeded
            | DomainError::MaintenanceMode => StatusCode::FORBIDDEN,
            
            DomainError::ValidationFailed(_) => StatusCode::UNPROCESSABLE_ENTITY,
        }
    }
}
```

### 基础设施错误 (InfrastructureError)

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum InfrastructureError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Database connection error: {0}")]
    DatabaseConnection(String),
    
    #[error("Database timeout error: {0}")]
    DatabaseTimeout(String),
    
    #[error("Redis error: {0}")]
    Redis(String),
    
    #[error("Redis connection error: {0}")]
    RedisConnection(String),
    
    #[error("Kafka error: {0}")]
    Kafka(String),
    
    #[error("Kafka connection error: {0}")]
    KafkaConnection(String),
    
    #[error("Kafka producer error: {0}")]
    KafkaProducer(String),
    
    #[error("Kafka consumer error: {0}")]
    KafkaConsumer(String),
    
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    #[error("WebSocket connection error: {0}")]
    WebSocketConnection(String),
    
    #[error("File storage error: {0}")]
    FileStorage(String),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    
    #[error("Io error: {0}")]
    Io(String),
    
    #[error("Utf8 error: {0}")]
    Utf8(String),
    
    #[error("Connection not found")]
    ConnectionNotFound,
    
    #[error("Service unavailable")]
    ServiceUnavailable,
    
    #[error("Timeout error")]
    Timeout,
}

impl From<sqlx::Error> for InfrastructureError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(ref db_err) => {
                if db_err.is_constraint_violation() {
                    InfrastructureError::Database(db_err.message().to_string())
                } else if db_err.is_connection_exception() {
                    InfrastructureError::DatabaseConnection(db_err.message().to_string())
                } else {
                    InfrastructureError::Database(err.to_string())
                }
            }
            sqlx::Error::PoolTimedOut => {
                InfrastructureError::DatabaseTimeout("Connection pool timeout".to_string())
            }
            sqlx::Error::PoolClosed => {
                InfrastructureError::DatabaseConnection("Connection pool closed".to_string())
            }
            _ => InfrastructureError::Database(err.to_string()),
        }
    }
}

impl From<redis::RedisError> for InfrastructureError {
    fn from(err: redis::RedisError) -> Self {
        match err.kind() {
            redis::ErrorKind::ConnectionError => {
                InfrastructureError::RedisConnection(err.to_string())
            }
            _ => InfrastructureError::Redis(err.to_string()),
        }
    }
}

impl From<rdkafka::error::KafkaError> for InfrastructureError {
    fn from(err: rdkafka::error::KafkaError) -> Self {
        match err {
            rdkafka::error::KafkaError::ConnectionCreation(_) => {
                InfrastructureError::KafkaConnection(err.to_string())
            }
            rdkafka::error::KafkaError::MessageProduction(_) => {
                InfrastructureError::KafkaProducer(err.to_string())
            }
            rdkafka::error::KafkaError::MessageConsumption(_) => {
                InfrastructureError::KafkaConsumer(err.to_string())
            }
            _ => InfrastructureError::Kafka(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for InfrastructureError {
    fn from(err: serde_json::Error) -> Self {
        if err.is_io() {
            InfrastructureError::Io(err.to_string())
        } else if err.is_syntax() {
            InfrastructureError::Deserialization(err.to_string())
        } else {
            InfrastructureError::Serialization(err.to_string())
        }
    }
}
```

### 验证错误 (ValidationError)

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum ValidationError {
    #[error("Required field missing: {0}")]
    RequiredField(String),
    
    #[error("Invalid email format: {0}")]
    InvalidEmail(String),
    
    #[error("Invalid username: {0}")]
    InvalidUsername(String),
    
    #[error("Invalid password: {0}")]
    InvalidPassword(String),
    
    #[error("Invalid room name: {0}")]
    InvalidRoomName(String),
    
    #[error("Invalid message content: {0}")]
    InvalidMessageContent(String),
    
    #[error("Invalid file format: {0}")]
    InvalidFileFormat(String),
    
    #[error("File size too large: {0} bytes (max: {1} bytes)")]
    FileSizeTooLarge(u64, u64),
    
    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),
    
    #[error("Invalid date format: {0}")]
    InvalidDateFormat(String),
    
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("Invalid phone number: {0}")]
    InvalidPhoneNumber(String),
    
    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),
    
    #[error("Invalid language: {0}")]
    InvalidLanguage(String),
    
    #[error("Invalid role: {0}")]
    InvalidRole(String),
    
    #[error("Invalid permission: {0}")]
    InvalidPermission(String),
    
    #[error("String too long: {0} (max: {1})")]
    StringTooLong(String, usize),
    
    #[error("String too short: {0} (min: {1})")]
    StringTooShort(String, usize),
    
    #[error("Number out of range: {0} (min: {1}, max: {2})")]
    NumberOutOfRange(i64, i64, i64),
    
    #[error("Invalid enum value: {0}")]
    InvalidEnumValue(String),
    
    #[error("Custom validation failed: {0}")]
    CustomValidation(String),
}

impl ValidationError {
    pub fn field(&self) -> String {
        match self {
            ValidationError::InvalidEmail(_) => "email".to_string(),
            ValidationError::InvalidUsername(_) => "username".to_string(),
            ValidationError::InvalidPassword(_) => "password".to_string(),
            ValidationError::InvalidRoomName(_) => "name".to_string(),
            ValidationError::InvalidMessageContent(_) => "content".to_string(),
            ValidationError::InvalidFileFormat(_) => "file".to_string(),
            ValidationError::RequiredField(field) => field.clone(),
            _ => "unknown".to_string(),
        }
    }
    
    pub fn error_code(&self) -> String {
        match self {
            ValidationError::RequiredField(_) => "REQUIRED_FIELD".to_string(),
            ValidationError::InvalidEmail(_) => "INVALID_EMAIL".to_string(),
            ValidationError::InvalidUsername(_) => "INVALID_USERNAME".to_string(),
            ValidationError::InvalidPassword(_) => "INVALID_PASSWORD".to_string(),
            ValidationError::InvalidRoomName(_) => "INVALID_ROOM_NAME".to_string(),
            ValidationError::InvalidMessageContent(_) => "INVALID_MESSAGE_CONTENT".to_string(),
            ValidationError::InvalidFileFormat(_) => "INVALID_FILE_FORMAT".to_string(),
            ValidationError::FileSizeTooLarge(_, _) => "FILE_SIZE_TOO_LARGE".to_string(),
            ValidationError::InvalidUuid(_) => "INVALID_UUID".to_string(),
            ValidationError::InvalidDateFormat(_) => "INVALID_DATE_FORMAT".to_string(),
            ValidationError::InvalidJson(_) => "INVALID_JSON".to_string(),
            ValidationError::InvalidUrl(_) => "INVALID_URL".to_string(),
            ValidationError::InvalidPhoneNumber(_) => "INVALID_PHONE_NUMBER".to_string(),
            ValidationError::InvalidTimezone(_) => "INVALID_TIMEZONE".to_string(),
            ValidationError::InvalidLanguage(_) => "INVALID_LANGUAGE".to_string(),
            ValidationError::InvalidRole(_) => "INVALID_ROLE".to_string(),
            ValidationError::InvalidPermission(_) => "INVALID_PERMISSION".to_string(),
            ValidationError::StringTooLong(_, _) => "STRING_TOO_LONG".to_string(),
            ValidationError::StringTooShort(_, _) => "STRING_TOO_SHORT".to_string(),
            ValidationError::NumberOutOfRange(_, _, _) => "NUMBER_OUT_OF_RANGE".to_string(),
            ValidationError::InvalidEnumValue(_) => "INVALID_ENUM_VALUE".to_string(),
            ValidationError::CustomValidation(_) => "CUSTOM_VALIDATION".to_string(),
        }
    }
}
```

### 认证错误 (AuthError)

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken(String),
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid issuer")]
    InvalidIssuer,
    
    #[error("Invalid audience")]
    InvalidAudience,
    
    #[error("Not before error")]
    NotBeforeError,
    
    #[error("Missing authorization header")]
    MissingAuthorizationHeader,
    
    #[error("Invalid authorization header format")]
    InvalidAuthorizationHeader,
    
    #[error("Invalid bearer token format")]
    InvalidBearerTokenFormat,
    
    #[error("User not found")]
    UserNotFound,
    
    #[error("User disabled")]
    UserDisabled,
    
    #[error("Session expired")]
    SessionExpired,
    
    #[error("Session not found")]
    SessionNotFound,
    
    #[error("Invalid credentials")]
    InvalidCredentials,
    
    #[error("Account locked")]
    AccountLocked,
    
    #[error("Account not verified")]
    AccountNotVerified,
    
    #[error("Too many login attempts")]
    TooManyLoginAttempts,
    
    #[error("Password change required")]
    PasswordChangeRequired,
    
    #[error("Invalid password reset token")]
    InvalidPasswordResetToken,
    
    #[error("Password reset token expired")]
    PasswordResetTokenExpired,
    
    #[error("Invalid verification code")]
    InvalidVerificationCode,
    
    #[error("Verification code expired")]
    VerificationCodeExpired,
    
    #[error("Invalid OTP code")]
    InvalidOtpCode,
    
    #[error("OTP code expired")]
    OtpCodeExpired,
}

impl AuthError {
    pub fn error_code(&self) -> String {
        match self {
            AuthError::InvalidToken(_) => "INVALID_TOKEN".to_string(),
            AuthError::TokenExpired => "TOKEN_EXPIRED".to_string(),
            AuthError::InvalidSignature => "INVALID_SIGNATURE".to_string(),
            AuthError::InvalidIssuer => "INVALID_ISSUER".to_string(),
            AuthError::InvalidAudience => "INVALID_AUDIENCE".to_string(),
            AuthError::NotBeforeError => "NOT_BEFORE_ERROR".to_string(),
            AuthError::MissingAuthorizationHeader => "MISSING_AUTHORIZATION_HEADER".to_string(),
            AuthError::InvalidAuthorizationHeader => "INVALID_AUTHORIZATION_HEADER".to_string(),
            AuthError::InvalidBearerTokenFormat => "INVALID_BEARER_TOKEN_FORMAT".to_string(),
            AuthError::UserNotFound => "USER_NOT_FOUND".to_string(),
            AuthError::UserDisabled => "USER_DISABLED".to_string(),
            AuthError::SessionExpired => "SESSION_EXPIRED".to_string(),
            AuthError::SessionNotFound => "SESSION_NOT_FOUND".to_string(),
            AuthError::InvalidCredentials => "INVALID_CREDENTIALS".to_string(),
            AuthError::AccountLocked => "ACCOUNT_LOCKED".to_string(),
            AuthError::AccountNotVerified => "ACCOUNT_NOT_VERIFIED".to_string(),
            AuthError::TooManyLoginAttempts => "TOO_MANY_LOGIN_ATTEMPTS".to_string(),
            AuthError::PasswordChangeRequired => "PASSWORD_CHANGE_REQUIRED".to_string(),
            AuthError::InvalidPasswordResetToken => "INVALID_PASSWORD_RESET_TOKEN".to_string(),
            AuthError::PasswordResetTokenExpired => "PASSWORD_RESET_TOKEN_EXPIRED".to_string(),
            AuthError::InvalidVerificationCode => "INVALID_VERIFICATION_CODE".to_string(),
            AuthError::VerificationCodeExpired => "VERIFICATION_CODE_EXPIRED".to_string(),
            AuthError::InvalidOtpCode => "INVALID_OTP_CODE".to_string(),
            AuthError::OtpCodeExpired => "OTP_CODE_EXPIRED".to_string(),
        }
    }
    
    pub fn http_status(&self) -> StatusCode {
        match self {
            AuthError::InvalidToken(_)
            | AuthError::InvalidSignature
            | AuthError::InvalidIssuer
            | AuthError::InvalidAudience
            | AuthError::NotBeforeError
            | AuthError::InvalidAuthorizationHeader
            | AuthError::InvalidBearerTokenFormat
            | AuthError::InvalidPasswordResetToken
            | AuthError::InvalidVerificationCode
            | AuthError::InvalidOtpCode => StatusCode::UNAUTHORIZED,
            
            AuthError::TokenExpired
            | AuthError::SessionExpired
            | AuthError::PasswordResetTokenExpired
            | AuthError::VerificationCodeExpired
            | AuthError::OtpCodeExpired => StatusCode::UNAUTHORIZED,
            
            AuthError::MissingAuthorizationHeader => StatusCode::BAD_REQUEST,
            
            AuthError::UserNotFound
            | AuthError::SessionNotFound => StatusCode::NOT_FOUND,
            
            AuthError::UserDisabled
            | AuthError::AccountLocked
            | AuthError::AccountNotVerified
            | AuthError::TooManyLoginAttempts
            | AuthError::PasswordChangeRequired => StatusCode::FORBIDDEN,
            
            AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
        }
    }
}
```

## 🔧 错误处理策略

### 错误处理中间件

```rust
pub async fn error_handler(
    err: axum::Error,
    req: Request,
) -> Result<Response, StatusCode> {
    tracing::error!("Request error: {:?}, path: {}", err, req.uri().path());
    
    // 根据错误类型返回适当的响应
    match err {
        axum::Error::BodyAlreadyExtracted => {
            Ok(Json(ApiResponse::error("Body already extracted".to_string()))
                .into_response())
        }
        axum::Error::BufferedBodySizeExceeded { .. } => {
            Ok(Json(ApiResponse::error("Request body too large".to_string()))
                .into_response())
        }
        axum::Error::FormRejectionFailed(_) => {
            Ok(Json(ApiResponse::error("Invalid form data".to_string()))
                .into_response())
        }
        axum::Error::FailedToBufferBody(_) => {
            Ok(Json(ApiResponse::error("Failed to buffer request body".to_string()))
                .into_response())
        }
        axum::Error::InvalidUri(_) => {
            Ok(Json(ApiResponse::error("Invalid URI".to_string()))
                .into_response())
        }
        axum::Error::MethodNotAllowed(_) => {
            Ok(Json(ApiResponse::error("Method not allowed".to_string()))
                .into_response())
        }
        axum::Error::UnknownBodyError(_) => {
            Ok(Json(ApiResponse::error("Unknown body error".to_string()))
                .into_response())
        }
        axum::Error::UnknownMethod(_) => {
            Ok(Json(ApiResponse::error("Unknown method".to_string()))
                .into_response())
        }
        axum::Error::MissingExtension(_) => {
            Ok(Json(ApiResponse::error("Missing extension".to_string()))
                .into_response())
        }
        axum::Error::MissingHeader(_) => {
            Ok(Json(ApiResponse::error("Missing header".to_string()))
                .into_response())
        }
        _ => {
            Ok(Json(ApiResponse::error("Internal server error".to_string()))
                .into_response())
        }
    }
}

// 全局错误处理
pub async fn global_error_handler(
    err: ChatRoomError,
    req: Request,
) -> Response {
    let request_id = req.extensions().get::<RequestId>().cloned()
        .unwrap_or_else(|| RequestId::new());
    
    let path = req.uri().path();
    let method = req.method().to_string();
    
    // 记录错误日志
    tracing::error!(
        request_id = %request_id,
        method = %method,
        path = %path,
        error = %err,
        "Request failed"
    );
    
    // 构造错误响应
    let error_response = ErrorResponse {
        success: false,
        error: Some(ErrorDetail {
            code: err.error_code(),
            message: err.to_string(),
            details: None,
            timestamp: Utc::now(),
            request_id: request_id.to_string(),
        }),
        data: None,
    };
    
    let status = err.http_status();
    
    // 根据环境决定是否包含错误详情
    #[cfg(debug_assertions)]
    let response = Json(error_response);
    
    #[cfg(not(debug_assertions))]
    let response = match err {
        ChatRoomError::Domain(domain_err) => {
            Json(ErrorResponse {
                success: false,
                error: Some(ErrorDetail {
                    code: domain_err.error_code(),
                    message: domain_err.to_string(),
                    details: None,
                    timestamp: Utc::now(),
                    request_id: request_id.to_string(),
                }),
                data: None,
            })
        }
        _ => {
            Json(ErrorResponse {
                success: false,
                error: Some(ErrorDetail {
                    code: "INTERNAL_SERVER_ERROR".to_string(),
                    message: "Internal server error".to_string(),
                    details: None,
                    timestamp: Utc::now(),
                    request_id: request_id.to_string(),
                }),
                data: None,
            })
        }
    };
    
    (status, response).into_response()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse<T> {
    pub success: bool,
    pub error: Option<ErrorDetail>,
    pub data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
}

#[derive(Debug, Clone, Copy)]
pub struct RequestId(Uuid);

impl RequestId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<RequestId> for HeaderValue {
    fn from(request_id: RequestId) -> Self {
        HeaderValue::from_str(&request_id.to_string()).unwrap()
    }
}
```

### 请求ID中间件

```rust
pub async fn request_id_middleware(
    mut req: Request,
    next: Next,
) -> Response {
    // 生成或从请求头获取请求ID
    let request_id = req.headers()
        .get("X-Request-ID")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .map(RequestId)
        .unwrap_or_else(RequestId::new);
    
    // 将请求ID添加到请求扩展中
    req.extensions_mut().insert(request_id);
    
    // 将请求ID添加到响应头中
    let mut response = next.run(req).await;
    response.headers_mut().insert("X-Request-ID", request_id.into());
    
    response
}
```

### 日志中间件

```rust
pub async fn logging_middleware(
    req: Request,
    next: Next,
) -> Response {
    let request_id = req.extensions().get::<RequestId>().cloned()
        .unwrap_or_else(|| RequestId::new());
    
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();
    
    // 记录请求开始
    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        "Request started"
    );
    
    let response = next.run(req).await;
    
    let duration = start.elapsed();
    let status = response.status();
    
    // 记录请求完成
    tracing::info!(
        request_id = %request_id,
        method = %method,
        uri = %uri,
        status = %status,
        duration_ms = %duration.as_millis(),
        "Request completed"
    );
    
    // 记录错误情况
    if status.is_server_error() {
        tracing::error!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = %duration.as_millis(),
            "Server error occurred"
        );
    } else if status.is_client_error() {
        tracing::warn!(
            request_id = %request_id,
            method = %method,
            uri = %uri,
            status = %status,
            duration_ms = %duration.as_millis(),
            "Client error occurred"
        );
    }
    
    response
}
```

## 🧪 测试策略

### 单元测试

#### 领域层测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use chrono::Utc;

    #[test]
    fn test_user_creation() {
        let user_id = Uuid::new_v4();
        let username = "test_user".to_string();
        let email = "test@example.com".to_string();
        
        let user = User::new(user_id, username.clone(), email.clone(), None).unwrap();
        
        assert_eq!(user.id, user_id);
        assert_eq!(user.username, username);
        assert_eq!(user.email, email);
        assert_eq!(user.status, UserStatus::Active);
        assert!(user.created_at <= Utc::now());
        assert!(user.updated_at <= Utc::now());
    }

    #[test]
    fn test_user_validation() {
        let user_id = Uuid::new_v4();
        
        // 测试空用户名
        let result = User::new(user_id, "".to_string(), "test@example.com".to_string(), None);
        assert!(matches!(result, Err(DomainError::ValidationFailed(_))));
        
        // 测试无效邮箱
        let result = User::new(user_id, "test_user".to_string(), "invalid_email".to_string(), None);
        assert!(matches!(result, Err(DomainError::ValidationFailed(_))));
        
        // 测试有效用户
        let result = User::new(user_id, "test_user".to_string(), "test@example.com".to_string(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chat_room_creation() {
        let room_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let name = "Test Room".to_string();
        
        let room = ChatRoom::new(room_id, name.clone(), None, owner_id, false, None).unwrap();
        
        assert_eq!(room.id, room_id);
        assert_eq!(room.name, name);
        assert_eq!(room.owner_id, owner_id);
        assert!(!room.is_private);
        assert!(room.created_at <= Utc::now());
        assert!(room.updated_at <= Utc::now());
    }

    #[test]
    fn test_message_creation() {
        let message_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let content = "Hello, World!".to_string();
        
        let message = Message::new(message_id, room_id, user_id, content.clone(), MessageType::Text, None).unwrap();
        
        assert_eq!(message.id, message_id);
        assert_eq!(message.room_id, room_id);
        assert_eq!(message.user_id, user_id);
        assert_eq!(message.content, content);
        assert_eq!(message.message_type, MessageType::Text);
        assert!(!message.is_edited);
        assert!(!message.is_deleted);
        assert!(message.created_at <= Utc::now());
        assert!(message.updated_at <= Utc::now());
    }

    #[test]
    fn test_room_membership() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        let mut room = ChatRoom::new(
            room_id,
            "Test Room".to_string(),
            None,
            user_id,
            false,
            None,
        ).unwrap();
        
        // 初始时，只有创建者在房间中
        assert_eq!(room.member_count(), 1);
        assert!(room.is_user_in_room(user_id));
        
        // 添加新成员
        let new_user_id = Uuid::new_v4();
        room.add_user(new_user_id).unwrap();
        
        assert_eq!(room.member_count(), 2);
        assert!(room.is_user_in_room(new_user_id));
        
        // 移除成员
        room.remove_user(new_user_id).unwrap();
        
        assert_eq!(room.member_count(), 1);
        assert!(!room.is_user_in_room(new_user_id));
    }

    #[test]
    fn test_private_room_with_password() {
        let room_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let password = "secret123".to_string();
        let password_hash = hash_password(&password).unwrap();
        
        // 创建私密房间需要密码
        let room = ChatRoom::new(
            room_id,
            "Private Room".to_string(),
            None,
            owner_id,
            true,
            Some(password_hash),
        ).unwrap();
        
        assert!(room.is_private);
        assert!(room.password_hash.is_some());
        
        // 验证密码
        assert!(room.verify_password(&password).unwrap());
        assert!(!room.verify_password("wrong_password").unwrap());
    }
}
```

#### 应用层测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repositories::MockUserRepository;
    use crate::domain::repositories::MockChatRoomRepository;
    use crate::infrastructure::MockEventBus;
    use uuid::Uuid;
    use chrono::Utc;

    #[tokio::test]
    async fn test_register_user() {
        let mut user_repository = MockUserRepository::new();
        let event_bus = MockEventBus::new();
        
        // 配置mock
        user_repository
            .expect_find_by_username()
            .returning(|_| Ok(None));
        
        user_repository
            .expect_find_by_email()
            .returning(|_| Ok(None));
        
        user_repository
            .expect_save()
            .returning(|user| Ok(user.clone()));
        
        let handler = UserCommandHandler::new(
            Arc::new(user_repository),
            Arc::new(MockPasswordService::new()),
            Arc::new(event_bus),
        );
        
        let command = RegisterUserCommand {
            username: "test_user".to_string(),
            email: "test@example.com".to_string(),
            password: "password123".to_string(),
            avatar_url: None,
        };
        
        let result = handler.handle(command).await;
        assert!(result.is_ok());
        
        let user = result.unwrap();
        assert_eq!(user.username, "test_user");
        assert_eq!(user.email, "test@example.com");
    }

    #[tokio::test]
    async fn test_register_duplicate_user() {
        let mut user_repository = MockUserRepository::new();
        let event_bus = MockEventBus::new();
        
        // 配置mock - 用户名已存在
        let existing_user = User::new(
            Uuid::new_v4(),
            "test_user".to_string(),
            "test@example.com".to_string(),
            None,
        ).unwrap();
        
        user_repository
            .expect_find_by_username()
            .returning(move |_| Ok(Some(existing_user.clone())));
        
        let handler = UserCommandHandler::new(
            Arc::new(user_repository),
            Arc::new(MockPasswordService::new()),
            Arc::new(event_bus),
        );
        
        let command = RegisterUserCommand {
            username: "test_user".to_string(),
            email: "new@example.com".to_string(),
            password: "password123".to_string(),
            avatar_url: None,
        };
        
        let result = handler.handle(command).await;
        assert!(matches!(result, Err(DomainError::UserAlreadyExists)));
    }

    #[tokio::test]
    async fn test_send_message() {
        let mut room_repository = MockChatRoomRepository::new();
        let mut message_repository = MockMessageRepository::new();
        let event_bus = MockEventBus::new();
        
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        // 配置mock - 用户在房间中
        room_repository
            .expect_is_user_in_room()
            .returning(|_, _| Ok(true));
        
        message_repository
            .expect_save()
            .returning(|message| Ok(message.clone()));
        
        let handler = ChatRoomCommandHandler::new(
            Arc::new(room_repository),
            Arc::new(message_repository),
            Arc::new(MockUserRepository::new()),
            Arc::new(event_bus),
        );
        
        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "Hello, World!".to_string(),
            message_type: MessageType::Text,
            reply_to_message_id: None,
        };
        
        let result = handler.handle(command).await;
        assert!(result.is_ok());
        
        let message = result.unwrap();
        assert_eq!(message.content, "Hello, World!");
        assert_eq!(message.message_type, MessageType::Text);
    }

    #[tokio::test]
    async fn test_send_message_user_not_in_room() {
        let mut room_repository = MockChatRoomRepository::new();
        let message_repository = MockMessageRepository::new();
        let event_bus = MockEventBus::new();
        
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        
        // 配置mock - 用户不在房间中
        room_repository
            .expect_is_user_in_room()
            .returning(|_, _| Ok(false));
        
        let handler = ChatRoomCommandHandler::new(
            Arc::new(room_repository),
            Arc::new(message_repository),
            Arc::new(MockUserRepository::new()),
            Arc::new(event_bus),
        );
        
        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "Hello, World!".to_string(),
            message_type: MessageType::Text,
            reply_to_message_id: None,
        };
        
        let result = handler.handle(command).await;
        assert!(matches!(result, Err(DomainError::UserNotInRoom)));
    }
}
```

### 集成测试

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use axum::extract::Path;
    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use axum::Router;
    use serde_json::json;
    use tower_http::cors::CorsLayer;

    // 创建测试应用
    fn create_test_app() -> Router {
        Router::new()
            .route("/api/v1/health", get(health_check))
            .route("/api/v1/auth/register", post(register_user))
            .route("/api/v1/auth/login", post(login_user))
            .route("/api/v1/users/me", get(get_current_user))
            .layer(CorsLayer::permissive())
    }

    #[tokio::test]
    async fn test_health_check() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(response["status"], "healthy");
    }

    #[tokio::test]
    async fn test_user_registration_flow() {
        let app = create_test_app();

        // 注册用户
        let registration_data = json!({
            "username": "test_user",
            "email": "test@example.com",
            "password": "password123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/register")
                    .method(Method::POST)
                    .header("Content-Type", "application/json")
                    .body(Body::from(registration_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(response["success"].as_bool().unwrap());
        assert_eq!(response["data"]["user"]["username"], "test_user");
        assert_eq!(response["data"]["user"]["email"], "test@example.com");

        // 登录用户
        let login_data = json!({
            "email": "test@example.com",
            "password": "password123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/login")
                    .method(Method::POST)
                    .header("Content-Type", "application/json")
                    .body(Body::from(login_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(response["success"].as_bool().unwrap());
        assert!(response["data"]["access_token"].is_string());
        assert!(response["data"]["refresh_token"].is_string());

        // 使用token获取用户信息
        let access_token = response["data"]["access_token"].as_str().unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/users/me")
                    .method(Method::GET)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(response["success"].as_bool().unwrap());
        assert_eq!(response["data"]["username"], "test_user");
        assert_eq!(response["data"]["email"], "test@example.com");
    }

    #[tokio::test]
    async fn test_invalid_registration() {
        let app = create_test_app();

        // 测试无效邮箱
        let registration_data = json!({
            "username": "test_user",
            "email": "invalid_email",
            "password": "password123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/register")
                    .method(Method::POST)
                    .header("Content-Type", "application/json")
                    .body(Body::from(registration_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        // 测试密码过短
        let registration_data = json!({
            "username": "test_user",
            "email": "test@example.com",
            "password": "123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/register")
                    .method(Method::POST)
                    .header("Content-Type", "application/json")
                    .body(Body::from(registration_data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_unauthorized_access() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/users/me")
                    .method(Method::GET)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
```

### 性能测试

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_concurrent_user_registration() {
        let app = create_test_app();
        let num_users = 100;
        let start = Instant::now();

        let mut join_set = JoinSet::new();

        for i in 0..num_users {
            let app = app.clone();
            let username = format!("user_{}", i);
            let email = format!("user_{}@example.com", i);

            join_set.spawn(async move {
                let registration_data = json!({
                    "username": username,
                    "email": email,
                    "password": "password123"
                });

                let response = app
                    .oneshot(
                        Request::builder()
                            .uri("/api/v1/auth/register")
                            .method(Method::POST)
                            .header("Content-Type", "application/json")
                            .body(Body::from(registration_data.to_string()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();

                response.status()
            });
        }

        let mut success_count = 0;
        let mut error_count = 0;

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(status) => {
                    if status == StatusCode::OK {
                        success_count += 1;
                    } else {
                        error_count += 1;
                    }
                }
                Err(_) => {
                    error_count += 1;
                }
            }
        }

        let duration = start.elapsed();
        println!("Concurrent registration of {} users took {:?}", num_users, duration);
        println!("Success: {}, Errors: {}", success_count, error_count);

        // 断言所有请求都成功
        assert_eq!(success_count, num_users);
        assert_eq!(error_count, 0);

        // 断言性能要求 (应该小于10秒)
        assert!(duration.as_secs() < 10);
    }

    #[tokio::test]
    async fn test_message_send_performance() {
        let app = create_test_app();
        let num_messages = 1000;
        let start = Instant::now();

        let mut join_set = JoinSet::new();

        for i in 0..num_messages {
            let app = app.clone();
            let message_content = format!("Test message {}", i);

            join_set.spawn(async move {
                let message_data = json!({
                    "room_id": "550e8400-e29b-41d4-a716-446655440000",
                    "content": message_content,
                    "message_type": "text"
                });

                let response = app
                    .oneshot(
                        Request::builder()
                            .uri("/api/v1/rooms/550e8400-e29b-41d4-a716-446655440000/messages")
                            .method(Method::POST)
                            .header("Content-Type", "application/json")
                            .header("Authorization", "Bearer valid_token")
                            .body(Body::from(message_data.to_string()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();

                response.status()
            });
        }

        let mut success_count = 0;
        let mut error_count = 0;

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(status) => {
                    if status == StatusCode::OK {
                        success_count += 1;
                    } else {
                        error_count += 1;
                    }
                }
                Err(_) => {
                    error_count += 1;
                }
            }
        }

        let duration = start.elapsed();
        println!("Sending {} messages took {:?}", num_messages, duration);
        println!("Success: {}, Errors: {}", success_count, error_count);
        println!("Messages per second: {}", num_messages as f64 / duration.as_secs_f64());

        // 断言性能要求 (至少100消息/秒)
        let messages_per_second = num_messages as f64 / duration.as_secs_f64();
        assert!(messages_per_second > 100.0);
    }

    #[tokio::test]
    async fn test_database_connection_pool_performance() {
        let pool = create_test_connection_pool().await;
        let num_queries = 1000;
        let start = Instant::now();

        let mut join_set = JoinSet::new();

        for _ in 0..num_queries {
            let pool = pool.clone();
            join_set.spawn(async move {
                let result = sqlx::query("SELECT 1")
                    .fetch_one(&pool)
                    .await;
                result.is_ok()
            });
        }

        let mut success_count = 0;
        let mut error_count = 0;

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(success) => {
                    if success {
                        success_count += 1;
                    } else {
                        error_count += 1;
                    }
                }
                Err(_) => {
                    error_count += 1;
                }
            }
        }

        let duration = start.elapsed();
        println!("Database connection pool test with {} queries took {:?}", num_queries, duration);
        println!("Success: {}, Errors: {}", success_count, error_count);
        println!("Queries per second: {}", num_queries as f64 / duration.as_secs_f64());

        // 断言性能要求 (至少1000查询/秒)
        let queries_per_second = num_queries as f64 / duration.as_secs_f64();
        assert!(queries_per_second > 1000.0);
    }
}
```

### 测试工具和Mock

```rust
// Mock UserRepository
pub struct MockUserRepository {
    pub users: Arc<RwLock<HashMap<Uuid, User>>>,
}

impl MockUserRepository {
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_user(&self, user: User) {
        let mut users = self.users.write().await;
        users.insert(user.id, user);
    }
}

#[async_trait]
impl UserRepository for MockUserRepository {
    async fn save(&self, user: &User) -> Result<User> {
        let mut users = self.users.write().await;
        users.insert(user.id, user.clone());
        Ok(user.clone())
    }

    async fn find_by_id(&self, user_id: Uuid) -> Result<Option<User>> {
        let users = self.users.read().await;
        Ok(users.get(&user_id).cloned())
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<User>> {
        let users = self.users.read().await;
        Ok(users.values().find(|u| u.username == username).cloned())
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let users = self.users.read().await;
        Ok(users.values().find(|u| u.email == email).cloned())
    }

    async fn delete(&self, user_id: Uuid) -> Result<()> {
        let mut users = self.users.write().await;
        users.remove(&user_id);
        Ok(())
    }

    async fn find_by_ids(&self, user_ids: &[Uuid]) -> Result<Vec<User>> {
        let users = self.users.read().await;
        let result: Vec<User> = users
            .iter()
            .filter(|(id, _)| user_ids.contains(id))
            .map(|(_, user)| user.clone())
            .collect();
        Ok(result)
    }

    async fn find_by_room_id(&self, room_id: Uuid, limit: u32, offset: u32) -> Result<Vec<User>> {
        let users = self.users.read().await;
        let result: Vec<User> = users
            .values()
            .cloned()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(result)
    }

    async fn search_users(&self, keyword: &str, limit: u32, offset: u32) -> Result<Vec<User>> {
        let users = self.users.read().await;
        let result: Vec<User> = users
            .values()
            .filter(|u| u.username.contains(keyword) || u.email.contains(keyword))
            .cloned()
            .skip(offset as usize)
            .take(limit as usize)
            .collect();
        Ok(result)
    }

    async fn count_by_room_id(&self, room_id: Uuid) -> Result<u32> {
        let users = self.users.read().await;
        Ok(users.len() as u32)
    }

    async fn update_status(&self, user_id: Uuid, status: UserStatus) -> Result<()> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(&user_id) {
            user.status = status;
        }
        Ok(())
    }

    async fn update_last_active(&self, user_id: Uuid) -> Result<()> {
        let mut users = self.users.write().await;
        if let Some(user) = users.get_mut(&user_id) {
            user.last_active_at = Some(Utc::now());
        }
        Ok(())
    }
}

// Mock EventBus
pub struct MockEventBus {
    pub events: Arc<RwLock<Vec<DomainEvent>>>,
}

impl MockEventBus {
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn get_events(&self) -> Vec<DomainEvent> {
        let events = self.events.read().await;
        events.clone()
    }
}

#[async_trait]
impl EventBus for MockEventBus {
    async fn publish(&self, event: DomainEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event);
        Ok(())
    }

    async fn subscribe(&self, _handler: Arc<dyn EventHandler>) -> Result<()> {
        Ok(())
    }
}

// 测试数据库设置
pub async fn setup_test_database() -> sqlx::PgPool {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/chatroom_test".to_string());

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create test database pool");

    // 运行数据库迁移
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    pool
}

// 清理测试数据
pub async fn cleanup_test_database(pool: &sqlx::PgPool) {
    let tables = vec![
        "messages",
        "room_members", 
        "chat_rooms",
        "user_roles",
        "organizations",
        "users",
        "sessions",
    ];

    for table in tables {
        sqlx::query(&format!("TRUNCATE TABLE {} CASCADE", table))
            .execute(pool)
            .await
            .expect("Failed to truncate table");
    }
}

// 创建测试用户
pub async fn create_test_user(pool: &sqlx::PgPool, username: &str, email: &str) -> User {
    let user_id = Uuid::new_v4();
    let password_hash = hash_password("password123").unwrap();

    sqlx::query!(
        r#"
        INSERT INTO users (id, username, email, password_hash, status, created_at, updated_at)
        VALUES ($1, $2, $3, $4, 'active', NOW(), NOW())
        RETURNING id, username, email, avatar_url, status, password_hash, created_at, updated_at
        "#,
        user_id,
        username,
        email,
        password_hash
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create test user")
    .into()
}

// 创建测试聊天室
pub async fn create_test_room(pool: &sqlx::PgPool, name: &str, owner_id: Uuid) -> ChatRoom {
    let room_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO chat_rooms (id, name, owner_id, is_private, created_at, updated_at)
        VALUES ($1, $2, $3, false, NOW(), NOW())
        RETURNING id, name, description, owner_id, is_private, password_hash, max_members, allow_invites, require_approval, settings, created_at, updated_at
        "#,
        room_id,
        name,
        owner_id
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create test room")
    .into()
}
```

## 📊 测试覆盖率

### 测试配置

```toml
[package]
name = "chatroom-backend"
version = "0.1.0"
edition = "2021"

[dev-dependencies]
tokio-test = "0.4"
mockall = "0.11"
assert-json-diff = "2.0"
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.4"
testcontainers = "0.23"
wiremock = "0.5"

[profile.dev]
debug = true
opt-level = 0

[profile.test]
debug = true
opt-level = 1

[[bench]]
name = "user_registration"
harness = false

[[bench]]
name = "message_sending"
harness = false

[[bench]]
name = "websocket_performance"
harness = false
```

### 覆盖率配置

```toml
[workspace.metadata.cargo-llvm-cov]
branch = true
include = [
    "src/*",
    "src/**/*",
]
exclude = [
    "src/main.rs",
    "src/bin/*",
    "tests/*",
]
```

### CI/CD测试配置

```yaml
name: CI/CD Pipeline

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: password
          POSTGRES_DB: chatroom_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432
      
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Set up Rust
      uses: dtolnay/rust-toolchain@stable
      with:
        components: rustfmt, clippy
    
    - name: Cache cargo registry
      uses: actions/cache@v3
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Install dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y postgresql-client
        cargo install cargo-llvm-cov
    
    - name: Run formatting check
      run: cargo fmt --all -- --check
    
    - name: Run clippy
      run: cargo clippy -- -D warnings
    
    - name: Run tests
      run: |
        export TEST_DATABASE_URL=postgresql://postgres:password@localhost:5432/chatroom_test
        cargo llvm-cov --lcov --output-path coverage-report
    
    - name: Upload coverage to Codecov
      uses: codecov/codecov-action@v3
      with:
        file: ./coverage-report
        fail_ci_if_error: true
    
    - name: Run integration tests
      run: |
        export TEST_DATABASE_URL=postgresql://postgres:password@localhost:5432/chatroom_test
        cargo test --test integration_tests -- --nocapture
    
    - name: Run performance tests
      run: |
        export TEST_DATABASE_URL=postgresql://postgres:password@localhost:5432/chatroom_test
        cargo bench
```

## 🔧 测试最佳实践

### 1. 测试命名约定

```rust
// 单元测试命名
#[test]
fn test_user_creation_with_valid_data() {}

#[test]
fn test_user_creation_with_invalid_email() {}

#[test]
fn test_user_creation_with_duplicate_username() {}

// 集成测试命名
#[tokio::test]
async fn test_complete_user_registration_flow() {}

#[tokio::test]
async fn test_user_login_with_valid_credentials() {}

#[tokio::test]
async fn test_user_login_with_invalid_credentials() {}

// 性能测试命名
#[tokio::test]
async fn test_concurrent_user_registration_performance() {}

#[tokio::test]
async fn test_message_sending_throughput() {}

#[tokio::test]
async fn test_database_connection_pool_efficiency() {}
```

### 2. 测试数据管理

```rust
// 测试数据工厂
pub struct TestDataFactory {
    pool: sqlx::PgPool,
}

impl TestDataFactory {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_user(&self) -> User {
        let user_id = Uuid::new_v4();
        let username = format!("user_{}", user_id);
        let email = format!("user_{}@example.com", user_id);
        
        create_test_user(&self.pool, &username, &email).await
    }

    pub async fn create_room(&self, owner_id: Uuid) -> ChatRoom {
        let room_name = format!("Room {}", Uuid::new_v4());
        create_test_room(&self.pool, &room_name, owner_id).await
    }

    pub async fn create_message(&self, room_id: Uuid, user_id: Uuid) -> Message {
        let message_id = Uuid::new_v4();
        let content = format!("Test message {}", message_id);
        
        sqlx::query!(
            r#"
            INSERT INTO messages (id, room_id, user_id, content, message_type, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'text', NOW(), NOW())
            RETURNING id, room_id, user_id, content, message_type, reply_to_message_id, is_edited, is_deleted, metadata, created_at, updated_at
            "#,
            message_id,
            room_id,
            user_id,
            content
        )
        .fetch_one(&self.pool)
        .await
        .expect("Failed to create test message")
        .into()
    }

    pub async fn create_organization(&self, owner_id: Uuid) -> Organization {
        let org_id = Uuid::new_v4();
        let name = format!("Organization {}", org_id);
        
        sqlx::query!(
            r#"
            INSERT INTO organizations (id, name, owner_id, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, true, NOW(), NOW())
            RETURNING id, name, description, owner_id, settings, max_members, is_active, created_at, updated_at
            "#,
            org_id,
            name,
            owner_id
        )
        .fetch_one(&self.pool)
        .await
        .expect("Failed to create test organization")
        .into()
    }
}
```

### 3. 测试断言工具

```rust
pub trait TestAssertions {
    fn assert_user_equal(&self, other: &User);
    fn assert_room_equal(&self, other: &ChatRoom);
    fn assert_message_equal(&self, other: &Message);
    fn assert_error_code(&self, expected_code: &str);
}

impl TestAssertions for User {
    fn assert_user_equal(&self, other: &User) {
        assert_eq!(self.id, other.id);
        assert_eq!(self.username, other.username);
        assert_eq!(self.email, other.email);
        assert_eq!(self.status, other.status);
    }
}

impl TestAssertions for ChatRoom {
    fn assert_room_equal(&self, other: &ChatRoom) {
        assert_eq!(self.id, other.id);
        assert_eq!(self.name, other.name);
        assert_eq!(self.owner_id, other.owner_id);
        assert_eq!(self.is_private, other.is_private);
    }
}

impl TestAssertions for Message {
    fn assert_message_equal(&self, other: &Message) {
        assert_eq!(self.id, other.id);
        assert_eq!(self.room_id, other.room_id);
        assert_eq!(self.user_id, other.user_id);
        assert_eq!(self.content, other.content);
        assert_eq!(self.message_type, other.message_type);
    }
}

impl TestAssertions for DomainError {
    fn assert_error_code(&self, expected_code: &str) {
        assert_eq!(self.error_code(), expected_code);
    }
}

// 自定义断言宏
macro_rules! assert_success {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(e) => panic!("Expected success, but got error: {}", e),
        }
    };
}

macro_rules! assert_error {
    ($result:expr, $expected_error:pat) => {
        match $result {
            Ok(_) => panic!("Expected error, but got success"),
            Err($expected_error) => {},
            Err(e) => panic!("Expected error pattern, but got different error: {}", e),
        }
    };
}

macro_rules! assert_status_code {
    ($response:expr, $expected_code:expr) => {
        assert_eq!($response.status(), $expected_code);
    };
}
```

---

**下一步**: 阅读[08-websocket-message-protocol.md](./08-websocket-message-protocol.md)了解WebSocket消息协议的详细设计。
