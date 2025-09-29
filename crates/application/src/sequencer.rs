use domain::{MessageId, RoomId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use time::OffsetDateTime;

/// 带序列号的消息
/// 每个房间维护递增序列号，确保消息有序且不重复
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencedMessage {
    pub sequence_id: u64,
    pub room_id: RoomId,
    pub message_id: MessageId,
    pub timestamp: OffsetDateTime,
}

/// Redis-based消息序列化器和去重器
/// 使用Redis原子操作实现分布式序列号分配和消息去重
pub struct MessageSequencer {
    /// Redis客户端
    redis_client: Arc<redis::Client>,
}

impl MessageSequencer {
    /// 创建新的Redis-based序列化器
    pub fn new(redis_client: Arc<redis::Client>) -> Self {
        Self { redis_client }
    }

    /// 获取Redis连接
    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, String> {
        self.redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| format!("Redis connection failed: {}", e))
    }

    /// 生成房间序列号键
    fn room_sequence_key(&self, room_id: RoomId) -> String {
        format!("room_sequence:{}", room_id)
    }

    /// 生成已处理消息键
    fn processed_messages_key(&self) -> String {
        "processed_messages".to_string()
    }

    /// 为消息分配序列号
    /// 使用Redis原子操作实现分布式序列号分配和消息去重
    pub async fn assign_sequence(
        &self,
        room_id: RoomId,
        message_id: MessageId,
    ) -> Result<SequencedMessage, String> {
        let mut conn = self.get_connection().await?;
        let room_key = self.room_sequence_key(room_id);
        let processed_key = self.processed_messages_key();

        // 使用Lua脚本确保原子性：去重 + 序列号分配
        let script = redis::Script::new(
            r#"
            local room_key = KEYS[1]
            local processed_key = KEYS[2]
            local message_id = ARGV[1]

            -- 检查消息是否已处理（去重）
            local existing_seq = redis.call('HGET', processed_key, message_id)
            if existing_seq then
                return {0, tonumber(existing_seq)}
            end

            -- 分配新序列号
            local sequence_id = redis.call('INCR', room_key)

            -- 记录已处理的消息，设置24小时过期
            redis.call('HSET', processed_key, message_id, sequence_id)
            redis.call('EXPIRE', processed_key, 86400)

            return {1, sequence_id}
            "#,
        );

        let result: Vec<i64> = script
            .key(&room_key)
            .key(&processed_key)
            .arg(message_id.to_string())
            .invoke_async(&mut conn)
            .await
            .map_err(|e| format!("Redis script execution failed: {}", e))?;

        let _is_duplicate = result[0] == 0;
        let sequence_id = result[1] as u64;

        Ok(SequencedMessage {
            sequence_id,
            room_id,
            message_id,
            timestamp: OffsetDateTime::now_utc(),
        })
    }

    /// 检查消息是否已处理（去重检查）
    pub async fn is_duplicate(&self, message_id: MessageId) -> Result<bool, String> {
        let mut conn = self.get_connection().await?;
        let processed_key = self.processed_messages_key();

        let exists: bool = redis::cmd("HEXISTS")
            .arg(&processed_key)
            .arg(message_id.to_string())
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("Redis HEXISTS failed: {}", e))?;

        Ok(exists)
    }

    /// 获取房间的当前序列号
    pub async fn get_room_sequence(&self, room_id: RoomId) -> Result<u64, String> {
        let mut conn = self.get_connection().await?;
        let room_key = self.room_sequence_key(room_id);

        let sequence: i64 = redis::cmd("GET")
            .arg(&room_key)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        Ok(sequence as u64)
    }
}

// 移除Default实现，因为需要Redis客户端
// 使用者必须显式提供Redis客户端
