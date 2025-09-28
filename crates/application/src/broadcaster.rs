use async_trait::async_trait;
use domain::{Message, RoomId};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio_stream::{Stream, StreamExt};

use crate::presence::OnlineStats;

/// WebSocket消息类型枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WebSocketMessage {
    /// 聊天消息
    #[serde(rename = "chat_message")]
    ChatMessage(Message),
    /// 在线统计更新
    #[serde(rename = "online_stats")]
    OnlineStatsUpdate(OnlineStats),
    /// 系统通知
    #[serde(rename = "system_notification")]
    SystemNotification {
        message: String,
        #[serde(with = "chrono::serde::ts_seconds")]
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// 通用广播消息，可以携带不同类型的WebSocket消息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageBroadcast {
    pub room_id: RoomId,
    pub message: WebSocketMessage,
}

impl MessageBroadcast {
    /// 创建聊天消息广播
    pub fn chat(room_id: RoomId, message: Message) -> Self {
        Self {
            room_id,
            message: WebSocketMessage::ChatMessage(message),
        }
    }

    /// 创建统计更新广播
    pub fn stats(room_id: RoomId, stats: OnlineStats) -> Self {
        Self {
            room_id,
            message: WebSocketMessage::OnlineStatsUpdate(stats),
        }
    }

    /// 创建系统通知广播
    pub fn system_notification(room_id: RoomId, message: String) -> Self {
        Self {
            room_id,
            message: WebSocketMessage::SystemNotification {
                message,
                timestamp: chrono::Utc::now(),
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum BroadcastError {
    #[error("broadcast failed: {0}")]
    Failed(String),
}

impl BroadcastError {
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }
}

#[async_trait]
pub trait MessageBroadcaster: Send + Sync {
    async fn broadcast(&self, payload: MessageBroadcast) -> Result<(), BroadcastError>;
    async fn subscribe(&self, room_id: RoomId) -> Result<MessageStream, BroadcastError>;
}

enum MessageStreamKind {
    Local {
        receiver: broadcast::Receiver<MessageBroadcast>,
        room_id: RoomId,
    },
    Remote {
        stream: Pin<Box<dyn Stream<Item = Result<MessageBroadcast, BroadcastError>> + Send>>,
    },
}

pub struct MessageStream {
    kind: MessageStreamKind,
}

impl MessageStream {
    pub fn local(receiver: broadcast::Receiver<MessageBroadcast>, room_id: RoomId) -> Self {
        Self {
            kind: MessageStreamKind::Local { receiver, room_id },
        }
    }

    pub fn remote(
        stream: Pin<Box<dyn Stream<Item = Result<MessageBroadcast, BroadcastError>> + Send>>,
    ) -> Self {
        Self {
            kind: MessageStreamKind::Remote { stream },
        }
    }

    pub async fn recv(&mut self) -> Option<MessageBroadcast> {
        match &mut self.kind {
            MessageStreamKind::Local { receiver, room_id } => loop {
                match receiver.recv().await {
                    Ok(broadcast) if broadcast.room_id == *room_id => return Some(broadcast),
                    Ok(_) => continue,
                    Err(_) => return None,
                }
            },
            MessageStreamKind::Remote { stream } => loop {
                match stream.next().await {
                    Some(Ok(broadcast)) => return Some(broadcast),
                    Some(Err(err)) => {
                        tracing::warn!(error = %err, "message stream error");
                        continue;
                    }
                    None => return None,
                }
            },
        }
    }

    pub fn try_recv(&mut self) -> Result<Option<MessageBroadcast>, BroadcastError> {
        match &mut self.kind {
            MessageStreamKind::Local { receiver, room_id } => loop {
                match receiver.try_recv() {
                    Ok(broadcast) if broadcast.room_id == *room_id => return Ok(Some(broadcast)),
                    Ok(_) => continue,
                    Err(TryRecvError::Empty | TryRecvError::Closed) => return Ok(None),
                    Err(TryRecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "local broadcast lagged");
                        continue;
                    }
                }
            },
            MessageStreamKind::Remote { .. } => Ok(None),
        }
    }
}
