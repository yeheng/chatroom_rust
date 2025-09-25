use async_trait::async_trait;
use domain::{Message, RoomId};
use thiserror::Error;

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
}
