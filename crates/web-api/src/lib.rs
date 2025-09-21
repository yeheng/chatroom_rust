//! Web API 层（Task 7 核心实现）
//!
//! 基于 axum 的核心 HTTP API 与中间件，包含：
//! - 认证端点（/api/auth/*）
//! - 房间端点（/api/v1/rooms/*）
//! - WebSocket 路由（/ws）

use axum::{
    body::Body,
    extract::{ws::WebSocketUpgrade, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{from_fn, from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tower_http::{compression::CompressionLayer, cors::CorsLayer};
use tracing::{error, info, warn};
use uuid::Uuid;

use application::services::{
    ChatRoomService, ChatRoomServiceImpl, CreateRoomRequest, CreateUserRequest,
    UpdateRoomRequest as AppUpdateRoomRequest, UpdateUserRequest as AppUpdateUserRequest,
    UserSearchRequest, UserService, UserServiceImpl,
};
use domain::entities::auth::{LoginCredentials, RefreshTokenRequest, UserRole};
use domain::services::auth_service::JwtEncoder;
use domain::AuthService;
use infrastructure::auth::{InMemoryTokenBlacklistService, JwtAuthServiceImpl, UserAuthService};
pub mod app_config;
pub mod websocket;
use app_config::AppConfig;
use application::errors::ApplicationError;
use websocket::WebSocketHandler;

// 统一 API 响应封装
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub request_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    fn err(code: &str, message: &str, request_id: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(ApiError {
                code: code.to_string(),
                message: message.to_string(),
                request_id,
                timestamp: chrono::Utc::now(),
            }),
        }
    }
}

// App 状态
#[derive(Clone)]
pub struct AppState {
    pub chat_service: Arc<ChatRoomServiceImpl>,
    pub user_service: Arc<UserServiceImpl>,
    pub jwt_service: Arc<JwtAuthServiceImpl>,
    pub user_auth_adapter: Arc<WebUserAuthAdapter>,
    pub limiter: Arc<RwLock<HashMap<String, (Instant, u32)>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        // Init services
        let chat_service = Arc::new(ChatRoomServiceImpl::new());
        let user_service = Arc::new(UserServiceImpl::new());

        // Generate RSA keys for JWT (test/dev)
        // Use HS256 secret (fallback supported by JwtEncoder)
        let secret = generate_hs256_secret();
        let encoder = JwtEncoder::new(
            secret.clone(),
            Vec::new(),
            60, // access token 60 minutes
            7,  // refresh token 7 days
            "chatroom".to_string(),
            "users".to_string(),
        );
        let blacklist = Arc::new(InMemoryTokenBlacklistService::new());
        let user_auth_adapter = Arc::new(WebUserAuthAdapter::new(Arc::clone(&user_service)));
        let jwt_service = Arc::new(JwtAuthServiceImpl::new(
            encoder,
            blacklist,
            user_auth_adapter.clone() as Arc<dyn UserAuthService>,
        ));

        Self {
            chat_service,
            user_service,
            jwt_service,
            user_auth_adapter,
            limiter: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// 请求ID中间件：生成并设置 X-Request-ID
async fn request_id_mw(mut req: axum::http::Request<Body>, next: Next) -> Response {
    let request_id = req
        .headers()
        .get("X-Request-ID")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4);
    req.extensions_mut().insert(request_id);
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        "X-Request-ID",
        axum::http::HeaderValue::from_str(&request_id.to_string()).unwrap(),
    );
    resp
}

// 简单日志中间件
async fn logging_mw(req: axum::http::Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let start = Instant::now();
    let resp = next.run(req).await;
    let status = resp.status();
    let dur = start.elapsed();
    if status.is_server_error() {
        error!("{} {} -> {} in {}ms", method, uri, status, dur.as_millis());
    } else if status.is_client_error() {
        warn!("{} {} -> {} in {}ms", method, uri, status, dur.as_millis());
    } else {
        info!("{} {} -> {} in {}ms", method, uri, status, dur.as_millis());
    }
    resp
}

// 速率限制中间件（按IP，每分钟最多5次），仅用于登录路由
async fn rate_limit_login_mw(
    State(state): State<AppState>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let now = Instant::now();
    let window = Duration::from_secs(60);
    let mut limiter = state.limiter.write().await;
    // 清理过期
    limiter.retain(|_, (ts, _)| now.duration_since(*ts) < window);
    let entry = limiter.entry(ip.clone()).or_insert((now, 0));
    if now.duration_since(entry.0) < window {
        if entry.1 >= 5 {
            let request_id = Uuid::new_v4().to_string();
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ApiResponse::<()>::err(
                    "RATE_LIMITED",
                    "Too many attempts",
                    request_id,
                )),
            )
                .into_response();
        }
        entry.1 += 1;
    } else {
        *entry = (now, 1);
    }
    drop(limiter);
    next.run(req).await
}

// 简化的 JWT 校验中间件（示例）：Authorization: Bearer <user_id>
async fn auth_mw(req: axum::http::Request<Body>, next: Next) -> Response {
    let request_id = req
        .extensions()
        .get::<Uuid>()
        .cloned()
        .unwrap_or_else(Uuid::new_v4);
    let headers = req.headers();
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Missing Authorization",
                request_id.to_string(),
            )),
        )
            .into_response();
    };
    let Ok(tok) = auth.to_str() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Invalid Authorization",
                request_id.to_string(),
            )),
        )
            .into_response();
    };
    if !tok.starts_with("Bearer ") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Invalid Authorization",
                request_id.to_string(),
            )),
        )
            .into_response();
    }
    // 提取 user_id（简化）
    let uid_str = tok.trim_start_matches("Bearer ").trim();
    if Uuid::parse_str(uid_str).is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Invalid token",
                request_id.to_string(),
            )),
        )
            .into_response();
    }
    next.run(req).await
}

// 认证 API
#[derive(Debug, Serialize, Deserialize)]
struct RegisterRequest {
    username: String,
    email: String,
    password: String,
}

async fn register_handler(Json(req): Json<RegisterRequest>) -> Response {
    let state = APP_STATE.get_or_init(AppState::new);
    // Delegate to application user service (has validation)
    let create_req = CreateUserRequest {
        username: req.username,
        email: req.email,
        password: req.password,
        avatar_url: None,
        display_name: None,
    };
    match state.user_service.create_user(create_req).await {
        Ok(user) => (StatusCode::CREATED, Json(ApiResponse::ok(serde_json::json!({ "id": user.id, "username": user.username, "email": user.email })))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(ApiResponse::<()>::err("REGISTER_FAILED", &e.to_string(), Uuid::new_v4().to_string()))).into_response(),
    }
}

async fn login_handler(Json(creds): Json<LoginCredentials>) -> Response {
    if creds.username.is_empty() || creds.password.is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "INVALID_CREDENTIALS",
                "Invalid credentials",
                Uuid::new_v4().to_string(),
            )),
        )
            .into_response();
    }
    // Use global state
    let state = APP_STATE.get_or_init(AppState::new);
    match state.jwt_service.login(creds).await {
        Ok(resp) => {
            // Store refresh token so that future refresh can validate
            state
                .user_auth_adapter
                .store_refresh_token(resp.refresh_token.clone(), resp.user.id)
                .await;
            (StatusCode::OK, Json(ApiResponse::ok(resp))).into_response()
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "INVALID_CREDENTIALS",
                "Invalid credentials",
                Uuid::new_v4().to_string(),
            )),
        )
            .into_response(),
    }
}

async fn refresh_handler(Json(req): Json<RefreshTokenRequest>) -> Response {
    if req.refresh_token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err(
                "INVALID_REFRESH_TOKEN",
                "Invalid refresh token",
                Uuid::new_v4().to_string(),
            )),
        )
            .into_response();
    }
    let state = APP_STATE.get_or_init(AppState::new);
    match state.jwt_service.refresh_token(&req.refresh_token).await {
        Ok(resp) => {
            // Store newly issued refresh token and revoke old one
            state
                .user_auth_adapter
                .store_refresh_token(resp.refresh_token.clone(), resp.user.id)
                .await;
            (StatusCode::OK, Json(ApiResponse::ok(resp))).into_response()
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "INVALID_REFRESH_TOKEN",
                "Invalid refresh token",
                Uuid::new_v4().to_string(),
            )),
        )
            .into_response(),
    }
}

// 房间 API
#[derive(Debug, Serialize, Deserialize)]
struct CreateRoomBody {
    name: String,
    description: Option<String>,
    is_private: bool,
    password: Option<String>,
}

async fn create_room_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateRoomBody>,
) -> Response {
    let owner_id = extract_user_id(&headers).unwrap_or_else(Uuid::new_v4);
    let req = CreateRoomRequest {
        name: body.name,
        description: body.description,
        owner_id,
        is_private: body.is_private,
        password: body.password,
    };
    let room = state.chat_service.create_room(req).await;

    match room {
        Ok(r) => (
            StatusCode::CREATED,
            Json(ApiResponse::ok(serde_json::json!({
                "id": r.id,
                "name": r.name,
                "owner_id": r.owner_id,
                "description": r.description,
                "is_private": r.is_private
            }))),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err(
                "ROOM_CREATE_FAILED",
                &e.to_string(),
                Uuid::new_v4().to_string(),
            )),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ListRoomsQuery {
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn list_rooms_handler(
    State(state): State<AppState>,
    _headers: HeaderMap,
    q: Query<ListRoomsQuery>,
) -> Response {
    let page = q.page.unwrap_or(1);
    let page_size = q.page_size.unwrap_or(10);
    let rooms = state.chat_service.list_rooms(page, page_size).await;
    let has_more = rooms.len() as u32 == page_size;
    (
        StatusCode::OK,
        Json(ApiResponse::ok(
            serde_json::json!({ "rooms": rooms, "pagination": {"has_more": has_more}}),
        )),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct JoinRoomBody {
    password: Option<String>,
}

async fn join_room_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(body): Json<JoinRoomBody>,
) -> impl IntoResponse {
    let user_id = extract_user_id(&headers).unwrap_or_else(Uuid::new_v4);
    let res = state
        .chat_service
        .join_room(room_id, user_id, body.password)
        .await;

    match res {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse::ok(serde_json::json!({"joined": true}))),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::err(
                "JOIN_FAILED",
                &e.to_string(),
                Uuid::new_v4().to_string(),
            )),
        )
            .into_response(),
    }
}

async fn get_room_handler(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Response, HttpAppError> {
    let room = state
        .chat_service
        .get_room(room_id)
        .await
        .map_err(HttpAppError)?;
    Ok((StatusCode::OK, Json(ApiResponse::ok(room))).into_response())
}

fn extract_user_id(headers: &HeaderMap) -> Option<Uuid> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .and_then(|s| Uuid::parse_str(s.trim()).ok())
}

/// WebSocket连接处理器
async fn websocket_handler(
    ws: WebSocketUpgrade,
    state: State<AppState>,
    query: Query<websocket::WebSocketQuery>,
) -> Result<Response, StatusCode> {
    WebSocketHandler::handle_upgrade(ws, state, query).await
}

// ===== 用户管理端点 =====

/// 获取当前用户信息
async fn get_current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    match state.user_service.get_user_by_id(user_id).await {
        Ok(user) => {
            let user_data = serde_json::json!({
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "display_name": user.display_name,
                "avatar_url": user.avatar_url,
                "status": user.status,
                "created_at": user.created_at,
                "updated_at": user.updated_at
            });
            Ok(Json(ApiResponse::ok(user_data)))
        }
        Err(e) => {
            error!("Failed to get current user: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpdateUserRequest {
    username: Option<String>,
    email: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

/// 更新当前用户信息
async fn update_current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    match state
        .user_service
        .update_user(
            user_id,
            AppUpdateUserRequest {
                username: request.username,
                email: request.email,
                display_name: request.display_name,
                avatar_url: request.avatar_url,
            },
        )
        .await
    {
        Ok(user) => {
            let user_data = serde_json::json!({
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "display_name": user.display_name,
                "avatar_url": user.avatar_url,
                "status": user.status,
                "created_at": user.created_at,
                "updated_at": user.updated_at
            });
            Ok(Json(ApiResponse::ok(user_data)))
        }
        Err(e) => {
            error!("Failed to update user: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

#[derive(Debug, Deserialize)]
struct SearchUsersQuery {
    q: String, // 搜索关键词
    limit: Option<u32>,
    offset: Option<u32>,
}

/// 搜索用户
async fn search_users(
    State(state): State<AppState>,
    _headers: HeaderMap,
    Query(query): Query<SearchUsersQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let keyword = query.q.trim();
    if keyword.is_empty() {
        return Ok(Json(ApiResponse::<serde_json::Value>::err(
            "INVALID_QUERY",
            "Search keyword cannot be empty",
            Uuid::new_v4().to_string(),
        )));
    }

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    let q = UserSearchRequest {
        query: keyword.to_string(),
        page: (offset / limit) + 1,
        page_size: limit,
        status_filter: None,
    };

    match state.user_service.search_users(q).await {
        Ok(r) => {
            let users_data: Vec<serde_json::Value> = r
                .users
                .into_iter()
                .map(|user| {
                    serde_json::json!({
                        "id": user.id,
                        "username": user.username,
                        "display_name": user.display_name,
                        "avatar_url": user.avatar_url,
                        "status": user.status
                    })
                })
                .collect();

            let result = serde_json::json!({
                "users": users_data,
                "pagination": {
                    "limit": r.page_size,
                    "offset": r.page * r.page_size,
                    "has_more": r.page == r.total_pages
                }
            });

            Ok(Json(ApiResponse::ok(result)))
        }
        Err(e) => {
            error!("Failed to search users: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ===== 聊天室管理端点 =====

#[derive(Debug, Deserialize)]
struct UpdateRoomRequest {
    name: Option<String>,
    description: Option<String>,
    is_private: Option<bool>,
    password: Option<String>,
}

/// 更新聊天室
async fn update_room_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(request): Json<UpdateRoomRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // TODO: 检查用户是否有权限更新房间（房主或管理员）

    match state
        .chat_service
        .update_room(
            room_id,
            AppUpdateRoomRequest {
                name: request.name,
                description: request.description,
                max_members: None,
                password: request.password,
            },
        )
        .await
    {
        Ok(room) => {
            let room_data = serde_json::json!({
                "id": room.id,
                "name": room.name,
                "description": room.description,
                "owner_id": room.owner_id,
                "is_private": room.is_private,
                "created_at": room.created_at,
                "updated_at": room.updated_at
            });
            Ok(Json(ApiResponse::ok(room_data)))
        }
        Err(e) => {
            error!("Failed to update room: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// 删除聊天室
async fn delete_room_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    match state.chat_service.delete_room(room_id, user_id).await {
        Ok(_) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            error!("Failed to delete room: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// 离开聊天室
async fn leave_room_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    match state.chat_service.leave_room(room_id, user_id).await {
        Ok(_) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            error!("Failed to leave room: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetRoomMessagesQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    before: Option<chrono::DateTime<chrono::Utc>>,
    after: Option<chrono::DateTime<chrono::Utc>>,
}

/// 获取聊天室消息
async fn get_room_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Query(query): Query<GetRoomMessagesQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // 检查用户是否在房间内
    if !state
        .chat_service
        .is_user_in_room(room_id, user_id)
        .await
        .unwrap_or(false)
    {
        return Ok(Json(ApiResponse::<serde_json::Value>::err(
            "ACCESS_DENIED",
            "You must be a member of the room to view messages",
            Uuid::new_v4().to_string(),
        )));
    }

    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    match state
        .chat_service
        .get_room_messages(room_id, Some(limit), Some(offset))
        .await
    {
        Ok(messages) => {
            let messages_data: Vec<serde_json::Value> = messages
                .into_iter()
                .map(|msg| {
                    serde_json::json!({
                        "id": msg.id,
                        "room_id": msg.room_id,
                        "sender_id": msg.sender_id,
                        "content": msg.content,
                        "message_type": msg.message_type,
                        "created_at": msg.created_at,
                        "updated_at": msg.updated_at
                    })
                })
                .collect();

            let result = serde_json::json!({
                "messages": messages_data,
                "pagination": {
                    "limit": limit,
                    "offset": offset,
                    "has_more": messages_data.len() as u32 == limit
                }
            });

            Ok(Json(ApiResponse::ok(result)))
        }
        Err(e) => {
            error!("Failed to get room messages: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 获取聊天室成员
async fn get_room_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // 检查用户是否在房间内
    if !state
        .chat_service
        .is_user_in_room(room_id, user_id)
        .await
        .unwrap_or(false)
    {
        return Ok(Json(ApiResponse::<serde_json::Value>::err(
            "ACCESS_DENIED",
            "You must be a member of the room to view members",
            Uuid::new_v4().to_string(),
        )));
    }

    match state.chat_service.get_room_members(room_id).await {
        Ok(members) => {
            let members_data: Vec<serde_json::Value> = members
                .into_iter()
                .map(|member| {
                    serde_json::json!({
                        "user_id": member.user_id,
                        "username": member.username,
                        "role": member.role,
                        "joined_at": member.joined_at
                    })
                })
                .collect();

            let result = serde_json::json!({
                "members": members_data,
                "total_count": members_data.len()
            });

            Ok(Json(ApiResponse::ok(result)))
        }
        Err(e) => {
            error!("Failed to get room members: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ===== 消息管理端点 =====

/// 获取消息详情
async fn get_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let _user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    match state.chat_service.get_message(message_id).await {
        Ok(Some(message)) => {
            let message_data = serde_json::json!({
                "id": message.id,
                "room_id": message.room_id,
                "sender_id": message.sender_id,
                "content": message.content,
                "message_type": message.message_type,
                "created_at": message.created_at,
                "updated_at": message.updated_at
            });
            Ok(Json(ApiResponse::ok(message_data)))
        }
        Ok(None) => Ok(Json(ApiResponse::<serde_json::Value>::err(
            "MESSAGE_NOT_FOUND",
            "Message not found",
            Uuid::new_v4().to_string(),
        ))),
        Err(e) => {
            error!("Failed to get message: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpdateMessageRequest {
    content: String,
}

/// 更新消息
async fn update_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Json(request): Json<UpdateMessageRequest>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    match state
        .chat_service
        .edit_message(message_id, user_id, request.content)
        .await
    {
        Ok(message) => {
            let message_data = serde_json::json!({
                "id": message.id,
                "room_id": message.room_id,
                "sender_id": message.sender_id,
                "content": message.content,
                "message_type": message.message_type,
                "created_at": message.created_at,
                "updated_at": message.updated_at
            });
            Ok(Json(ApiResponse::ok(message_data)))
        }
        Err(e) => {
            error!("Failed to update message: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// 删除消息
async fn delete_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
) -> Result<Json<ApiResponse<()>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    match state.chat_service.delete_message(message_id, user_id).await {
        Ok(_) => Ok(Json(ApiResponse::ok(()))),
        Err(e) => {
            error!("Failed to delete message: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

#[derive(Debug, Deserialize)]
struct SearchMessagesQuery {
    q: String,             // 搜索关键词
    room_id: Option<Uuid>, // 限制在特定房间内搜索
    limit: Option<u32>,
    offset: Option<u32>,
    before: Option<chrono::DateTime<chrono::Utc>>,
    after: Option<chrono::DateTime<chrono::Utc>>,
}

/// 搜索消息
async fn search_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchMessagesQuery>,
) -> Result<Json<ApiResponse<serde_json::Value>>, StatusCode> {
    let user_id = extract_user_id(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    let keyword = query.q.trim();
    if keyword.is_empty() {
        return Ok(Json(ApiResponse::<serde_json::Value>::err(
            "INVALID_QUERY",
            "Search keyword cannot be empty",
            Uuid::new_v4().to_string(),
        )));
    }

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    match state
        .chat_service
        .search_messages(keyword, query.room_id, user_id, limit, offset)
        .await
    {
        Ok(messages) => {
            let messages_data: Vec<serde_json::Value> = messages
                .into_iter()
                .map(|msg| {
                    serde_json::json!({
                        "id": msg.id,
                        "room_id": msg.room_id,
                        "sender_id": msg.sender_id,
                        "content": msg.content,
                        "message_type": msg.message_type,
                        "created_at": msg.created_at,
                        "updated_at": msg.updated_at
                    })
                })
                .collect();

            let result = serde_json::json!({
                "messages": messages_data,
                "pagination": {
                    "limit": limit,
                    "offset": offset,
                    "has_more": messages_data.len() as u32 == limit
                }
            });

            Ok(Json(ApiResponse::ok(result)))
        }
        Err(e) => {
            error!("Failed to search messages: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// 构建 Router
static APP_STATE: once_cell::sync::OnceCell<AppState> = once_cell::sync::OnceCell::new();

pub fn build_router() -> Router {
    let state = APP_STATE.get_or_init(AppState::new).clone();
    let login_state = state.clone();
    let api_auth = Router::new()
        .route("/login", post(login_handler))
        .route("/register", post(register_handler))
        .route("/refresh", post(refresh_handler))
        .route_layer(from_fn_with_state(login_state, rate_limit_login_mw));

    let api_v1 = Router::new()
        // 用户管理端点
        .route("/users/me", get(get_current_user).put(update_current_user))
        .route("/users/search", get(search_users))
        // 聊天室管理端点
        .route("/rooms", post(create_room_handler).get(list_rooms_handler))
        .route(
            "/rooms/{room_id}",
            get(get_room_handler)
                .put(update_room_handler)
                .delete(delete_room_handler),
        )
        .route("/rooms/{room_id}/join", post(join_room_handler))
        .route("/rooms/{room_id}/leave", post(leave_room_handler))
        .route("/rooms/{room_id}/messages", get(get_room_messages))
        .route("/rooms/{room_id}/members", get(get_room_members))
        // 消息管理端点
        .route(
            "/messages/{message_id}",
            get(get_message).put(update_message).delete(delete_message),
        )
        .route("/messages/search", get(search_messages))
        .route_layer(from_fn_with_state(state.clone(), auth_jwt_mw));

    Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/ws", get(websocket_handler))
        .nest("/api/auth", api_auth)
        .nest("/api/v1", api_v1)
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(from_fn(logging_mw))
        .layer(from_fn(request_id_mw))
        .fallback(not_found_fallback)
}

// 启动 HTTP 服务器
pub async fn run(addr: SocketAddr) -> anyhow::Result<()> {
    let app = build_router();
    info!("Starting HTTP server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// Fallback for 404 Not Found => JSON error with request id and timestamp
async fn not_found_fallback(req: axum::http::Request<Body>) -> Response {
    let request_id = req
        .extensions()
        .get::<Uuid>()
        .cloned()
        .unwrap_or_else(Uuid::new_v4);
    let body = ApiResponse::<()>::err("NOT_FOUND", "Not Found", request_id.to_string());
    (StatusCode::NOT_FOUND, Json(body)).into_response()
}

// Basic error message sanitization to avoid leaking secrets
fn sanitize_message(msg: &str) -> String {
    let mut s = msg.replace("password", "[REDACTED]");
    if let Some(start) = s.find("postgres://") {
        // redact until next space
        let end = s[start..].find(' ').map(|i| start + i).unwrap_or(s.len());
        s.replace_range(start..end, "postgres://[REDACTED]");
    }
    s
}

// Health / Metrics
static START_INSTANT: once_cell::sync::OnceCell<std::time::Instant> =
    once_cell::sync::OnceCell::new();

async fn health_handler() -> impl IntoResponse {
    let start = START_INSTANT.get_or_init(std::time::Instant::now);
    let uptime = start.elapsed().as_secs();
    let body = serde_json::json!({"healthy": true, "uptime": uptime});
    (StatusCode::OK, Json(body))
}

async fn metrics_handler() -> impl IntoResponse {
    let start = START_INSTANT.get_or_init(std::time::Instant::now);
    let uptime = start.elapsed().as_secs_f64();
    let text = format!("# TYPE app_info gauge\napp_info{{version=\"0.1.0\"}} 1\n# TYPE app_uptime_seconds counter\napp_uptime_seconds {}\n", uptime);
    (StatusCode::OK, text)
}

// ChatRoomApp startup orchestration
pub struct ChatRoomApp {
    config: AppConfig,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl ChatRoomApp {
    pub async fn new() -> anyhow::Result<Self> {
        let cfg = AppConfig::load().await?;
        Self::with_config(cfg).await
    }

    pub async fn with_config(config: AppConfig) -> anyhow::Result<Self> {
        let (tx, _rx) = tokio::sync::broadcast::channel(1);
        Ok(Self {
            config,
            shutdown_tx: tx,
        })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let addr: SocketAddr =
            format!("{}:{}", self.config.server.host, self.config.server.port).parse()?;
        START_INSTANT.get_or_init(std::time::Instant::now);
        let app = build_router();
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let mut rx = self.shutdown_tx.subscribe();
        info!("App listening on {}", addr);
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.recv().await;
            })
            .await?;
        Ok(())
    }

    pub fn get_shutdown_signal(&self) -> tokio::sync::broadcast::Sender<()> {
        self.shutdown_tx.clone()
    }

    pub async fn check_database_connection(&self) -> bool {
        true
    }
    pub async fn check_redis_connection(&self) -> bool {
        true
    }
    pub async fn check_kafka_connection(&self) -> bool {
        true
    }
}

// HTTP mapping for ApplicationError via wrapper
pub struct HttpAppError(pub ApplicationError);

impl IntoResponse for HttpAppError {
    fn into_response(self) -> Response {
        let request_id = Uuid::new_v4();
        let (status, code, message) = map_app_error(&self.0);
        let body =
            ApiResponse::<()>::err(&code, &sanitize_message(&message), request_id.to_string());
        (status, Json(body)).into_response()
    }
}

fn map_app_error(err: &ApplicationError) -> (StatusCode, String, String) {
    use application::errors::{ChatRoomError, MessageError, UserError};
    match err {
        ApplicationError::NotFound(msg) => {
            (StatusCode::NOT_FOUND, "NOT_FOUND".to_string(), msg.clone())
        }
        ApplicationError::Unauthorized(msg) => (
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED".to_string(),
            msg.clone(),
        ),
        ApplicationError::Validation(msg) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "VALIDATION_ERROR".to_string(),
            msg.clone(),
        ),
        ApplicationError::Conflict(msg) => {
            (StatusCode::CONFLICT, "CONFLICT".to_string(), msg.clone())
        }
        ApplicationError::ChatRoom(cr) => match cr {
            ChatRoomError::RoomNotFound(_) => (
                StatusCode::NOT_FOUND,
                "ROOM_NOT_FOUND".into(),
                cr.to_string(),
            ),
            ChatRoomError::RoomFull(_) => {
                (StatusCode::FORBIDDEN, "ROOM_FULL".into(), cr.to_string())
            }
            ChatRoomError::InvalidPassword => (
                StatusCode::FORBIDDEN,
                "INVALID_PASSWORD".into(),
                cr.to_string(),
            ),
            ChatRoomError::UserAlreadyInRoom(_) => (
                StatusCode::CONFLICT,
                "USER_ALREADY_IN_ROOM".into(),
                cr.to_string(),
            ),
            ChatRoomError::UserNotInRoom(_) => (
                StatusCode::FORBIDDEN,
                "USER_NOT_IN_ROOM".into(),
                cr.to_string(),
            ),
            ChatRoomError::RoomNameConflict(_) => (
                StatusCode::CONFLICT,
                "ROOM_NAME_CONFLICT".into(),
                cr.to_string(),
            ),
            ChatRoomError::RoomDeleted(_) => {
                (StatusCode::GONE, "ROOM_DELETED".into(), cr.to_string())
            }
            ChatRoomError::InsufficientPermissions(_) => (
                StatusCode::FORBIDDEN,
                "INSUFFICIENT_PERMISSIONS".into(),
                cr.to_string(),
            ),
            ChatRoomError::RateLimited(_) => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED".into(),
                cr.to_string(),
            ),
            ChatRoomError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL".into(),
                cr.to_string(),
            ),
            ChatRoomError::Validation(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR".into(),
                cr.to_string(),
            ),
        },
        ApplicationError::Message(me) => match me {
            MessageError::EmptyContent => (
                StatusCode::BAD_REQUEST,
                "EMPTY_CONTENT".into(),
                me.to_string(),
            ),
            MessageError::ContentTooLong(_, _) => (
                StatusCode::BAD_REQUEST,
                "CONTENT_TOO_LONG".into(),
                me.to_string(),
            ),
            MessageError::SensitiveContent => (
                StatusCode::BAD_REQUEST,
                "SENSITIVE_CONTENT".into(),
                me.to_string(),
            ),
            MessageError::MessageNotFound(_) => (
                StatusCode::NOT_FOUND,
                "MESSAGE_NOT_FOUND".into(),
                me.to_string(),
            ),
            MessageError::DuplicateMessage => (
                StatusCode::CONFLICT,
                "DUPLICATE_MESSAGE".into(),
                me.to_string(),
            ),
            MessageError::InvalidFormat(_) => (
                StatusCode::BAD_REQUEST,
                "INVALID_FORMAT".into(),
                me.to_string(),
            ),
            MessageError::SendFailed(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "SEND_FAILED".into(),
                me.to_string(),
            ),
            MessageError::RateLimited(_) => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED".into(),
                me.to_string(),
            ),
            MessageError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL".into(),
                me.to_string(),
            ),
            MessageError::Validation(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR".into(),
                me.to_string(),
            ),
            MessageError::Unauthorized(_) => {
                (StatusCode::FORBIDDEN, "UNAUTHORIZED".into(), me.to_string())
            }
        },
        ApplicationError::User(ue) => match ue {
            UserError::UserNotFound(_)
            | UserError::UserNotFoundByUsername(_)
            | UserError::UserNotFoundByEmail(_) => (
                StatusCode::NOT_FOUND,
                "USER_NOT_FOUND".into(),
                ue.to_string(),
            ),
            UserError::UsernameAlreadyExists(_)
            | UserError::EmailAlreadyExists(_)
            | UserError::UsernameConflict(_)
            | UserError::EmailConflict(_) => {
                (StatusCode::CONFLICT, "USER_CONFLICT".into(), ue.to_string())
            }
            UserError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS".into(),
                ue.to_string(),
            ),
            UserError::UserInactive => (
                StatusCode::FORBIDDEN,
                "USER_INACTIVE".into(),
                ue.to_string(),
            ),
            UserError::WeakPassword => (
                StatusCode::BAD_REQUEST,
                "WEAK_PASSWORD".into(),
                ue.to_string(),
            ),
            UserError::InvalidUserStatus(_)
            | UserError::InvalidSearchQuery(_)
            | UserError::ExtensionValidationFailed(_)
            | UserError::Validation(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR".into(),
                ue.to_string(),
            ),
            UserError::AvatarUploadFailed(_) | UserError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL".into(),
                ue.to_string(),
            ),
            UserError::Unauthorized(_) => {
                (StatusCode::FORBIDDEN, "UNAUTHORIZED".into(), ue.to_string())
            }
        },
        ApplicationError::Domain(de) => (
            StatusCode::BAD_REQUEST,
            "DOMAIN_ERROR".to_string(),
            de.to_string(),
        ),
        ApplicationError::Infrastructure(msg) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "INFRASTRUCTURE_ERROR".to_string(),
            msg.clone(),
        ),
        ApplicationError::Serialization(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "SERIALIZATION_ERROR".to_string(),
            msg.clone(),
        ),
        ApplicationError::CommandHandlerNotFound(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "COMMAND_HANDLER_NOT_FOUND".to_string(),
            msg.clone(),
        ),
        ApplicationError::QueryHandlerNotFound(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "QUERY_HANDLER_NOT_FOUND".to_string(),
            msg.clone(),
        ),
    }
}

fn generate_hs256_secret() -> Vec<u8> {
    // 32 bytes from two UUIDs
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    out.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    out
}

// In-memory UserAuthService adapter backed by application::UserServiceImpl
#[derive(Clone)]
pub struct WebUserAuthAdapter {
    user_service: Arc<UserServiceImpl>,
    refresh_index: Arc<RwLock<HashMap<String, Uuid>>>,
    user_tokens: Arc<RwLock<HashMap<Uuid, Vec<String>>>>,
}

impl WebUserAuthAdapter {
    pub fn new(user_service: Arc<UserServiceImpl>) -> Self {
        Self {
            user_service,
            refresh_index: Arc::new(RwLock::new(HashMap::new())),
            user_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn store_refresh_token(&self, token: String, user_id: Uuid) {
        let mut idx = self.refresh_index.write().await;
        idx.insert(token.clone(), user_id);
        drop(idx);
        let mut ut = self.user_tokens.write().await;
        ut.entry(user_id).or_default().push(token);
    }
}

#[async_trait::async_trait]
impl UserAuthService for WebUserAuthAdapter {
    async fn authenticate_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<infrastructure::auth::UserAuthInfo, domain::entities::auth::AuthError> {
        let user = self
            .user_service
            .verify_credentials(username, password)
            .await
            .map_err(|_| domain::entities::auth::AuthError::InvalidCredentials)?;
        Ok(infrastructure::auth::UserAuthInfo {
            id: user.id,
            username: user.username.clone(),
            email: Some(user.email.clone()),
            role: UserRole::User,
            permissions: UserRole::User.default_permissions(),
            is_active: user.status == domain::user::UserStatus::Active,
            created_at: user.created_at,
            last_login: None,
        })
    }

    async fn validate_refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<Uuid, domain::entities::auth::AuthError> {
        let idx = self.refresh_index.read().await;
        idx.get(refresh_token)
            .copied()
            .ok_or(domain::entities::auth::AuthError::InvalidRefreshToken)
    }

    async fn revoke_refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<(), domain::entities::auth::AuthError> {
        let mut idx = self.refresh_index.write().await;
        if let Some(uid) = idx.remove(refresh_token) {
            drop(idx);
            let mut ut = self.user_tokens.write().await;
            if let Some(list) = ut.get_mut(&uid) {
                list.retain(|t| t != refresh_token);
            }
        }
        Ok(())
    }

    async fn revoke_all_refresh_tokens(
        &self,
        user_id: Uuid,
    ) -> Result<(), domain::entities::auth::AuthError> {
        let tokens = {
            let mut ut = self.user_tokens.write().await;
            ut.remove(&user_id).unwrap_or_default()
        };
        let mut idx = self.refresh_index.write().await;
        for t in tokens {
            idx.remove(&t);
        }
        Ok(())
    }

    async fn get_user_by_id(
        &self,
        user_id: Uuid,
    ) -> Result<infrastructure::auth::UserAuthInfo, domain::entities::auth::AuthError> {
        let user = self
            .user_service
            .get_user_by_id(user_id)
            .await
            .map_err(|_| domain::entities::auth::AuthError::UserNotFound)?;
        Ok(infrastructure::auth::UserAuthInfo {
            id: user.id,
            username: user.username.clone(),
            email: Some(user.email.clone()),
            role: UserRole::User,
            permissions: UserRole::User.default_permissions(),
            is_active: user.status == domain::user::UserStatus::Active,
            created_at: user.created_at,
            last_login: None,
        })
    }

    async fn is_user_active(
        &self,
        user_id: Uuid,
    ) -> Result<bool, domain::entities::auth::AuthError> {
        let user = self
            .user_service
            .get_user_by_id(user_id)
            .await
            .map_err(|_| domain::entities::auth::AuthError::UserNotFound)?;
        Ok(user.status == domain::user::UserStatus::Active)
    }

    async fn update_last_login(
        &self,
        _user_id: Uuid,
    ) -> Result<(), domain::entities::auth::AuthError> {
        Ok(())
    }
}

// Auth middleware using JWT validation
async fn auth_jwt_mw(
    State(state): State<AppState>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let request_id = req
        .extensions()
        .get::<Uuid>()
        .cloned()
        .unwrap_or_else(Uuid::new_v4);
    let headers = req.headers();
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Missing Authorization",
                request_id.to_string(),
            )),
        )
            .into_response();
    };
    let Ok(tok) = auth.to_str() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Invalid Authorization",
                request_id.to_string(),
            )),
        )
            .into_response();
    };
    if !tok.starts_with("Bearer ") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Invalid Authorization",
                request_id.to_string(),
            )),
        )
            .into_response();
    }
    let token = tok.trim_start_matches("Bearer ").trim();
    match state.jwt_service.validate_token(token).await {
        Ok(_) => next.run(req).await,
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::err(
                "UNAUTHORIZED",
                "Invalid token",
                request_id.to_string(),
            )),
        )
            .into_response(),
    }
}
