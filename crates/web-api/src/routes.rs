use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{delete, get, post, put},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
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

    Ok(ws.on_upgrade(move |socket| websocket_handler(socket, state, user_id, query.room_id)))
}

async fn websocket_handler(socket: WebSocket, state: AppState, user_id: Uuid, room_id: Uuid) {
    let room_id_domain = domain::RoomId::from(room_id);
    let user_id_domain = domain::UserId::from(user_id);

    tracing::info!(user_id = %user_id, room_id = %room_id, "WebSocket 连接已建立");

    // 用户连接到房间 - 更新在线状态
    if let Err(err) = state
        .presence_manager
        .user_connected(room_id_domain, user_id_domain)
        .await
    {
        tracing::error!(error = %err, "Failed to update user presence");
        return;
    }

    // 创建消息流 - 直接订阅广播器
    let mut message_stream =
        application::MessageStream::new(state.broadcaster.subscribe(), room_id_domain);

    // 清空任何旧消息，只接收连接后发送的新消息
    let mut cleared_count = 0;
    while let Ok(Some(_)) = message_stream.try_recv() {
        cleared_count += 1;
    }
    tracing::info!(cleared_count, "清空了旧消息");

    let (mut sender, mut incoming) = socket.split();

    // 创建 mpsc channel 来解耦对 sender 的访问
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel::<WsCommand>(32);

    // 定义 WebSocket 写操作的命令
    #[derive(Debug)]
    enum WsCommand {
        SendText(String),
        SendPong(Vec<u8>),
    }

    // 创建一个通道用于协调任务
    let (disconnect_tx, mut disconnect_rx) = tokio::sync::oneshot::channel::<()>();

    // 发送任务：统一处理所有对 WebSocket sender 的写操作
    let send_task = {
        let _presence_manager = state.presence_manager.clone();
        let cmd_tx_for_broadcast = cmd_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // 处理来自 mpsc channel 的写命令
                    Some(cmd) = cmd_rx.recv() => {
                        match cmd {
                            WsCommand::SendText(text) => {
                                if sender.send(WsMessage::Text(text.into())).await.is_err() {
                                    tracing::warn!(user_id = %user_id, "Failed to send text message");
                                    break;
                                }
                            }
                            WsCommand::SendPong(data) => {
                                if sender.send(WsMessage::Pong(data.into())).await.is_err() {
                                    tracing::warn!(user_id = %user_id, "Failed to send pong message");
                                    break;
                                }
                            }
                        }
                    }
                    // 处理来自消息流的广播消息
                    Some(broadcast) = message_stream.recv() => {
                        let payload = match serde_json::to_string(&broadcast.message) {
                            Ok(json) => json,
                            Err(err) => {
                                tracing::warn!(error = %err, "failed to serialize websocket payload");
                                continue;
                            }
                        };
                        if cmd_tx_for_broadcast.send(WsCommand::SendText(payload)).await.is_err() {
                            tracing::warn!(user_id = %user_id, "Failed to send broadcast message to command channel");
                            break;
                        }
                    }
                    // 检查是否收到断开连接的信号
                    _ = &mut disconnect_rx => {
                        tracing::info!(user_id = %user_id, "Received disconnect signal");
                        break;
                    }
                }
            }
            tracing::info!(user_id = %user_id, "WebSocket发送任务结束");
        })
    };

    // 接收任务：处理来自WebSocket客户端的消息
    let recv_task = {
        let mut disconnect_tx = Some(disconnect_tx);
        tokio::spawn(async move {
            while let Some(Ok(message)) = incoming.next().await {
                match message {
                    WsMessage::Close(_) => {
                        tracing::info!(user_id = %user_id, "WebSocket收到关闭消息");
                        if let Some(tx) = disconnect_tx.take() {
                            let _ = tx.send(());
                        }
                        break;
                    }
                    WsMessage::Ping(data) => {
                        // 现在可以正确发送pong了！
                        tracing::debug!(user_id = %user_id, "收到ping消息，发送pong回应");
                        if cmd_tx
                            .send(WsCommand::SendPong(data.to_vec()))
                            .await
                            .is_err()
                        {
                            tracing::warn!(user_id = %user_id, "Failed to send pong command");
                            break;
                        }
                    }
                    WsMessage::Pong(_) => {
                        tracing::debug!(user_id = %user_id, "收到pong消息");
                    }
                    WsMessage::Text(_) | WsMessage::Binary(_) => {
                        // 暂时不处理客户端发送的消息，后续可以添加心跳或其他功能
                        tracing::debug!(user_id = %user_id, "收到客户端消息");
                    }
                }
            }
            tracing::info!(user_id = %user_id, "WebSocket接收任务结束");
        })
    };

    // 等待任意一个任务完成（连接断开）
    tokio::select! {
        _ = send_task => {
            tracing::info!(user_id = %user_id, "WebSocket发送任务完成");
        }
        _ = recv_task => {
            tracing::info!(user_id = %user_id, "WebSocket接收任务完成");
        }
    }

    // 连接断开时清理在线状态
    if let Err(err) = state
        .presence_manager
        .user_disconnected(room_id_domain, user_id_domain)
        .await
    {
        tracing::error!(error = %err, user_id = %user_id, room_id = %room_id, "Failed to cleanup user presence");
    }

    tracing::info!(user_id = %user_id, room_id = %room_id, "WebSocket连接已断开，在线状态已清理");
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
