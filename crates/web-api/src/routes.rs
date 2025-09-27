use axum::{
    extract::{ws::WebSocketUpgrade, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use application::services::{
    AuthenticateUserRequest, CreateRoomRequest, DeleteRoomRequest, InviteMemberRequest,
    LeaveRoomRequest, RegisterUserRequest, RemoveMemberRequest, SendMessageRequest,
    UpdateRoomRequest,
};
use domain::{ChatRoom, ChatRoomVisibility, Message, MessageType, User};

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
    _password: Option<String>,
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

#[derive(Debug, Deserialize)]
struct InviteMemberPayload {
    invitee_id: Uuid, // 被邀请用户的ID
    password: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoveMemberPayload {
    _user_id: Uuid, // 要踢出的用户ID
}

#[derive(Debug, Deserialize)]
struct UpdateRoomPayload {
    name: Option<String>,
    visibility: Option<ChatRoomVisibility>,
    password: Option<String>,
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
        .route("/auth/logout", post(logout_user))
        // 需要认证的路由
        .route("/rooms", post(create_room))
        // 修改：邀请用户加入房间（替代join_room）
        .route("/rooms/{room_id}/members", post(invite_member))
        // 新增：管理路由
        .route("/rooms/{room_id}/members/{user_id}", delete(remove_member))
        .route("/rooms/{room_id}", put(update_room).delete(delete_room))
        .route("/rooms/{room_id}/leave", post(leave_room))
        .route(
            "/rooms/{room_id}/messages",
            post(send_message).get(get_history),
        )
        .route("/rooms/{room_id}/online", get(get_online_users)) // 新增：获取房间在线用户
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

async fn logout_user(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;
    state.user_service.logout(user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_room(
    headers: HeaderMap, // 从请求头获取JWT
    State(state): State<AppState>,
    Json(payload): Json<CreateRoomPayload>,
) -> Result<(StatusCode, Json<ChatRoom>), ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;

    let room = state
        .chat_service
        .create_room(CreateRoomRequest {
            name: payload.name,
            owner_id: user_id, // 使用JWT中的用户ID
            visibility: payload.visibility,
            password: payload.password,
        })
        .await?;

    Ok((StatusCode::CREATED, Json(room)))
}

// 邀请用户加入房间（替代join_room）
async fn invite_member(
    headers: HeaderMap, // 从请求头获取JWT
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<InviteMemberPayload>,
) -> Result<StatusCode, ApiError> {
    let inviter_id = state.jwt_service.extract_user_from_headers(&headers)?;

    state
        .chat_service
        .invite_member(InviteMemberRequest {
            room_id,
            inviter_id,                     // 邀请人（从JWT获取）
            invitee_id: payload.invitee_id, // 被邀请人
            password: payload.password,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn leave_room(
    headers: HeaderMap, // 从请求头获取JWT
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;

    state
        .chat_service
        .leave_room(LeaveRoomRequest {
            room_id,
            user_id, // 使用JWT中的用户ID
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn send_message(
    headers: HeaderMap, // 从请求头获取JWT
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<SendMessagePayload>,
) -> Result<Json<Message>, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;

    let message = state
        .chat_service
        .send_message(SendMessageRequest {
            room_id,
            sender_id: user_id, // 使用JWT中的用户ID
            content: payload.content,
            message_type: payload.message_type,
            reply_to: payload.reply_to,
        })
        .await?;

    Ok(Json(message))
}

async fn get_history(
    headers: HeaderMap, // 需要身份验证才能查看历史
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<Message>>, ApiError> {
    let _user_id = state.jwt_service.extract_user_from_headers(&headers)?; // 验证身份但不使用

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
    token: Option<String>, // 通过查询参数传递JWT token
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
        return Err(ApiError::unauthorized(
            "Missing JWT token in query parameter",
        ));
    };

    Ok(ws.on_upgrade(move |socket| async move {
        match crate::ws_connection::WebSocketConnection::new(socket, state, user_id, query.room_id)
            .await
        {
            Ok(connection) => connection.run().await,
            Err(err) => {
                tracing::error!(?err, "Failed to create WebSocket connection");
            }
        }
    }))
}

// 踢出房间成员（只有owner和admin可以）
async fn remove_member(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let operator_id = state.jwt_service.extract_user_from_headers(&headers)?;

    state
        .chat_service
        .remove_member(RemoveMemberRequest {
            room_id,
            operator_id,
            target_user_id: user_id,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// 更新房间信息（只有owner和admin可以）
async fn update_room(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Json(payload): Json<UpdateRoomPayload>,
) -> Result<Json<ChatRoom>, ApiError> {
    let operator_id = state.jwt_service.extract_user_from_headers(&headers)?;

    let room = state
        .chat_service
        .update_room(UpdateRoomRequest {
            room_id,
            operator_id,
            name: payload.name,
            visibility: payload.visibility,
            password: payload.password,
        })
        .await?;

    Ok(Json(room))
}

//  删除房间（只有owner可以）
async fn delete_room(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let operator_id = state.jwt_service.extract_user_from_headers(&headers)?;

    state
        .chat_service
        .delete_room(DeleteRoomRequest {
            room_id,
            operator_id,
        })
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// 获取房间在线用户列表
async fn get_online_users(
    headers: HeaderMap, // 需要认证才能查看在线用户
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, ApiError> {
    let _user_id = state.jwt_service.extract_user_from_headers(&headers)?; // 验证身份但不使用

    let room_id_domain = domain::RoomId::from(room_id);

    // 获取房间在线用户
    let online_users = state
        .presence_manager
        .get_online_users(room_id_domain)
        .await
        .map_err(|err| {
            ApiError::internal_server_error(format!("Failed to get online users: {}", err))
        })?;

    // 转换为UUID列表返回
    let user_ids: Vec<Uuid> = online_users
        .into_iter()
        .map(|user_id| user_id.into())
        .collect();

    Ok(Json(user_ids))
}
