use application::{
    broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster, MessageStream,
};
use async_trait::async_trait;
use domain::RoomId;
use redis::{aio::PubSub, AsyncCommands, Client as RedisClient, Msg};
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt};
use uuid::Uuid;

#[derive(Clone)]
pub struct LocalMessageBroadcaster {
    sender: broadcast::Sender<MessageBroadcast>,
}

impl LocalMessageBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
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

    async fn subscribe(&self, room_id: RoomId) -> Result<MessageStream, BroadcastError> {
        Ok(MessageStream::local(self.sender.subscribe(), room_id))
    }
}

// Redis 消息流，用于 WebSocket 处理
pub struct RedisMessageStream {
    pubsub: PubSub,
    _room_id: domain::RoomId,
}

impl RedisMessageStream {
    pub async fn new(
        redis_client: &RedisClient,
        room_id: domain::RoomId,
    ) -> Result<Self, BroadcastError> {
        let mut pubsub = redis_client
            .get_async_pubsub()
            .await
            .map_err(|err| BroadcastError::failed(format!("Failed to create pubsub: {}", err)))?;

        let channel = RedisMessageBroadcaster::channel_name(room_id);
        pubsub.subscribe(&channel).await.map_err(|err| {
            BroadcastError::failed(format!("Failed to subscribe to {}: {}", channel, err))
        })?;

        tracing::debug!(channel = %channel, "Subscribed to Redis channel");

        Ok(Self {
            pubsub,
            _room_id: room_id,
        })
    }

    // 将 Redis PubSub 转换为异步流
    pub fn into_stream(mut self) -> impl Stream<Item = Result<MessageBroadcast, BroadcastError>> {
        async_stream::stream! {
            let mut pubsub_stream = self.pubsub.on_message();
            loop {
                match pubsub_stream.next().await {
                    Some(msg) => {
                        match Self::parse_message(msg) {
                            Ok(broadcast) => yield Ok(broadcast),
                            Err(err) => {
                                tracing::warn!(error = %err, "Failed to parse Redis message");
                                continue;
                            }
                        }
                    }
                    None => {
                        tracing::warn!("Redis PubSub stream ended");
                        break;
                    }
                }
            }
        }
    }

    fn parse_message(msg: Msg) -> Result<MessageBroadcast, BroadcastError> {
        let payload: String = msg
            .get_payload()
            .map_err(|err| BroadcastError::failed(format!("Failed to get payload: {}", err)))?;

        let broadcast: MessageBroadcast = serde_json::from_str(&payload)
            .map_err(|err| BroadcastError::failed(format!("Failed to deserialize: {}", err)))?;

        Ok(broadcast)
    }
}

// 新的 Redis 广播器实现
#[derive(Clone)]
pub struct RedisMessageBroadcaster {
    client: RedisClient,
}

impl RedisMessageBroadcaster {
    pub fn new(client: RedisClient) -> Self {
        Self { client }
    }

    pub(crate) fn channel_name(room_id: domain::RoomId) -> String {
        format!("chat-room:{}", Uuid::from(room_id))
    }

    // 创建消息流用于特定房间
    pub async fn create_stream(
        &self,
        room_id: domain::RoomId,
    ) -> Result<RedisMessageStream, BroadcastError> {
        RedisMessageStream::new(&self.client, room_id).await
    }
}

#[async_trait]
impl MessageBroadcaster for RedisMessageBroadcaster {
    async fn broadcast(&self, payload: MessageBroadcast) -> Result<(), BroadcastError> {
        let channel = Self::channel_name(payload.room_id);

        // 序列化消息负载
        let serialized_payload = serde_json::to_string(&payload)
            .map_err(|err| BroadcastError::failed(format!("Serialization failed: {}", err)))?;

        // 获取普通连接用于发布
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|err| BroadcastError::failed(format!("Redis connection failed: {}", err)))?;

        // 发布消息到 Redis 频道
        let _: i32 = conn
            .publish(&channel, serialized_payload)
            .await
            .map_err(|err| BroadcastError::failed(format!("Redis publish failed: {}", err)))?;

        tracing::debug!(channel = %channel, "Message broadcasted to Redis");
        Ok(())
    }

    async fn subscribe(&self, room_id: RoomId) -> Result<MessageStream, BroadcastError> {
        let stream = self.create_stream(room_id).await?.into_stream();
        Ok(MessageStream::remote(Box::pin(stream)))
    }
}
