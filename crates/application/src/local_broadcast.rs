// 简单的本地广播器实现
use crate::{broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster};
use async_trait::async_trait;
use domain::RoomId;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;

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
        // 如果没有接收者，send 会返回错误，但在测试环境中这是正常的
        // 我们检查是否有接收者，如果没有就直接返回成功
        let receiver_count = self.sender.receiver_count();
        if receiver_count == 0 {
            // 没有接收者是正常情况，不算错误
            return Ok(());
        }

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

    // 非阻塞接收方法，用于清空旧消息
    pub fn try_recv(&mut self) -> Result<Option<MessageBroadcast>, TryRecvError> {
        loop {
            match self.receiver.try_recv() {
                Ok(broadcast) => {
                    // 过滤只属于当前房间的消息
                    if broadcast.room_id == self.room_id {
                        return Ok(Some(broadcast));
                    }
                    // 如果不是当前房间的消息，继续尝试下一条
                    continue;
                }
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Closed) => return Ok(None),
                Err(e) => return Err(e),
            }
        }
    }
}