use domain::{MessageId, RoomId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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

/// 消息序列化器和去重器
/// 确保消息按顺序传递且无重复
pub struct MessageSequencer {
    /// 每个房间的当前序列号
    room_sequences: Arc<RwLock<HashMap<RoomId, u64>>>,
    /// 已处理的消息ID（用于去重）
    processed_messages: Arc<RwLock<HashMap<MessageId, u64>>>,
}

impl MessageSequencer {
    pub fn new() -> Self {
        Self {
            room_sequences: Arc::new(RwLock::new(HashMap::new())),
            processed_messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 为消息分配序列号
    /// 如果消息已存在，返回原序列号；否则分配新序列号
    pub fn assign_sequence(&self, room_id: RoomId, message_id: MessageId) -> Result<SequencedMessage, String> {
        // 检查是否已处理过此消息（去重）
        {
            let processed = self.processed_messages.read().map_err(|e| format!("RwLock poisoned: {}", e))?;
            if let Some(&existing_seq) = processed.get(&message_id) {
                return Ok(SequencedMessage {
                    sequence_id: existing_seq,
                    room_id,
                    message_id,
                    timestamp: OffsetDateTime::now_utc(),
                });
            }
        }

        // 为消息分配新的序列号
        let sequence_id = {
            let mut sequences = self.room_sequences.write().map_err(|e| format!("RwLock poisoned: {}", e))?;
            let seq = sequences.entry(room_id).or_insert(0);
            *seq += 1;
            *seq
        };

        // 记录已处理的消息
        {
            let mut processed = self.processed_messages.write().map_err(|e| format!("RwLock poisoned: {}", e))?;
            processed.insert(message_id, sequence_id);
        }

        Ok(SequencedMessage {
            sequence_id,
            room_id,
            message_id,
            timestamp: OffsetDateTime::now_utc(),
        })
    }

    /// 检查消息是否已处理（去重检查）
    pub fn is_duplicate(&self, message_id: MessageId) -> Result<bool, String> {
        let processed = self.processed_messages.read().map_err(|e| format!("RwLock poisoned: {}", e))?;
        Ok(processed.contains_key(&message_id))
    }

    /// 获取房间的当前序列号
    pub fn get_room_sequence(&self, room_id: RoomId) -> Result<u64, String> {
        let sequences = self.room_sequences.read().map_err(|e| format!("RwLock poisoned: {}", e))?;
        Ok(sequences.get(&room_id).copied().unwrap_or(0))
    }

    /// 清理旧的已处理消息记录（防止内存泄漏）
    pub fn cleanup_old_messages(&self, max_entries: usize) -> Result<usize, String> {
        let mut processed = self.processed_messages.write().map_err(|e| format!("RwLock poisoned: {}", e))?;

        if processed.len() <= max_entries {
            return Ok(0);
        }

        let excess = processed.len() - max_entries;

        // 简单的清理策略：随机删除一些旧记录
        // 实际生产中可能需要基于时间戳的更智能策略
        let keys_to_remove: Vec<MessageId> = processed.keys().take(excess).cloned().collect();

        for key in &keys_to_remove {
            processed.remove(key);
        }

        Ok(keys_to_remove.len())
    }
}

impl Default for MessageSequencer {
    fn default() -> Self {
        Self::new()
    }
}