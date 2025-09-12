# Web API层设计

Web API层负责处理HTTP请求和WebSocket连接，提供REST API端点和WebSocket服务。本层使用Axum框架实现，包含JWT认证、CORS处理、请求验证等功能。

## 🌐 Web API层架构

### 核心组件

```rust
// HTTP服务器
pub struct HttpServer {
    app: Router,
    port: u16,
}

// WebSocket处理器
pub struct WebSocketHandler {
    websocket_manager: Arc<dyn WebSocketManager>,
    auth_service: Arc<dyn AuthenticationService>,
}

// 认证中间件
pub struct AuthMiddleware {
    auth_service: Arc<dyn AuthenticationService>,
}

// 请求验证器
pub struct RequestValidator {
    validator: Arc<Validator>,
}

// 响应格式化器
pub struct ResponseFormatter {
    formatter: Arc<Formatter>,
}
```

### Axum路由配置

```rust
pub fn create_app(
    application_container: Arc<ApplicationContainer>,
    infrastructure_container: Arc<InfrastructureContainer>,
) -> Router {
    // 创建状态共享
    let app_state = AppState {
        application_container: application_container.clone(),
        infrastructure_container: infrastructure_container.clone(),
    };
    
    // 创建基础路由
    let app = Router::new()
        // 健康检查端点
        .route("/health", axum::routing::get(health_check))
        
        // API版本控制
        .nest("/api/v1", api_routes())
        
        // WebSocket端点
        .route("/ws", axum::routing::get(websocket_handler))
        
        // 静态文件服务
        .nest_service("/static", axum::routing::get_service(static_service()))
        
        // 状态共享
        .with_state(app_state)
        
        // 全局中间件
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024 * 10)); // 10MB
}

fn api_routes() -> Router {
    Router::new()
        // 认证相关路由
        .route("/auth/register", axum::routing::post(register_user))
        .route("/auth/login", axum::routing::post(login_user))
        .route("/auth/logout", axum::routing::post(logout_user))
        .route("/auth/refresh", axum::routing::post(refresh_token))
        
        // 用户相关路由
        .route("/users/me", axum::routing::get(get_current_user))
        .route("/users/me", axum::routing::put(update_current_user))
        .route("/users/search", axum::routing::get(search_users))
        
        // 聊天室相关路由
        .route("/rooms", axum::routing::post(create_room))
        .route("/rooms", axum::routing::get(get_user_rooms))
        .route("/rooms/:room_id", axum::routing::get(get_room_details))
        .route("/rooms/:room_id", axum::routing::put(update_room))
        .route("/rooms/:room_id", axum::routing::delete(delete_room))
        .route("/rooms/:room_id/join", axum::routing::post(join_room))
        .route("/rooms/:room_id/leave", axum::routing::post(leave_room))
        .route("/rooms/:room_id/messages", axum::routing::get(get_room_messages))
        .route("/rooms/:room_id/members", axum::routing::get(get_room_members))
        
        // 消息相关路由
        .route("/messages/:message_id", axum::routing::get(get_message))
        .route("/messages/:message_id", axum::routing::put(update_message))
        .route("/messages/:message_id", axum::routing::delete(delete_message))
        .route("/messages/search", axum::routing::get(search_messages))
        
        // 组织相关路由
        .route("/organizations", axum::routing::post(create_organization))
        .route("/organizations", axum::routing::get(get_user_organizations))
        .route("/organizations/:org_id", axum::routing::get(get_organization_details))
        .route("/organizations/:org_id", axum::routing::put(update_organization))
        .route("/organizations/:org_id/members", axum::routing::get(get_organization_members))
        .route("/organizations/:org_id/users", axum::routing::post(add_user_to_organization))
        
        // 认证中间件
        .layer(axum::middleware::from_fn_with_state(
            AppState::default(),
            auth_middleware,
        ))
}

#[derive(Clone)]
pub struct AppState {
    pub application_container: Arc<ApplicationContainer>,
    pub infrastructure_container: Arc<InfrastructureContainer>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            application_container: Arc::new(ApplicationContainer::new(
                // 这里应该传入实际的依赖
                Arc::new(InMemoryUserRepository::new()),
                Arc::new(InMemoryChatRoomRepository::new()),
                Arc::new(InMemoryMessageRepository::new()),
                Arc::new(InMemoryOrganizationRepository::new()),
                Arc::new(InMemoryRoleRepository::new()),
                Arc::new(InMemoryDepartmentRepository::new()),
                Arc::new(InMemoryUserRoleRepository::new()),
                Arc::new(InMemoryPositionRepository::new()),
                Arc::new(InMemoryUserProxyRepository::new()),
                Arc::new(InMemoryOnlineTimeRepository::new()),
                Arc::new(InMemoryWebSocketManager::new()),
                Arc::new(JwtService::new("secret".to_string())),
                Arc::new(PasswordService::new()),
                Arc::new(RedisClient::new(&RedisConfig::default()).await.unwrap()),
                Arc::new(InMemoryNotificationService::new()),
                Arc::new(InMemorySearchService::new()),
                Arc::new(InMemoryPresenceService::new()),
                Arc::new(InMemoryAnalyticsService::new()),
                FeatureFlags::default(),
            )),
            infrastructure_container: Arc::new(InfrastructureContainer::new(
                DatabaseConfig::default(),
                RedisConfig::default(),
                KafkaConfig::default(),
                WebSocketConfig::default(),
            ).await.unwrap()),
        }
    }
}
```

## 🔐 JWT认证和会话管理

### JWT服务实现

```rust
#[derive(Debug, Clone)]
pub struct JwtService {
    secret: String,
    access_token_expiry: Duration,
    refresh_token_expiry: Duration,
}

impl JwtService {
    pub fn new(secret: String) -> Self {
        Self {
            secret,
            access_token_expiry: Duration::hours(1),
            refresh_token_expiry: Duration::days(7),
        }
    }
    
    pub fn generate_access_token(&self, user_id: Uuid) -> Result<String> {
        let now = Utc::now();
        let exp = now + self.access_token_expiry;
        
        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
            token_type: "access".to_string(),
        };
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_ref()),
        )?;
        
        Ok(token)
    }
    
    pub fn generate_refresh_token(&self, user_id: Uuid) -> Result<String> {
        let now = Utc::now();
        let exp = now + self.refresh_token_expiry;
        
        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
            token_type: "refresh".to_string(),
        };
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_ref()),
        )?;
        
        Ok(token)
    }
    
    pub fn verify_access_token(&self, token: &str) -> Result<TokenData> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )?;
        
        if token_data.claims.token_type != "access" {
            return Err(AuthError::InvalidToken("Invalid token type".to_string()));
        }
        
        Ok(TokenData {
            user_id: Uuid::parse_str(&token_data.claims.sub)?,
            claims: token_data.claims,
        })
    }
    
    pub fn verify_refresh_token(&self, token: &str) -> Result<TokenData> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_ref()),
            &Validation::default(),
        )?;
        
        if token_data.claims.token_type != "refresh" {
            return Err(AuthError::InvalidToken("Invalid token type".to_string()));
        }
        
        Ok(TokenData {
            user_id: Uuid::parse_str(&token_data.claims.sub)?,
            claims: token_data.claims,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // 用户ID
    pub exp: usize,  // 过期时间
    pub iat: usize,  // 签发时间
    pub token_type: String, // token类型
}

#[derive(Debug)]
pub struct TokenData {
    pub user_id: Uuid,
    pub claims: Claims,
}
```

### 认证中间件

```rust
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 跳过认证的路由
    let path = req.uri().path();
    if path.starts_with("/api/v1/auth/") || path == "/health" {
        return Ok(next.run(req).await);
    }
    
    // 从请求头获取Authorization token
    let auth_header = req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());
    
    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            header[7..].trim()
        }
        _ => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    };
    
    // 验证token
    let token_data = match state.application_container.auth_service.jwt_service.verify_access_token(token) {
        Ok(data) => data,
        Err(_) => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    };
    
    // 检查用户是否存在
    let user_exists = match state.application_container.auth_service.user_repository.find_by_id(token_data.user_id).await {
        Ok(Some(_)) => true,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    if !user_exists {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 将用户信息添加到请求扩展中
    let mut req = req;
    req.extensions_mut().insert(AuthInfo {
        user_id: token_data.user_id,
        token: token.to_string(),
    });
    
    Ok(next.run(req).await)
}

#[derive(Debug, Clone)]
pub struct AuthInfo {
    pub user_id: Uuid,
    pub token: String,
}

pub fn extract_auth_info(req: &Request) -> Option<AuthInfo> {
    req.extensions().get::<AuthInfo>().cloned()
}
```

## 📡 REST API端点实现

### 认证端点

```rust
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserDto,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            timestamp: Utc::now(),
        }
    }
    
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message),
            timestamp: Utc::now(),
        }
    }
}

// 用户注册
pub async fn register_user(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, StatusCode> {
    // 验证请求参数
    if let Err(errors) = validate_register_request(&request) {
        return Ok(Json(ApiResponse::error(format!("Validation failed: {:?}", errors))));
    }
    
    // 处理注册
    let auth_request = RegisterUserRequest {
        username: request.username,
        email: request.email,
        password: request.password,
        avatar_url: request.avatar_url,
    };
    
    match state.application_container.auth_service.register(auth_request).await {
        Ok(response) => Ok(Json(ApiResponse::success(response))),
        Err(e) => match e {
            DomainError::UserAlreadyExists => {
                Ok(Json(ApiResponse::error("User already exists".to_string())))
            }
            DomainError::ValidationFailed(msg) => {
                Ok(Json(ApiResponse::error(format!("Validation failed: {}", msg))))
            }
            _ => {
                tracing::error!("Registration error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 用户登录
pub async fn login_user(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, StatusCode> {
    // 验证请求参数
    if request.email.is_empty() || request.password.is_empty() {
        return Ok(Json(ApiResponse::error("Email and password are required".to_string())));
    }
    
    // 处理登录
    let auth_request = LoginUserRequest {
        email: request.email,
        password: request.password,
    };
    
    match state.application_container.auth_service.login(auth_request).await {
        Ok(response) => Ok(Json(ApiResponse::success(response))),
        Err(e) => match e {
            DomainError::UserNotFound => {
                Ok(Json(ApiResponse::error("User not found".to_string())))
            }
            DomainError::InvalidCredentials => {
                Ok(Json(ApiResponse::error("Invalid credentials".to_string())))
            }
            DomainError::UserNotActive => {
                Ok(Json(ApiResponse::error("User account is not active".to_string())))
            }
            _ => {
                tracing::error!("Login error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 用户登出
pub async fn logout_user(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.application_container.auth_service.logout(auth_info.user_id).await {
        Ok(_) => Ok(Json(ApiResponse::success(()))),
        Err(e) => {
            tracing::error!("Logout error: {:?}", e);
            Ok(Json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

// 刷新token
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(request): Json<RefreshTokenRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, StatusCode> {
    if request.refresh_token.is_empty() {
        return Ok(Json(ApiResponse::error("Refresh token is required".to_string())));
    }
    
    match state.application_container.auth_service.refresh_token(&request.refresh_token).await {
        Ok(response) => Ok(Json(ApiResponse::success(response))),
        Err(e) => match e {
            DomainError::InvalidToken => {
                Ok(Json(ApiResponse::error("Invalid refresh token".to_string())))
            }
            DomainError::TokenExpired => {
                Ok(Json(ApiResponse::error("Refresh token expired".to_string())))
            }
            _ => {
                tracing::error!("Token refresh error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}
```

### 用户管理端点

```rust
// 获取当前用户信息
pub async fn get_current_user(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
) -> Result<Json<ApiResponse<UserProfileDto>>, StatusCode> {
    match state.application_container.auth_service.get_user_profile(auth_info.user_id).await {
        Ok(profile) => Ok(Json(ApiResponse::success(profile))),
        Err(e) => {
            tracing::error!("Get user profile error: {:?}", e);
            Ok(Json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

// 更新当前用户信息
pub async fn update_current_user(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<UserDto>>, StatusCode> {
    let command = UpdateUserCommand {
        user_id: auth_info.user_id,
        username: request.username,
        email: request.email,
        avatar_url: request.avatar_url,
    };
    
    match state.application_container.auth_service.update_user(command).await {
        Ok(user) => Ok(Json(ApiResponse::success(UserDto::from(user)))),
        Err(e) => match e {
            DomainError::UserNotFound => {
                Ok(Json(ApiResponse::error("User not found".to_string())))
            }
            DomainError::UserAlreadyExists => {
                Ok(Json(ApiResponse::error("Username or email already exists".to_string())))
            }
            DomainError::ValidationFailed(msg) => {
                Ok(Json(ApiResponse::error(format!("Validation failed: {}", msg))))
            }
            _ => {
                tracing::error!("Update user error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 搜索用户
pub async fn search_users(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Query(params): Query<SearchUsersQuery>,
) -> Result<Json<ApiResponse<Vec<UserDto>>>, StatusCode> {
    let keyword = params.keyword.trim();
    if keyword.is_empty() {
        return Ok(Json(ApiResponse::error("Search keyword is required".to_string())));
    }
    
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);
    
    match state.application_container.auth_service.search_users(keyword, limit, offset).await {
        Ok(users) => Ok(Json(ApiResponse::success(users))),
        Err(e) => {
            tracing::error!("Search users error: {:?}", e);
            Ok(Json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SearchUsersQuery {
    pub keyword: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
```

### 聊天室管理端点

```rust
#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    pub name: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoomRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_private: Option<bool>,
    pub password: Option<String>,
}

// 创建聊天室
pub async fn create_room(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<ApiResponse<ChatRoomDto>>, StatusCode> {
    // 验证请求参数
    if let Err(errors) = validate_create_room_request(&request) {
        return Ok(Json(ApiResponse::error(format!("Validation failed: {:?}", errors))));
    }
    
    let room_request = CreateRoomRequest {
        name: request.name,
        description: request.description,
        is_private: request.is_private,
        password: request.password,
    };
    
    match state.application_container.chat_room_service.create_room(room_request, auth_info.user_id).await {
        Ok(room) => Ok(Json(ApiResponse::success(room))),
        Err(e) => match e {
            DomainError::ValidationFailed(msg) => {
                Ok(Json(ApiResponse::error(format!("Validation failed: {}", msg))))
            }
            _ => {
                tracing::error!("Create room error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 获取用户的聊天室列表
pub async fn get_user_rooms(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Query(params): Query<GetUserRoomsQuery>,
) -> Result<Json<ApiResponse<Vec<ChatRoomDto>>>, StatusCode> {
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);
    
    match state.application_container.chat_room_service.get_user_rooms(auth_info.user_id, Some(limit), Some(offset)).await {
        Ok(rooms) => Ok(Json(ApiResponse::success(rooms))),
        Err(e) => {
            tracing::error!("Get user rooms error: {:?}", e);
            Ok(Json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

// 获取聊天室详情
pub async fn get_room_details(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ApiResponse<ChatRoomDetailDto>>, StatusCode> {
    match state.application_container.chat_room_service.get_room_details(room_id).await {
        Ok(room) => {
            // 检查用户是否有权限访问该房间
            if !state.application_container.chat_room_service.is_user_in_room(room_id, auth_info.user_id).await.unwrap_or(false) {
                return Ok(Json(ApiResponse::error("Access denied".to_string())));
            }
            Ok(Json(ApiResponse::success(room)))
        }
        Err(e) => match e {
            DomainError::RoomNotFound => {
                Ok(Json(ApiResponse::error("Room not found".to_string())))
            }
            _ => {
                tracing::error!("Get room details error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 更新聊天室
pub async fn update_room(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Path(room_id): Path<Uuid>,
    Json(request): Json<UpdateRoomRequest>,
) -> Result<Json<ApiResponse<ChatRoomDto>>, StatusCode> {
    // 检查用户是否有权限更新房间
    if !state.application_container.chat_room_service.is_room_owner(room_id, auth_info.user_id).await.unwrap_or(false) {
        return Ok(Json(ApiResponse::error("Access denied".to_string())));
    }
    
    let command = UpdateChatRoomCommand {
        room_id,
        name: request.name,
        description: request.description,
        is_private: request.is_private,
        password: request.password,
        updated_by: auth_info.user_id,
    };
    
    match state.application_container.chat_room_service.update_room(command).await {
        Ok(room) => Ok(Json(ApiResponse::success(room))),
        Err(e) => match e {
            DomainError::RoomNotFound => {
                Ok(Json(ApiResponse::error("Room not found".to_string())))
            }
            DomainError::ValidationFailed(msg) => {
                Ok(Json(ApiResponse::error(format!("Validation failed: {}", msg))))
            }
            _ => {
                tracing::error!("Update room error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 删除聊天室
pub async fn delete_room(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    // 检查用户是否有权限删除房间
    if !state.application_container.chat_room_service.is_room_owner(room_id, auth_info.user_id).await.unwrap_or(false) {
        return Ok(Json(ApiResponse::error("Access denied".to_string())));
    }
    
    match state.application_container.chat_room_service.delete_room(room_id, auth_info.user_id).await {
        Ok(_) => Ok(Json(ApiResponse::success(()))),
        Err(e) => match e {
            DomainError::RoomNotFound => {
                Ok(Json(ApiResponse::error("Room not found".to_string())))
            }
            _ => {
                tracing::error!("Delete room error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 加入聊天室
pub async fn join_room(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Path(room_id): Path<Uuid>,
    Json(request): Json<JoinRoomRequest>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.application_container.chat_room_service.join_room(room_id, auth_info.user_id, request.password).await {
        Ok(_) => Ok(Json(ApiResponse::success(()))),
        Err(e) => match e {
            DomainError::RoomNotFound => {
                Ok(Json(ApiResponse::error("Room not found".to_string())))
            }
            DomainError::RoomIsPrivate => {
                Ok(Json(ApiResponse::error("Room is private".to_string())))
            }
            DomainError::InvalidPassword => {
                Ok(Json(ApiResponse::error("Invalid password".to_string())))
            }
            DomainError::UserAlreadyInRoom => {
                Ok(Json(ApiResponse::error("User already in room".to_string())))
            }
            _ => {
                tracing::error!("Join room error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 离开聊天室
pub async fn leave_room(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    match state.application_container.chat_room_service.leave_room(room_id, auth_info.user_id).await {
        Ok(_) => Ok(Json(ApiResponse::success(()))),
        Err(e) => match e {
            DomainError::RoomNotFound => {
                Ok(Json(ApiResponse::error("Room not found".to_string())))
            }
            DomainError::UserNotInRoom => {
                Ok(Json(ApiResponse::error("User not in room".to_string())))
            }
            _ => {
                tracing::error!("Leave room error: {:?}", e);
                Ok(Json(ApiResponse::error("Internal server error".to_string())))
            }
        },
    }
}

// 获取聊天室消息
pub async fn get_room_messages(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Path(room_id): Path<Uuid>,
    Query(params): Query<GetRoomMessagesQuery>,
) -> Result<Json<ApiResponse<Vec<MessageDto>>>, StatusCode> {
    // 检查用户是否有权限访问该房间
    if !state.application_container.chat_room_service.is_user_in_room(room_id, auth_info.user_id).await.unwrap_or(false) {
        return Ok(Json(ApiResponse::error("Access denied".to_string())));
    }
    
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = params.offset.unwrap_or(0);
    
    match state.application_container.chat_room_service.get_room_messages(room_id, Some(limit), Some(offset)).await {
        Ok(messages) => Ok(Json(ApiResponse::success(messages))),
        Err(e) => {
            tracing::error!("Get room messages error: {:?}", e);
            Ok(Json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}

// 获取聊天室成员
pub async fn get_room_members(
    State(state): State<AppState>,
    Extension(auth_info): Extension<AuthInfo>,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<RoomMemberDto>>>, StatusCode> {
    // 检查用户是否有权限访问该房间
    if !state.application_container.chat_room_service.is_user_in_room(room_id, auth_info.user_id).await.unwrap_or(false) {
        return Ok(Json(ApiResponse::error("Access denied".to_string())));
    }
    
    match state.application_container.chat_room_service.get_room_members(room_id).await {
        Ok(members) => Ok(Json(ApiResponse::success(members))),
        Err(e) => {
            tracing::error!("Get room members error: {:?}", e);
            Ok(Json(ApiResponse::error("Internal server error".to_string())))
        }
    }
}
```

## 🔌 WebSocket路由和处理

### WebSocket处理器

```rust
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(params): Query<WebSocketQuery>,
) -> Result<Response, StatusCode> {
    // 验证token
    let token = params.token.trim();
    if token.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 验证JWT token
    let token_data = match state.application_container.auth_service.jwt_service.verify_access_token(token) {
        Ok(data) => data,
        Err(_) => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    };
    
    // 检查用户是否存在
    let user_exists = match state.application_container.auth_service.user_repository.find_by_id(token_data.user_id).await {
        Ok(Some(_)) => true,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    
    if !user_exists {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    // 升级到WebSocket连接
    Ok(ws.on_upgrade(move |socket| {
        handle_websocket_connection(socket, state, token_data.user_id)
    }))
}

#[derive(Debug, Deserialize)]
pub struct WebSocketQuery {
    pub token: String,
}

async fn handle_websocket_connection(
    socket: WebSocket,
    state: AppState,
    user_id: Uuid,
) {
    // 处理WebSocket连接
    if let Err(e) = state.infrastructure_container.websocket_manager.handle_connection(socket, user_id).await {
        tracing::error!("WebSocket connection error: {:?}", e);
    }
}
```

## ⚙️ 配置管理

### 应用配置

```rust
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub kafka: KafkaConfig,
    pub websocket: WebSocketConfig,
    pub jwt: JwtConfig,
    pub cors: CorsConfig,
    pub logging: LoggingConfig,
    pub feature_flags: FeatureFlags,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub access_token_expiry_hours: u64,
    pub refresh_token_expiry_days: u64,
}

#[derive(Debug, Deserialize)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age: u64,
}

#[derive(Debug, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub file: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                workers: 4,
            },
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            kafka: KafkaConfig::default(),
            websocket: WebSocketConfig::default(),
            jwt: JwtConfig {
                secret: "your-secret-key-change-in-production".to_string(),
                access_token_expiry_hours: 1,
                refresh_token_expiry_days: 7,
            },
            cors: CorsConfig {
                allowed_origins: vec!["*".to_string()],
                allowed_methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()],
                allowed_headers: vec!["*".to_string()],
                allow_credentials: true,
                max_age: 86400,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
                file: None,
            },
            feature_flags: FeatureFlags::default(),
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        // 使用figment库加载配置
        let config = Figment::new()
            .merge(Json::default())
            .merge(Env::prefixed("CHATROOM_").split("_"))
            .extract()?;
        
        Ok(config)
    }
    
    pub fn validate(&self) -> Result<()> {
        // 验证配置参数
        if self.server.port == 0 {
            return Err(ConfigError::InvalidPort);
        }
        
        if self.jwt.secret.is_empty() {
            return Err(ConfigError::MissingJwtSecret);
        }
        
        if self.database.url.is_empty() {
            return Err(ConfigError::MissingDatabaseUrl);
        }
        
        Ok(())
    }
}
```

### 服务器启动

```rust
pub struct ChatRoomServer {
    config: AppConfig,
    app_state: AppState,
}

impl ChatRoomServer {
    pub async fn new(config: AppConfig) -> Result<Self> {
        // 验证配置
        config.validate()?;
        
        // 初始化基础设施容器
        let infrastructure_container = Arc::new(
            InfrastructureContainer::new(
                config.database.clone(),
                config.redis.clone(),
                config.kafka.clone(),
                config.websocket.clone(),
            ).await?
        );
        
        // 初始化应用容器
        let application_container = Arc::new(
            ApplicationContainer::new(
                infrastructure_container.user_repository.clone(),
                infrastructure_container.room_repository.clone(),
                infrastructure_container.message_repository.clone(),
                infrastructure_container.organization_repository.clone(),
                infrastructure_container.role_repository.clone(),
                infrastructure_container.department_repository.clone(),
                infrastructure_container.user_role_repository.clone(),
                infrastructure_container.position_repository.clone(),
                infrastructure_container.user_proxy_repository.clone(),
                infrastructure_container.online_time_repository.clone(),
                infrastructure_container.websocket_manager.clone(),
                Arc::new(JwtService::new(config.jwt.secret.clone())),
                Arc::new(PasswordService::new()),
                infrastructure_container.redis_client.clone(),
                Arc::new(InMemoryNotificationService::new()),
                Arc::new(InMemorySearchService::new()),
                Arc::new(InMemoryPresenceService::new()),
                Arc::new(InMemoryAnalyticsService::new()),
                config.feature_flags.clone(),
            )
        );
        
        let app_state = AppState {
            application_container,
            infrastructure_container,
        };
        
        Ok(Self { config, app_state })
    }
    
    pub async fn start(&self) -> Result<()> {
        // 启动后台服务
        self.app_state.infrastructure_container.start_background_services().await?;
        
        // 创建Axum应用
        let app = create_app(
            self.app_state.application_container.clone(),
            self.app_state.infrastructure_container.clone(),
        );
        
        // 配置CORS
        let cors = tower_http::cors::CorsLayer::new()
            .allow_origin(tower_http::cors::Any)
            .allow_methods(tower_http::cors::Any)
            .allow_headers(tower_http::cors::Any);
        
        let app = app.layer(cors);
        
        // 配置日志
        let trace_layer = tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(|request: &Request<_>| {
                tracing::info_span!(
                    "http_request",
                    method = %request.method(),
                    uri = %request.uri(),
                    version = ?request.version(),
                )
            })
            .on_request(|_request: &Request<_>, _span: &tracing::Span| {
                tracing::info!("started processing request");
            })
            .on_response(|_response: &Response, latency: Duration, _span: &tracing::Span| {
                tracing::info!("finished processing request in {:?}", latency);
            });
        
        let app = app.layer(trace_layer);
        
        // 创建TCP监听器
        let addr = SocketAddr::from_str(&format!("{}:{}", self.config.server.host, self.config.server.port))?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        
        tracing::info!("Server started on http://{}", addr);
        
        // 启动服务器
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    // 加载配置
    let config = AppConfig::from_env()?;
    
    // 创建并启动服务器
    let server = ChatRoomServer::new(config).await?;
    server.start().await?;
    
    Ok(())
}
```

## 🛡️ 安全考虑

### 输入验证

```rust
pub fn validate_register_request(request: &RegisterRequest) -> Vec<String> {
    let mut errors = Vec::new();
    
    // 验证用户名
    if request.username.len() < 3 || request.username.len() > 20 {
        errors.push("Username must be between 3 and 20 characters".to_string());
    }
    
    if !request.username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        errors.push("Username can only contain alphanumeric characters and underscores".to_string());
    }
    
    // 验证邮箱
    if !request.email.contains('@') || !request.email.contains('.') {
        errors.push("Invalid email format".to_string());
    }
    
    // 验证密码
    if request.password.len() < 8 {
        errors.push("Password must be at least 8 characters long".to_string());
    }
    
    if !request.password.chars().any(|c| c.is_uppercase()) {
        errors.push("Password must contain at least one uppercase letter".to_string());
    }
    
    if !request.password.chars().any(|c| c.is_lowercase()) {
        errors.push("Password must contain at least one lowercase letter".to_string());
    }
    
    if !request.password.chars().any(|c| c.is_ascii_digit()) {
        errors.push("Password must contain at least one digit".to_string());
    }
    
    errors
}

pub fn validate_create_room_request(request: &CreateRoomRequest) -> Vec<String> {
    let mut errors = Vec::new();
    
    // 验证房间名称
    if request.name.len() < 1 || request.name.len() > 50 {
        errors.push("Room name must be between 1 and 50 characters".to_string());
    }
    
    // 验证房间描述
    if let Some(ref description) = request.description {
        if description.len() > 500 {
            errors.push("Room description must be less than 500 characters".to_string());
        }
    }
    
    // 验证密码
    if request.is_private {
        if let Some(ref password) = request.password {
            if password.len() < 4 {
                errors.push("Password must be at least 4 characters long".to_string());
            }
        } else {
            errors.push("Password is required for private rooms".to_string());
        }
    }
    
    errors
}
```

### 速率限制

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct RateLimiter {
    limits: Arc<RwLock<HashMap<String, Vec<Instant>>>>,
    max_requests: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window,
        }
    }
    
    pub async fn check_rate_limit(&self, key: &str) -> bool {
        let mut limits = self.limits.write().await;
        let now = Instant::now();
        let window_start = now - self.window;
        
        // 清理过期的请求记录
        let requests = limits.entry(key.to_string()).or_insert_with(Vec::new);
        requests.retain(|&timestamp| timestamp > window_start);
        
        // 检查是否超过限制
        if requests.len() >= self.max_requests as usize {
            return false;
        }
        
        // 记录当前请求
        requests.push(now);
        true
    }
}

pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 获取客户端IP
    let client_ip = req.headers()
        .get("X-Forwarded-For")
        .and_then(|header| header.to_str().ok())
        .and_then(|s| s.split(',').next())
        .unwrap_or_else(|| {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.ip().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });
    
    // 检查速率限制
    let rate_limiter = RateLimiter::new(100, Duration::from_secs(60)); // 100 requests per minute
    if !rate_limiter.check_rate_limit(&client_ip).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    
    Ok(next.run(req).await)
}
```

---

**下一步**: 阅读[06-data-models-design.md](./06-data-models-design.md)了解数据模型的详细设计。
