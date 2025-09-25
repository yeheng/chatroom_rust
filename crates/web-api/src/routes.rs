use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use application::services::{AuthenticateUserRequest, CreateRoomRequest, JoinRoomRequest, LeaveRoomRequest, RegisterUserRequest, SendMessageRequest};
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
        .route("/rooms/{room_id}/messages", post(send_message).get(get_history))
        .route("/ws", get(websocket_placeholder))
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

async fn websocket_placeholder() -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
