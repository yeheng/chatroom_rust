//! WebSocket测试客户端
//! 
//! 提供WebSocket连接测试工具，用于模拟客户端与服务器的实时通信

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use tracing::{debug, error, info};
use uuid::Uuid;

use domain::{MessageType, websocket::*};
use crate::{TestEnvironment, TestUser};

/// WebSocket测试客户端
/// 
/// 封装WebSocket连接和消息处理逻辑
pub struct WebSocketTestClient {
    user_id: Uuid,
    ws_stream: Arc<Mutex<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>>,
    message_receiver: mpsc::UnboundedReceiver<WebSocketFrame>,
    message_sender: mpsc::UnboundedSender<WebSocketFrame>,
    _task_handle: tokio::task::JoinHandle<()>,
}

impl WebSocketTestClient {
    /// 创建并连接WebSocket测试客户端
    pub async fn connect(
        env: &TestEnvironment, 
        user: &TestUser, 
        token: &str
    ) -> Result<Self> {
        let ws_url = format!("{}/ws?token={}", 
            env.app_base_url().replace("http", "ws"), 
            token
        );
        
        info!("连接WebSocket: {}", ws_url);
        
        // 建立WebSocket连接
        let (ws_stream, _response) = connect_async(&ws_url)
            .await
            .context("WebSocket连接失败")?;
        
        let ws_stream = Arc::new(Mutex::new(ws_stream));
        
        // 创建消息通道
        let (tx, rx) = mpsc::unbounded_channel();
        let (internal_tx, mut internal_rx) = mpsc::unbounded_channel::<WebSocketFrame>();
        
        // 启动消息处理任务
        let ws_stream_clone = ws_stream.clone();
        let task_handle = tokio::spawn(async move {
            let mut ws = ws_stream_clone.lock().await;
            
            loop {
                tokio::select! {
                    // 接收来自服务器的消息
                    msg = ws.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                match serde_json::from_str::<WebSocketFrame>(&text) {
                                    Ok(frame) => {
                                        debug!("收到服务器消息: {:?}", frame);
                                        if let Err(e) = tx.send(frame) {
                                            error!("发送消息到接收器失败: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        error!("解析WebSocket消息失败: {}", e);
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) => {
                                info!("WebSocket连接关闭");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("WebSocket错误: {}", e);
                                break;
                            }
                            None => {
                                info!("WebSocket流结束");
                                break;
                            }
                            _ => {
                                // 忽略其他类型的消息
                            }
                        }
                    }
                    
                    // 发送消息到服务器
                    Some(frame) = internal_rx.recv() => {
                        match serde_json::to_string(&frame) {
                            Ok(json) => {
                                if let Err(e) = ws.send(Message::Text(json)).await {
                                    error!("发送WebSocket消息失败: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("序列化WebSocket消息失败: {}", e);
                            }
                        }
                    }
                }
            }
        });
        
        Ok(Self {
            user_id: user.id,
            ws_stream,
            message_receiver: rx,
            message_sender: internal_tx,
            _task_handle: task_handle,
        })
    }

    /// 发送消息到WebSocket
    pub async fn send_message(&self, frame: WebSocketFrame) -> Result<()> {
        self.message_sender.send(frame)
            .context("发送消息到WebSocket失败")?;
        Ok(())
    }

    /// 发送聊天消息
    pub async fn send_chat_message(
        &self, 
        room_id: Uuid, 
        content: &str, 
        message_type: MessageType
    ) -> Result<()> {
        let frame = WebSocketFrame {
            message_id: format!("msg_{}", Uuid::new_v4()),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::ClientToServer(ClientMessage::SendMessage {
                room_id: room_id.to_string(),
                content: content.to_string(),
                message_type,
            }),
            payload: serde_json::Value::Null,
        };

        self.send_message(frame).await
    }

    /// 加入聊天室
    pub async fn join_room(&self, room_id: Uuid) -> Result<()> {
        let frame = WebSocketFrame {
            message_id: format!("join_{}", Uuid::new_v4()),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::ClientToServer(ClientMessage::JoinRoom {
                room_id: room_id.to_string(),
            }),
            payload: serde_json::Value::Null,
        };

        self.send_message(frame).await
    }

    /// 离开聊天室
    pub async fn leave_room(&self, room_id: Uuid) -> Result<()> {
        let frame = WebSocketFrame {
            message_id: format!("leave_{}", Uuid::new_v4()),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::ClientToServer(ClientMessage::LeaveRoom {
                room_id: room_id.to_string(),
            }),
            payload: serde_json::Value::Null,
        };

        self.send_message(frame).await
    }

    /// 发送心跳
    pub async fn send_heartbeat(&self) -> Result<()> {
        let frame = WebSocketFrame {
            message_id: format!("heartbeat_{}", Uuid::new_v4()),
            timestamp: chrono::Utc::now(),
            message_type: MessageType::ClientToServer(ClientMessage::Heartbeat),
            payload: serde_json::Value::Null,
        };

        self.send_message(frame).await
    }

    /// 接收下一条消息（带超时）
    pub async fn receive_message_timeout(
        &mut self, 
        timeout: std::time::Duration
    ) -> Result<Option<WebSocketFrame>> {
        match tokio::time::timeout(timeout, self.message_receiver.recv()).await {
            Ok(Some(frame)) => Ok(Some(frame)),
            Ok(None) => Ok(None),
            Err(_) => Ok(None), // 超时
        }
    }

    /// 接收下一条消息
    pub async fn receive_message(&mut self) -> Result<Option<WebSocketFrame>> {
        Ok(self.message_receiver.recv().await)
    }

    /// 接收指定数量的消息
    pub async fn receive_messages(&mut self, count: usize) -> Result<Vec<WebSocketFrame>> {
        let mut messages = Vec::with_capacity(count);
        
        for _ in 0..count {
            match self.receive_message_timeout(std::time::Duration::from_secs(5)).await? {
                Some(msg) => messages.push(msg),
                None => break,
            }
        }
        
        Ok(messages)
    }

    /// 等待特定类型的消息
    pub async fn wait_for_message_type(
        &mut self,
        expected_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<Option<WebSocketFrame>> {
        let start_time = tokio::time::Instant::now();
        
        while start_time.elapsed() < timeout {
            match self.receive_message_timeout(std::time::Duration::from_millis(100)).await? {
                Some(frame) => {
                    if std::mem::discriminant(&frame.message_type) == std::mem::discriminant(&expected_type) {
                        return Ok(Some(frame));
                    }
                    // 收到其他类型的消息，继续等待
                }
                None => {
                    // 超时或无消息，继续等待
                }
            }
        }
        
        Ok(None)
    }

    /// 清空消息接收缓冲区
    pub async fn clear_message_buffer(&mut self) {
        while self.message_receiver.try_recv().is_ok() {
            // 清空缓冲区
        }
    }

    /// 获取用户ID
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    /// 检查连接是否活跃
    pub async fn is_connected(&self) -> bool {
        // 发送心跳检测连接状态
        self.send_heartbeat().await.is_ok()
    }
}

/// WebSocket消息匹配器
/// 
/// 提供便捷的消息匹配和断言功能
pub struct MessageMatcher;

impl MessageMatcher {
    /// 检查是否为新消息
    pub fn is_new_message(frame: &WebSocketFrame) -> bool {
        matches!(frame.message_type, MessageType::ServerToClient(ServerMessage::NewMessage { .. }))
    }

    /// 检查是否为用户加入消息
    pub fn is_user_joined(frame: &WebSocketFrame) -> bool {
        matches!(frame.message_type, MessageType::ServerToClient(ServerMessage::UserJoined { .. }))
    }

    /// 检查是否为用户离开消息
    pub fn is_user_left(frame: &WebSocketFrame) -> bool {
        matches!(frame.message_type, MessageType::ServerToClient(ServerMessage::UserLeft { .. }))
    }

    /// 检查是否为错误消息
    pub fn is_error(frame: &WebSocketFrame) -> bool {
        matches!(frame.message_type, MessageType::ServerToClient(ServerMessage::Error { .. }))
    }

    /// 提取新消息内容
    pub fn extract_message_content(frame: &WebSocketFrame) -> Option<String> {
        match &frame.message_type {
            MessageType::ServerToClient(ServerMessage::NewMessage { message }) => {
                Some(message.content.clone())
            }
            _ => None,
        }
    }

    /// 提取错误信息
    pub fn extract_error_message(frame: &WebSocketFrame) -> Option<String> {
        match &frame.message_type {
            MessageType::ServerToClient(ServerMessage::Error { error_code, message }) => {
                Some(format!("{}: {}", error_code, message))
            }
            _ => None,
        }
    }
}

/// WebSocket测试工具集
pub struct WebSocketTestUtils;

impl WebSocketTestUtils {
    /// 创建多个WebSocket连接
    pub async fn create_multiple_connections(
        env: &TestEnvironment,
        users_and_tokens: &[(TestUser, String)],
    ) -> Result<Vec<WebSocketTestClient>> {
        let mut clients = Vec::new();
        
        for (user, token) in users_and_tokens {
            let client = WebSocketTestClient::connect(env, user, token).await?;
            clients.push(client);
        }
        
        Ok(clients)
    }

    /// 并发发送消息
    pub async fn send_concurrent_messages(
        clients: &[WebSocketTestClient],
        room_id: Uuid,
        message_count: usize,
    ) -> Result<()> {
        let tasks: Vec<_> = clients
            .iter()
            .enumerate()
            .map(|(i, client)| {
                let room_id = room_id;
                async move {
                    for j in 0..message_count {
                        let content = format!("并发消息 {} 来自客户端 {}", j + 1, i + 1);
                        client
                            .send_chat_message(room_id, &content, MessageType::Text)
                            .await?;
                    }
                    Result::<()>::Ok(())
                }
            })
            .collect();

        futures::future::try_join_all(tasks).await?;
        Ok(())
    }

    /// 等待所有客户端收到指定数量的消息
    pub async fn wait_for_all_messages(
        clients: &mut [WebSocketTestClient],
        expected_count: usize,
        timeout: std::time::Duration,
    ) -> Result<Vec<Vec<WebSocketFrame>>> {
        let tasks: Vec<_> = clients
            .iter_mut()
            .map(|client| async move {
                client.receive_messages(expected_count).await
            })
            .collect();

        let results = futures::future::try_join_all(tasks).await?;
        Ok(results)
    }
}
