use application::{broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster};
use async_trait::async_trait;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct LocalMessageBroadcaster {
    sender: broadcast::Sender<MessageBroadcast>,
}

impl LocalMessageBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MessageBroadcast> {
        self.sender.subscribe()
    }
}

#[async_trait]
impl MessageBroadcaster for LocalMessageBroadcaster {
    async fn broadcast(&self, payload: MessageBroadcast) -> Result<(), BroadcastError> {
        if self.sender.receiver_count() == 0 {
            return Ok(());
        }
        self.sender
            .send(payload)
            .map_err(|err| BroadcastError::failed(err.to_string()))?;
        Ok(())
    }
}
