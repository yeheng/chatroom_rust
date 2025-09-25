use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::StatusCode,
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
use domain::{ChatRoomVisibility, MessageType};

use crate::{error::ApiError, state::AppState};
use application::{MessageDto, RoomDto, UserDto};

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
    owner_id: Uuid,
    visibility: ChatRoomVisibility,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JoinRoomPayload {
    user_id: Uuid,
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SendMessagePayload {
    sender_id: Uuid,
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
        .route("/auth/register", post(register_user))
        .route("/auth/login", post(login_user))
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
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    let dto = state
        .user_service
        .register(RegisterUserRequest {
            username: payload.username,
            email: payload.email,
            password: payload.password,
        })
        .await?;

    Ok((StatusCode::CREATED, Json(dto)))
}

async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<UserDto>, ApiError> {
    let dto = state
        .user_service
        .authenticate(AuthenticateUserRequest {
            email: payload.email,
            password: payload.password,
        })
        .await?;

    Ok(Json(dto))
}

async fn create_room(
    State(state): State<AppState>,
    Json(payload): Json<CreateRoomPayload>,
) -> Result<(StatusCode, Json<RoomDto>), ApiError> {
    let dto = state
        .chat_service
        .create_room(CreateRoomRequest {
            name: payload.name,
            owner_id: payload.owner_id,
            visibility: payload.visibility,
            password: payload.password,
        })
        .await?;

    Ok((StatusCode::CREATED, Json(dto)))
}

async fn join_room(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<JoinRoomPayload>,
) -> Result<StatusCode, ApiError> {
    state
        .chat_service
        .join_room(JoinRoomRequest {
            room_id,
            user_id: payload.user_id,
            password: payload.password,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn leave_room(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<JoinRoomPayload>,
) -> Result<StatusCode, ApiError> {
    state
        .chat_service
        .leave_room(LeaveRoomRequest {
            room_id,
            user_id: payload.user_id,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn send_message(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<SendMessagePayload>,
) -> Result<Json<MessageDto>, ApiError> {
    let dto = state
        .chat_service
        .send_message(SendMessageRequest {
            room_id,
            sender_id: payload.sender_id,
            content: payload.content,
            message_type: payload.message_type,
            reply_to: payload.reply_to,
        })
        .await?;

    Ok(Json(dto))
}

async fn get_history(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<MessageDto>>, ApiError> {
    let limit = query.limit.unwrap_or(50).min(100);
    let items = state
        .chat_service
        .get_history(room_id, limit, query.before)
        .await?;

    Ok(Json(items))
}

#[derive(Debug, Deserialize)]
struct WsQuery {
    #[allow(dead_code)]
    user_id: Uuid,
    room_id: Uuid,
}

async fn websocket_upgrade(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    Ok(ws.on_upgrade(move |socket| websocket_handler(socket, state, query)))
}

async fn websocket_handler(socket: WebSocket, state: AppState, query: WsQuery) {
    let mut receiver = state.broadcaster.subscribe();
    let room_id = query.room_id;

    let (mut sender, mut incoming) = socket.split();

    let send_task = tokio::spawn(async move {
        while let Ok(event) = receiver.recv().await {
            if Uuid::from(event.room_id) != room_id {
                continue;
            }
            let payload = match serde_json::to_string(&MessageDto::from(&event.message)) {
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
