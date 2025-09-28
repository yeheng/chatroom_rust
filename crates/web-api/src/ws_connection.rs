use crate::error::ApiError;
use crate::state::AppState;
use application::MessageBroadcast;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use domain::{RoomId, UserId};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

/// WebSocket 连接管理器
///
/// 封装单个 WebSocket 连接的所有状态和逻辑，包括：
/// - 消息接收和发送
/// - 在线状态管理
/// - 心跳机制
/// - 资源清理
pub struct WebSocketConnection {
    socket: Option<WebSocket>,
    state: AppState,
    user_id: UserId,
    room_id: RoomId,
    message_stream: Option<application::MessageStream>,
}

impl WebSocketConnection {
    /// 创建新的 WebSocket 连接
    ///
    /// 这个方法会：
    /// 1. 初始化连接状态
    /// 2. 更新用户在线状态
    /// 3. 设置消息流订阅
    pub async fn new(
        socket: WebSocket,
        state: AppState,
        user_id: Uuid,
        room_id: Uuid,
    ) -> Result<Self, ApiError> {
        let room_id_domain = domain::RoomId::from(room_id);
        let user_id_domain = domain::UserId::from(user_id);

        tracing::info!(user_id = %user_id, room_id = %room_id, "WebSocket 连接已建立");

        // 用户连接到房间 - 更新在线状态
        state
            .presence_manager
            .user_connected(room_id_domain, user_id_domain)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "Failed to update user presence");
                ApiError::internal_server_error("Failed to establish connection")
            })?;

        // 创建消息流 - 直接订阅广播器
        let mut message_stream =
            state
                .broadcaster
                .subscribe(room_id_domain)
                .await
                .map_err(|err| {
                    tracing::error!(error = %err, "Failed to subscribe message stream");
                    ApiError::internal_server_error("Failed to establish connection")
                })?;

        // 清空任何旧消息，只接收连接后发送的新消息
        let mut cleared_count = 0;
        loop {
            match message_stream.try_recv() {
                Ok(Some(_)) => cleared_count += 1,
                Ok(None) => break,
                Err(err) => {
                    tracing::warn!(error = %err, "Failed to drain pending messages");
                    break;
                }
            }
        }
        tracing::info!(cleared_count, "清空了旧消息");

        Ok(Self {
            socket: Some(socket),
            state,
            user_id: user_id_domain,
            room_id: room_id_domain,
            message_stream: Some(message_stream),
        })
    }

    /// 广播统计更新到房间
    pub async fn broadcast_stats_update(state: &AppState, room_id: RoomId) -> Result<(), ApiError> {
        match state.presence_manager.get_online_stats(room_id).await {
            Ok(stats) => {
                let broadcast = MessageBroadcast::stats(room_id, stats);
                if let Err(err) = state.broadcaster.broadcast(broadcast).await {
                    tracing::warn!(error = %err, "Failed to broadcast stats update");
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "Failed to get online stats");
            }
        }
        Ok(())
    }

    /// 运行 WebSocket 连接的主循环
    ///
    /// 这是连接的核心逻辑，处理：
    /// - 客户端消息接收
    /// - 广播消息转发
    /// - 连接生命周期管理
    pub async fn run(mut self) {
        let socket = self.socket.take().expect("Socket should be available");
        let mut message_stream = self
            .message_stream
            .take()
            .expect("Message stream should be available");

        let (mut sender, mut incoming) = socket.split();

        // 广播用户连接的统计更新
        tokio::spawn({
            let state = self.state.clone();
            let room_id = self.room_id;
            async move {
                if let Err(err) = Self::broadcast_stats_update(&state, room_id).await {
                    tracing::warn!(error = ?err, "Failed to broadcast connection stats");
                }
            }
        });

        // 创建 mpsc channel 来解耦对 sender 的访问
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<WsCommand>(32);

        // 发送任务：统一处理所有对 WebSocket sender 的写操作
        let send_task = {
            let cmd_tx_for_broadcast = cmd_tx.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        // 处理来自 mpsc channel 的写命令
                        Some(cmd) = cmd_rx.recv() => {
                            match cmd {
                                WsCommand::SendText(text) => {
                                    if sender.send(WsMessage::Text(text.into())).await.is_err() {
                                        tracing::warn!("Failed to send text message");
                                        break;
                                    }
                                }
                                WsCommand::SendPong(data) => {
                                    if sender.send(WsMessage::Pong(data.into())).await.is_err() {
                                        tracing::warn!("Failed to send pong message");
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
                                tracing::warn!("Failed to send broadcast message to command channel");
                                break;
                            }
                        }
                    }
                }
                tracing::info!("WebSocket发送任务结束");
            })
        };

        // 接收任务：处理来自WebSocket客户端的消息
        let recv_task = tokio::spawn(async move {
            while let Some(Ok(message)) = incoming.next().await {
                if (Self::handle_incoming(message, &cmd_tx).await).is_err() {
                    break;
                }
            }
            tracing::info!("WebSocket接收任务结束");
        });

        // 等待任意一个任务完成（连接断开）
        tokio::select! {
            _ = send_task => {
                tracing::info!("WebSocket发送任务完成");
            }
            _ = recv_task => {
                tracing::info!("WebSocket接收任务完成");
            }
        }

        // 连接断开时清理在线状态
        if let Err(err) = self
            .state
            .presence_manager
            .user_disconnected(self.room_id, self.user_id)
            .await
        {
            tracing::error!(error = %err, user_id = %self.user_id, room_id = %self.room_id, "Failed to cleanup user presence");
        }

        // 广播用户断开的统计更新
        tokio::spawn({
            let state = self.state.clone();
            let room_id = self.room_id;
            async move {
                if let Err(err) = Self::broadcast_stats_update(&state, room_id).await {
                    tracing::warn!(error = ?err, "Failed to broadcast disconnection stats");
                }
            }
        });

        tracing::info!(user_id = %self.user_id, room_id = %self.room_id, "WebSocket连接已断开，在线状态已清理");
    }

    /// 处理来自客户端的消息
    ///
    /// 包括：
    /// - 关闭消息处理
    /// - Ping/Pong 心跳机制
    /// - 其他客户端消息
    async fn handle_incoming(
        message: WsMessage,
        cmd_tx: &mpsc::Sender<WsCommand>,
    ) -> Result<(), ()> {
        match message {
            WsMessage::Close(_) => {
                tracing::info!("WebSocket收到关闭消息");
                return Err(());
            }
            WsMessage::Ping(data) => {
                tracing::debug!("收到ping消息，发送pong回应");
                if cmd_tx
                    .send(WsCommand::SendPong(data.to_vec()))
                    .await
                    .is_err()
                {
                    tracing::warn!("Failed to send pong command");
                    return Err(());
                }
            }
            WsMessage::Pong(_) => {
                tracing::debug!("收到pong消息");
            }
            WsMessage::Text(_) | WsMessage::Binary(_) => {
                // 暂时不处理客户端发送的消息，后续可以添加心跳或其他功能
                tracing::debug!("收到客户端消息");
            }
        }
        Ok(())
    }
}

/// WebSocket 写操作命令
///
/// 使用命令模式统一管理所有对 WebSocket sender 的写操作
#[derive(Debug)]
enum WsCommand {
    SendText(String),
    SendPong(Vec<u8>),
}

impl Drop for WebSocketConnection {
    fn drop(&mut self) {
        tracing::info!(
            user_id = %self.user_id,
            room_id = %self.room_id,
            "WebSocketConnection 被销毁"
        );
    }
}
