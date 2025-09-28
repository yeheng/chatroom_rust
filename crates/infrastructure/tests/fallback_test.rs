use application::{
    broadcaster::BroadcastError, MessageBroadcast, MessageBroadcaster, MessageStream,
};
use async_trait::async_trait;
use domain::{Message, MessageContent, MessageId, MessageType, RoomId, UserId};
use infrastructure::{FallbackBroadcaster, LocalMessageBroadcaster};
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

/// 模拟故障的Redis广播器
struct FailingRedisBroadcaster {
    should_fail: std::sync::atomic::AtomicBool,
}

impl FailingRedisBroadcaster {
    fn new() -> Self {
        Self {
            should_fail: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn set_failing(&self, failing: bool) {
        self.should_fail
            .store(failing, std::sync::atomic::Ordering::Relaxed);
    }
}

#[async_trait]
impl MessageBroadcaster for FailingRedisBroadcaster {
    async fn broadcast(&self, _message: MessageBroadcast) -> Result<(), BroadcastError> {
        if self.should_fail.load(std::sync::atomic::Ordering::Relaxed) {
            Err(BroadcastError::failed("Redis connection failed"))
        } else {
            Ok(())
        }
    }

    async fn subscribe(&self, _room_id: RoomId) -> Result<MessageStream, BroadcastError> {
        if self.should_fail.load(std::sync::atomic::Ordering::Relaxed) {
            Err(BroadcastError::failed("Redis connection failed"))
        } else {
            // 返回一个空的流（用于测试）
            let (_, receiver) = tokio::sync::broadcast::channel(16);
            Ok(MessageStream::local(receiver, _room_id))
        }
    }
}

fn create_test_message(room_id: RoomId) -> MessageBroadcast {
    let message = Message {
        id: MessageId::from(Uuid::new_v4()),
        room_id,
        sender_id: UserId::from(Uuid::new_v4()),
        content: MessageContent::new("Test message".to_string()).unwrap(),
        message_type: MessageType::Text,
        reply_to: None,
        created_at: OffsetDateTime::now_utc(),
        last_revision: None,
        is_deleted: false,
    };

    MessageBroadcast::chat(room_id, message)
}

#[tokio::test]
async fn test_redis_fallback() {
    let room_id = RoomId::from(Uuid::new_v4());

    // 创建模拟的Redis广播器和本地广播器
    let redis = Arc::new(FailingRedisBroadcaster::new());
    let local = Arc::new(LocalMessageBroadcaster::new(100));

    let fallback_broadcaster = FallbackBroadcaster::new(
        redis.clone() as Arc<dyn MessageBroadcaster>,
        local.clone() as Arc<dyn MessageBroadcaster>,
    );

    let test_message = create_test_message(room_id);

    // 测试正常情况：Redis工作正常
    let result = fallback_broadcaster.broadcast(test_message.clone()).await;
    assert!(result.is_ok(), "正常情况下广播应该成功");

    // 模拟Redis故障
    redis.set_failing(true);

    // 测试故障情况：应该降级到本地广播
    let result = fallback_broadcaster.broadcast(test_message.clone()).await;
    assert!(result.is_ok(), "Redis故障时应该降级到本地广播并成功");

    // 模拟Redis恢复
    redis.set_failing(false);

    // 再次测试：应该重新使用Redis
    let result = fallback_broadcaster.broadcast(test_message).await;
    assert!(result.is_ok(), "Redis恢复后应该重新工作");
}

#[tokio::test]
async fn test_subscribe_fallback() {
    let room_id = RoomId::from(Uuid::new_v4());

    let redis = Arc::new(FailingRedisBroadcaster::new());
    let local = Arc::new(LocalMessageBroadcaster::new(100));

    let fallback_broadcaster = FallbackBroadcaster::new(
        redis.clone() as Arc<dyn MessageBroadcaster>,
        local.clone() as Arc<dyn MessageBroadcaster>,
    );

    // 测试正常订阅
    let result = fallback_broadcaster.subscribe(room_id).await;
    assert!(result.is_ok(), "正常情况下订阅应该成功");

    // 模拟Redis故障
    redis.set_failing(true);

    // 测试故障情况下的订阅
    let result = fallback_broadcaster.subscribe(room_id).await;
    assert!(result.is_ok(), "Redis故障时订阅应该降级到本地并成功");
}
