//! WebSocket 处理器
//!
//! 实现WebSocket连接升级、认证、消息路由和实时通信功能。

use axum::{
    extract::{ws::WebSocket, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::Response,
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::AppState;
use domain::entities::websocket::*;
use domain::services::websocket_service::{ConnectionManager, MessageRouter};
use domain::services::auth_service::AuthService;
use application::services::{ChatRoomService, UserService};
use infrastructure::websocket::*;

/// WebSocket消息传输类型
#[derive(Debug, Clone, PartialEq)]
pub enum WebSocketMessageType {
    Text,
    Binary,
    Ping,
    Pong,
    Close,
}

/// WebSocket连接查询参数
#[derive(Debug, Deserialize)]
pub struct WebSocketQuery {
    /// JWT access token
    pub token: String,
}

/// WebSocket处理器状态
pub struct WebSocketHandler {
    /// 连接管理器
    connection_manager: Arc<InMemoryConnectionManager>,
    /// 消息路由器
    message_router: Arc<InMemoryMessageRouter>,
    /// 房间管理器
    room_manager: Arc<InMemoryRoomManager>,
}

impl WebSocketHandler {
    pub fn new() -> Self {
        Self {
            connection_manager: Arc::new(InMemoryConnectionManager::new()),
            message_router: Arc::new(InMemoryMessageRouter::new()),
            room_manager: Arc::new(InMemoryRoomManager::new()),
        }
    }

    /// 处理WebSocket连接升级
    pub async fn handle_upgrade(
        ws: WebSocketUpgrade,
        state: State<AppState>,
        query: Query<WebSocketQuery>,
    ) -> Result<Response, StatusCode> {
        // 验证JWT token
        let token = &query.token;
        if token.is_empty() {
            warn!("WebSocket upgrade failed: empty token");
            return Err(StatusCode::UNAUTHORIZED);
        }

        // 验证token并获取用户信息
        let user_id = match Self::validate_token(&state, token).await {
            Ok(user_id) => user_id,
            Err(_) => {
                warn!("WebSocket upgrade failed: invalid token");
                return Err(StatusCode::UNAUTHORIZED);
            }
        };

        info!("WebSocket upgrade for user: {}", user_id);

        // 创建WebSocket处理器实例
        let handler = Arc::new(Self::new());

        // 升级连接
        Ok(ws.on_upgrade(move |socket| {
            Self::handle_socket(socket, user_id, handler, state.0)
        }))
    }

    /// 验证JWT token
    async fn validate_token(state: &AppState, token: &str) -> Result<Uuid, StatusCode> {
        match state.jwt_service.validate_token(token).await {
            Ok(claims) => {
                // 从token中提取用户ID
                let user_id_str = claims.sub;
                Uuid::parse_str(&user_id_str).map_err(|_| StatusCode::UNAUTHORIZED)
            }
            Err(_) => Err(StatusCode::UNAUTHORIZED),
        }
    }

    /// 处理WebSocket连接
    async fn handle_socket(
        socket: WebSocket,
        user_id: Uuid,
        handler: Arc<WebSocketHandler>,
        app_state: AppState,
    ) {
        info!("New WebSocket connection for user: {}", user_id);

        // 分离socket的发送和接收部分
        let (mut sender, mut receiver) = socket.split();

        // 创建消息通道
        let (tx, mut rx) = mpsc::unbounded_channel::<WebSocketFrame>();

        // 获取用户信息
        let username = match app_state.user_service.get_user_by_id(user_id).await {
            Ok(user) => user.username,
            Err(_) => {
                error!("Failed to get user info for user: {}", user_id);
                return;
            }
        };

        // 创建连接信息
        let connection_info = ConnectionInfo::new(
            user_id,
            username.clone(),
            ClientInfo::new(None, None), // 可以从request headers中获取更多信息
        );
        let connection_id = connection_info.connection_id;

        // 注册连接
        if let Err(e) = handler
            .connection_manager
            .register_connection(connection_info)
            .await
        {
            error!("Failed to register connection: {}", e);
            return;
        }

        // 注册消息路由器
        handler
            .message_router
            .register_sender(connection_id, tx)
            .await;

        // 设置连接为已认证状态
        if let Err(e) = handler
            .connection_manager
            .update_connection_status(connection_id, ConnectionStatus::Authenticated)
            .await
        {
            error!("Failed to update connection status: {}", e);
        }

        // 发送欢迎消息
        let welcome_message = WebSocketFrame::new(
            MessageType::ServerToClient(domain::entities::websocket::ServerMessage::MessageSent {
                message_id: Uuid::new_v4(),
                room_id: "system".to_string(),
                sender_id: Uuid::nil(),
                sender_username: "system".to_string(),
                content: format!("Welcome, {}!", username),
                message_type: "text".to_string(),
                timestamp: chrono::Utc::now(),
                reply_to: None,
            }),
            serde_json::Value::Null,
        );

        if let Err(e) = handler
            .message_router
            .route_to_connection(connection_id, welcome_message)
            .await
        {
            error!("Failed to send welcome message: {}", e);
        }

        // 启动发送任务
        let send_handler = handler.clone();
        let send_connection_id = connection_id;
        let send_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                // 将WebSocketFrame序列化为JSON字符串发送
                match message.to_json() {
                    Ok(json_string) => {
                        if let Err(e) = sender
                            .send(axum::extract::ws::Message::Text(json_string.into()))
                            .await
                        {
                            error!("Failed to send text message: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize WebSocket message: {}", e);
                        break;
                    }
                }
            }

            debug!("Send task completed for connection: {}", send_connection_id);
        });

        // 处理接收到的消息
        let recv_handler = handler.clone();
        let recv_connection_id = connection_id;
        let recv_user_id = user_id;
        let recv_app_state = app_state;
        let recv_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(axum::extract::ws::Message::Text(text)) => {
                        if let Err(e) = Self::handle_text_message(
                            &recv_handler,
                            &recv_app_state,
                            recv_connection_id,
                            recv_user_id,
                            text.to_string(),
                        )
                        .await
                        {
                            error!("Failed to handle text message: {}", e);
                        }
                    }
                    Ok(axum::extract::ws::Message::Binary(_)) => {
                        debug!("Received binary message (not supported)");
                    }
                    Ok(axum::extract::ws::Message::Ping(data)) => {
                        // 自动回复pong
                        let pong_frame = WebSocketFrame::pong();
                        if let Err(e) = recv_handler
                            .message_router
                            .route_to_connection(recv_connection_id, pong_frame)
                            .await
                        {
                            error!("Failed to send pong: {}", e);
                        }
                    }
                    Ok(axum::extract::ws::Message::Pong(_)) => {
                        // 更新连接活动时间
                        if let Err(e) = recv_handler
                            .connection_manager
                            .update_connection_activity(recv_connection_id)
                            .await
                        {
                            error!("Failed to update connection activity: {}", e);
                        }
                    }
                    Ok(axum::extract::ws::Message::Close(_)) => {
                        info!("WebSocket connection closed by client: {}", recv_connection_id);
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }

            debug!("Receive task completed for connection: {}", recv_connection_id);
        });

        // 等待任一任务完成
        tokio::select! {
            _ = send_task => {
                debug!("Send task finished for connection: {}", connection_id);
            }
            _ = recv_task => {
                debug!("Receive task finished for connection: {}", connection_id);
            }
        }

        // 清理连接
        handler.message_router.unregister_sender(connection_id).await;
        if let Err(e) = handler
            .connection_manager
            .unregister_connection(connection_id)
            .await
        {
            error!("Failed to unregister connection: {}", e);
        }

        info!("WebSocket connection cleaned up: {}", connection_id);
    }

    /// 处理文本消息
    async fn handle_text_message(
        handler: &WebSocketHandler,
        app_state: &AppState,
        connection_id: Uuid,
        user_id: Uuid,
        text: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 更新连接活动时间
        handler
            .connection_manager
            .update_connection_activity(connection_id)
            .await?;

        // 解析客户端消息
        let client_message: ClientMessage = serde_json::from_str(&text)?;

        match client_message {
            ClientMessage::JoinRoom { room_id, password } => {
                Self::handle_join_room(handler, app_state, connection_id, user_id, room_id, password)
                    .await?;
            }
            ClientMessage::LeaveRoom { room_id } => {
                Self::handle_leave_room(handler, connection_id, user_id, room_id).await?;
            }
            ClientMessage::SendMessage {
                room_id,
                content,
                message_type,
            } => {
                Self::handle_send_message(
                    handler,
                    app_state,
                    connection_id,
                    user_id,
                    room_id,
                    content,
                    message_type.unwrap_or(domain::entities::message::MessageType::Text),
                )
                .await?;
            }
            ClientMessage::Ping => {
                // 发送pong响应
                let pong_message = WebSocketFrame::pong();
                handler
                    .message_router
                    .route_to_connection(connection_id, pong_message)
                    .await?;
            }
        }

        Ok(())
    }

    /// 处理加入房间
    async fn handle_join_room(
        handler: &WebSocketHandler,
        app_state: &AppState,
        connection_id: Uuid,
        user_id: Uuid,
        room_id: String,
        password: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 验证用户是否有权限加入房间
        if let Err(e) = app_state
            .chat_service
            .join_room(Uuid::parse_str(&room_id)?, user_id, password.clone())
            .await
        {
            // 发送错误消息
            let error_message = WebSocketFrame::error(
                "JOIN_ROOM_FAILED".to_string(),
                e.to_string(),
                None,
            );
            handler
                .message_router
                .route_to_connection(connection_id, error_message)
                .await?;
            return Ok(());
        }

        // 在WebSocket层面加入房间
        handler
            .connection_manager
            .join_room(connection_id, room_id.clone())
            .await?;

        // 获取连接信息以得到用户名
        let connection_info = handler.connection_manager.get_connection(connection_id).await;
        let username = connection_info.map(|conn| conn.username).unwrap_or_else(|| "Unknown".to_string());

        // 发送成功消息
        let success_message = WebSocketFrame::new(
            MessageType::ServerToClient(domain::entities::websocket::ServerMessage::RoomJoined {
                room_id: room_id.clone(),
                room_name: "Chat Room".to_string(), // TODO: 获取实际房间名称
                user_count: 1, // TODO: 获取实际用户数量
                joined_at: chrono::Utc::now(),
            }),
            serde_json::Value::Null,
        );
        handler
            .message_router
            .route_to_connection(connection_id, success_message)
            .await?;

        // 通知房间内其他用户
        let user_joined_message = WebSocketFrame::new(
            MessageType::ServerToClient(domain::entities::websocket::ServerMessage::UserJoined {
                user_id,
                username, // 从连接信息获取用户名
                room_id: room_id.clone(),
                joined_at: chrono::Utc::now(),
            }),
            serde_json::Value::Null,
        );

        // 获取房间内的其他连接
        let room_connections = handler
            .connection_manager
            .get_room_connections(&room_id)
            .await;

        let other_connection_ids: Vec<Uuid> = room_connections
            .iter()
            .filter(|conn| conn.connection_id != connection_id)
            .map(|conn| conn.connection_id)
            .collect();

        if !other_connection_ids.is_empty() {
            handler
                .message_router
                .route_to_connections(&other_connection_ids, user_joined_message)
                .await?;
        }

        info!("User {} joined room {} via WebSocket", user_id, room_id);
        Ok(())
    }

    /// 处理离开房间
    async fn handle_leave_room(
        handler: &WebSocketHandler,
        connection_id: Uuid,
        user_id: Uuid,
        room_id: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 获取连接信息以得到用户名
        let connection_info = handler.connection_manager.get_connection(connection_id).await;
        let username = connection_info.map(|conn| conn.username).unwrap_or_else(|| "Unknown".to_string());
        
        // 通知房间内其他用户
        let user_left_message = WebSocketFrame::new(
            MessageType::ServerToClient(domain::entities::websocket::ServerMessage::UserLeft {
                user_id,
                username,
                room_id: room_id.clone(),
                left_at: chrono::Utc::now(),
            }),
            serde_json::Value::Null,
        );

        // 获取房间内的其他连接
        let room_connections = handler
            .connection_manager
            .get_room_connections(&room_id)
            .await;

        let other_connection_ids: Vec<Uuid> = room_connections
            .iter()
            .filter(|conn| conn.connection_id != connection_id)
            .map(|conn| conn.connection_id)
            .collect();

        if !other_connection_ids.is_empty() {
            handler
                .message_router
                .route_to_connections(&other_connection_ids, user_left_message)
                .await?;
        }

        // 在WebSocket层面离开房间
        handler
            .connection_manager
            .leave_room(connection_id, &room_id)
            .await?;

        // 发送确认消息
        let left_message = WebSocketFrame::new(
            MessageType::ServerToClient(domain::entities::websocket::ServerMessage::RoomLeft {
                room_id: room_id.clone(),
                left_at: chrono::Utc::now(),
            }),
            serde_json::Value::Null,
        );
        handler
            .message_router
            .route_to_connection(connection_id, left_message)
            .await?;

        info!("User {} left room {} via WebSocket", user_id, room_id);
        Ok(())
    }

    /// 处理发送消息
    async fn handle_send_message(
        handler: &WebSocketHandler,
        app_state: &AppState,
        connection_id: Uuid,
        user_id: Uuid,
        room_id: String,
        content: String,
        message_type: domain::entities::message::MessageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 验证用户是否在房间内
        if !handler
            .connection_manager
            .is_connection_in_room(connection_id, &room_id)
            .await
        {
            let error_message = WebSocketFrame::error(
                "NOT_IN_ROOM".to_string(),
                "You must join the room before sending messages".to_string(),
                None,
            );
            handler
                .message_router
                .route_to_connection(connection_id, error_message)
                .await?;
            return Ok(());
        }

        // 使用应用服务发送消息
        let message = match app_state.chat_service.send_message(
            Uuid::parse_str(&room_id)?,
            user_id,
            content,
            message_type,
            None, // reply_to_message_id
        ).await {
            Ok(message) => message,
            Err(e) => {
                let error_message = WebSocketFrame::error(
                    "SEND_MESSAGE_FAILED".to_string(),
                    e.to_string(),
                    None,
                );
                handler
                    .message_router
                    .route_to_connection(connection_id, error_message)
                    .await?;
                return Ok(());
            }
        };

        // 获取发送者信息
        let sender = app_state.user_service.get_user_by_id(user_id).await?;

        // 广播消息到房间内所有连接
        let message_broadcast = WebSocketFrame::new(
            MessageType::ServerToClient(domain::entities::websocket::ServerMessage::MessageSent {
                message_id: message.id,
                room_id: room_id.clone(),
                sender_id: user_id,
                sender_username: sender.username,
                content: message.content,
                message_type: format!("{:?}", message.message_type),
                timestamp: message.created_at,
                reply_to: None,
            }),
            serde_json::Value::Null,
        );

        let room_connections = handler
            .connection_manager
            .get_room_connections(&room_id)
            .await;

        let connection_ids: Vec<Uuid> = room_connections
            .iter()
            .map(|conn| conn.connection_id)
            .collect();

        if !connection_ids.is_empty() {
            handler
                .message_router
                .route_to_connections(&connection_ids, message_broadcast)
                .await?;
        }

        info!("Message sent to room {} by user {}", room_id, user_id);
        Ok(())
    }
}

/// WebSocket消息类型定义
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// 加入房间
    JoinRoom {
        room_id: String,
        password: Option<String>,
    },
    /// 离开房间
    LeaveRoom {
        room_id: String,
    },
    /// 发送消息
    SendMessage {
        room_id: String,
        content: String,
        message_type: Option<domain::entities::message::MessageType>,
    },
    /// Ping消息
    Ping,
}

/// 服务器发送的消息类型
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// 欢迎消息
    Welcome {
        message: String,
        server_time: chrono::DateTime<chrono::Utc>,
    },
    /// Pong响应
    Pong,
    /// 房间加入成功
    RoomJoined {
        room_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// 房间离开成功
    RoomLeft {
        room_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// 用户加入房间
    UserJoined {
        room_id: String,
        user_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// 用户离开房间
    UserLeft {
        room_id: String,
        user_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// 新消息
    NewMessage {
        room_id: String,
        message_id: Uuid,
        sender_id: Uuid,
        sender_username: String,
        content: String,
        message_type: domain::entities::message::MessageType,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// 错误消息
    Error {
        code: String,
        message: String,
    },
}
