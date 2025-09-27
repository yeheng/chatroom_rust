use async_trait::async_trait;
use domain::{Message, RoomId};
use std::pin::Pin;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio_stream::{Stream, StreamExt};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MessageBroadcast {
    pub room_id: RoomId,
    pub message: Message,
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
