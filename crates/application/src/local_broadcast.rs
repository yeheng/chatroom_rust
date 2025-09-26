// 简单的本地广播器实现
use crate::{broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster};
use async_trait::async_trait;
use domain::RoomId;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct LocalMessageBroadcaster {
    sender: broadcast::Sender<MessageBroadcast>,
}

impl LocalMessageBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MessageBroadcast> {
        self.sender.subscribe()
    }
}

impl Default for LocalMessageBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageBroadcaster for LocalMessageBroadcaster {
    async fn broadcast(&self, payload: MessageBroadcast) -> Result<(), BroadcastError> {
        self.sender
            .send(payload)
            .map_err(|err| BroadcastError::failed(err.to_string()))?;
        Ok(())
    }
}

// 简单的消息流
pub struct MessageStream {
    receiver: broadcast::Receiver<MessageBroadcast>,
    room_id: RoomId,
}

impl MessageStream {
    pub fn new(receiver: broadcast::Receiver<MessageBroadcast>, room_id: RoomId) -> Self {
        Self { receiver, room_id }
    }

    pub async fn recv(&mut self) -> Option<MessageBroadcast> {
        loop {
            match self.receiver.recv().await {
                Ok(broadcast) => {
                    // 过滤只属于当前房间的消息
                    if broadcast.room_id == self.room_id {
                        return Some(broadcast);
                    }
                }
                Err(_) => return None,
            }
        }
    }
}