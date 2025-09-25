use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use uuid::Uuid;

use application::services::{
    AuthenticateUserRequest, CreateRoomRequest, JoinRoomRequest, LeaveRoomRequest,
    RegisterUserRequest, SendMessageRequest,
};
use domain::{ChatRoomVisibility, MessageType, User, ChatRoom, Message};

use crate::{error::ApiError, state::AppState, LoginResponse};

#[derive(Debug, Deserialize)]
struct RegisterPayload {
    username: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginPayload {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct CreateRoomPayload {
    name: String,
    visibility: ChatRoomVisibility,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JoinRoomPayload {
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendMessagePayload {
    content: String,
    message_type: MessageType,
    reply_to: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    before: Option<Uuid>,
    limit: Option<u32>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", api_routes())
        .with_state(state)
}

fn api_routes() -> Router<AppState> {
    Router::new()
        // 不需要认证的路由
        .route("/auth/register", post(register_user))
        .route("/auth/login", post(login_user))
        // 需要认证的路由
        .route("/rooms", post(create_room))
        .route("/rooms/{room_id}/join", post(join_room))
        .route("/rooms/{room_id}/leave", post(leave_room))
        .route(
            "/rooms/{room_id}/messages",
            post(send_message).get(get_history),
        )
        .route("/ws", get(websocket_upgrade))
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<RegisterPayload>,
) -> Result<(StatusCode, Json<User>), ApiError> {
    let user = state
        .user_service
        .register(RegisterUserRequest {
            username: payload.username,
            email: payload.email,
            password: payload.password,
        })
        .await?;

    Ok((StatusCode::CREATED, Json(user)))
}

async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<LoginResponse>, ApiError> {
    let user = state
        .user_service
        .authenticate(AuthenticateUserRequest {
            email: payload.email,
            password: payload.password,
        })
        .await?;

    // 生成 JWT token
    let token = state.jwt_service.generate_token(user.id.into())?;

    let response = LoginResponse { user, token };
    Ok(Json(response))
}

async fn create_room(
    headers: HeaderMap,  // 从请求头获取JWT
    State(state): State<AppState>,
    Json(payload): Json<CreateRoomPayload>,
) -> Result<(StatusCode, Json<ChatRoom>), ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;

    let room = state
        .chat_service
        .create_room(CreateRoomRequest {
            name: payload.name,
            owner_id: user_id,  // 使用JWT中的用户ID
            visibility: payload.visibility,
            password: payload.password,
        })
        .await?;

    Ok((StatusCode::CREATED, Json(room)))
}

async fn join_room(
    headers: HeaderMap,  // 从请求头获取JWT
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<JoinRoomPayload>,
) -> Result<StatusCode, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;

    state
        .chat_service
        .join_room(JoinRoomRequest {
            room_id,
            user_id,  // 使用JWT中的用户ID
            password: payload.password,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn leave_room(
    headers: HeaderMap,  // 从请求头获取JWT
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;

    state
        .chat_service
        .leave_room(LeaveRoomRequest {
            room_id,
            user_id,  // 使用JWT中的用户ID
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn send_message(
    headers: HeaderMap,  // 从请求头获取JWT
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<SendMessagePayload>,
) -> Result<Json<Message>, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;

    let message = state
        .chat_service
        .send_message(SendMessageRequest {
            room_id,
            sender_id: user_id,  // 使用JWT中的用户ID
            content: payload.content,
            message_type: payload.message_type,
            reply_to: payload.reply_to,
        })
        .await?;

    Ok(Json(message))
}

async fn get_history(
    headers: HeaderMap,  // 需要身份验证才能查看历史
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<Message>>, ApiError> {
    let _user_id = state.jwt_service.extract_user_from_headers(&headers)?;  // 验证身份但不使用

    let limit = query.limit.unwrap_or(50).min(100);
    let items = state
        .chat_service
        .get_history(room_id, limit, query.before)
        .await?;

    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    room_id: Uuid,
    token: Option<String>,  // 通过查询参数传递JWT token
}

async fn websocket_upgrade(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    // 从查询参数或Authorization header获取用户ID
    let user_id = if let Some(token) = query.token {
        // 从查询参数中的token验证用户
        state.jwt_service.verify_token(&token)?.user_id
    } else {
        return Err(ApiError::unauthorized("Missing JWT token in query parameter"));
    };

    Ok(ws.on_upgrade(move |socket| websocket_handler(socket, state, user_id, query.room_id)))
}

async fn websocket_handler(socket: WebSocket, state: AppState, user_id: Uuid, room_id: Uuid) {
    let room_id = domain::RoomId::from(room_id);

    tracing::info!(user_id = %user_id, room_id = %room_id, "WebSocket 连接已建立");

    // 创建消息流
    let mut message_stream = match state.broadcaster.create_message_stream(room_id).await {
        Ok(stream) => stream,
        Err(err) => {
            tracing::error!(error = %err, "Failed to create message stream");
            return;
        }
    };

    let (mut sender, mut incoming) = socket.split();

    let send_task = tokio::spawn(async move {
        while let Some(broadcast) = message_stream.recv().await {
            let payload = match serde_json::to_string(&broadcast.message) {
                Ok(json) => json,
                Err(err) => {
                    tracing::warn!(error = %err, "failed to serialize websocket payload");
                    continue;
                }
            };
            if sender.send(WsMessage::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = incoming.next().await {
            if matches!(message, WsMessage::Close(_)) {
                break;
            }
        }
    });

    let _ = tokio::join!(send_task, recv_task);
}
