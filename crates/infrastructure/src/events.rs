//! 聊天事件定义
//!
//! 定义系统中所有的聊天事件类型，用于 Kafka 消息队列和 Redis 发布订阅。

use chrono::{DateTime, Utc};
use domain::{chatroom::ChatRoom, message::Message};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 聊天事件枚举
///
/// 简化的事件结构，移除服务器实例绑定，降低系统耦合度
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChatEvent {
    /// 消息发送事件
    MessageSent {
        message: Message,
        room_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 消息编辑事件
    MessageEdited {
        message_id: Uuid,
        room_id: Uuid,
        new_content: String,
        edited_by: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 消息删除事件
    MessageDeleted {
        message_id: Uuid,
        room_id: Uuid,
        deleted_by: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 消息撤回事件
    MessageRecalled {
        message_id: Uuid,
        room_id: Uuid,
        recalled_by: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 用户加入房间事件
    UserJoinedRoom {
        user_id: Uuid,
        room_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 用户离开房间事件
    UserLeftRoom {
        user_id: Uuid,
        room_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 用户在线状态变更事件
    UserStatusChanged {
        user_id: Uuid,
        old_status: String,
        new_status: String,
        timestamp: DateTime<Utc>,
    },

    /// 房间创建事件
    RoomCreated {
        room: ChatRoom,
        created_by: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 房间更新事件
    RoomUpdated {
        room_id: Uuid,
        updated_by: Uuid,
        changes: RoomChanges,
        timestamp: DateTime<Utc>,
    },

    /// 房间删除事件
    RoomDeleted {
        room_id: Uuid,
        deleted_by: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 用户正在输入事件（仅用于 Redis，不存储到 Kafka）
    UserTyping {
        user_id: Uuid,
        room_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 用户停止输入事件（仅用于 Redis，不存储到 Kafka）
    UserStoppedTyping {
        user_id: Uuid,
        room_id: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 成员角色变更事件
    MemberRoleChanged {
        room_id: Uuid,
        user_id: Uuid,
        new_role: String,
        changed_by: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 成员被踢出事件
    MemberKicked {
        room_id: Uuid,
        user_id: Uuid,
        kicked_by: Uuid,
        timestamp: DateTime<Utc>,
    },

    /// 用户注册事件
    UserRegistered {
        user_id: Uuid,
        username: String,
        email: String,
        timestamp: DateTime<Utc>,
    },

    /// 用户创建事件
    UserCreated {
        user_id: Uuid,
        username: String,
        email: String,
        timestamp: DateTime<Utc>,
    },

    /// 用户更新事件
    UserUpdated {
        user_id: Uuid,
        nickname: String,
        timestamp: DateTime<Utc>,
    },

    /// 用户删除事件
    UserDeleted {
        user_id: Uuid,
        deleted_by: Uuid,
        timestamp: DateTime<Utc>,
    },
}

/// 房间变更详情
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomChanges {
    /// 名称变更
    pub name: Option<String>,
    /// 描述变更
    pub description: Option<Option<String>>,
    /// 最大成员数变更
    pub max_members: Option<Option<u32>>,
    /// 密码变更（不包含实际密码，只是变更标记）
    pub password_changed: bool,
}

impl ChatEvent {
    /// 获取事件的房间ID（用于分区键）
    pub fn room_id(&self) -> Option<Uuid> {
        match self {
            ChatEvent::MessageSent { room_id, .. } => Some(*room_id),
            ChatEvent::MessageEdited { room_id, .. } => Some(*room_id),
            ChatEvent::MessageDeleted { room_id, .. } => Some(*room_id),
            ChatEvent::MessageRecalled { room_id, .. } => Some(*room_id),
            ChatEvent::UserJoinedRoom { room_id, .. } => Some(*room_id),
            ChatEvent::UserLeftRoom { room_id, .. } => Some(*room_id),
            ChatEvent::UserStatusChanged { .. } => None, // 全局事件
            ChatEvent::RoomCreated { room, .. } => Some(room.id),
            ChatEvent::RoomUpdated { room_id, .. } => Some(*room_id),
            ChatEvent::RoomDeleted { room_id, .. } => Some(*room_id),
            ChatEvent::UserTyping { room_id, .. } => Some(*room_id),
            ChatEvent::UserStoppedTyping { room_id, .. } => Some(*room_id),
            ChatEvent::MemberRoleChanged { room_id, .. } => Some(*room_id),
            ChatEvent::MemberKicked { room_id, .. } => Some(*room_id),
            ChatEvent::UserRegistered { .. } => None, // 全局事件
            ChatEvent::UserCreated { .. } => None,    // 全局事件
            ChatEvent::UserUpdated { .. } => None,    // 全局事件
            ChatEvent::UserDeleted { .. } => None,    // 全局事件
        }
    }

    /// 获取事件的时间戳
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            ChatEvent::MessageSent { timestamp, .. } => *timestamp,
            ChatEvent::MessageEdited { timestamp, .. } => *timestamp,
            ChatEvent::MessageDeleted { timestamp, .. } => *timestamp,
            ChatEvent::MessageRecalled { timestamp, .. } => *timestamp,
            ChatEvent::UserJoinedRoom { timestamp, .. } => *timestamp,
            ChatEvent::UserLeftRoom { timestamp, .. } => *timestamp,
            ChatEvent::UserStatusChanged { timestamp, .. } => *timestamp,
            ChatEvent::RoomCreated { timestamp, .. } => *timestamp,
            ChatEvent::RoomUpdated { timestamp, .. } => *timestamp,
            ChatEvent::RoomDeleted { timestamp, .. } => *timestamp,
            ChatEvent::UserTyping { timestamp, .. } => *timestamp,
            ChatEvent::UserStoppedTyping { timestamp, .. } => *timestamp,
            ChatEvent::MemberRoleChanged { timestamp, .. } => *timestamp,
            ChatEvent::MemberKicked { timestamp, .. } => *timestamp,
            ChatEvent::UserRegistered { timestamp, .. } => *timestamp,
            ChatEvent::UserCreated { timestamp, .. } => *timestamp,
            ChatEvent::UserUpdated { timestamp, .. } => *timestamp,
            ChatEvent::UserDeleted { timestamp, .. } => *timestamp,
        }
    }

    /// 检查事件是否应该存储到 Kafka（持久化事件）
    pub fn should_persist(&self) -> bool {
        match self {
            ChatEvent::UserTyping { .. } | ChatEvent::UserStoppedTyping { .. } => false,
            _ => true,
        }
    }

    /// 检查事件是否应该通过 Redis 实时广播
    pub fn should_broadcast(&self) -> bool {
        true // 所有事件都应该实时广播
    }

    /// 获取事件类型名称（用于日志和监控）
    pub fn event_type(&self) -> &'static str {
        match self {
            ChatEvent::MessageSent { .. } => "message_sent",
            ChatEvent::MessageEdited { .. } => "message_edited",
            ChatEvent::MessageDeleted { .. } => "message_deleted",
            ChatEvent::MessageRecalled { .. } => "message_recalled",
            ChatEvent::UserJoinedRoom { .. } => "user_joined_room",
            ChatEvent::UserLeftRoom { .. } => "user_left_room",
            ChatEvent::UserStatusChanged { .. } => "user_status_changed",
            ChatEvent::RoomCreated { .. } => "room_created",
            ChatEvent::RoomUpdated { .. } => "room_updated",
            ChatEvent::RoomDeleted { .. } => "room_deleted",
            ChatEvent::UserTyping { .. } => "user_typing",
            ChatEvent::UserStoppedTyping { .. } => "user_stopped_typing",
            ChatEvent::MemberRoleChanged { .. } => "member_role_changed",
            ChatEvent::MemberKicked { .. } => "member_kicked",
            ChatEvent::UserRegistered { .. } => "user_registered",
            ChatEvent::UserCreated { .. } => "user_created",
            ChatEvent::UserUpdated { .. } => "user_updated",
            ChatEvent::UserDeleted { .. } => "user_deleted",
        }
    }
}

/// Redis 消息类型（用于 Pub/Sub）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RedisMessageType {
    /// 聊天事件
    ChatEvent(ChatEvent),
    /// 系统通知
    SystemNotification {
        message: String,
        level: NotificationLevel,
        timestamp: DateTime<Utc>,
    },
    /// 房间统计更新
    RoomStatsUpdate {
        room_id: Uuid,
        online_count: u32,
        total_messages: u64,
        timestamp: DateTime<Utc>,
    },
}

/// 通知级别
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

/// Redis 房间消息（用于房间频道）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedisRoomMessage {
    /// 房间ID
    pub room_id: Uuid,
    /// 消息类型
    pub message_type: RedisMessageType,
    /// 消息ID（用于去重）
    pub message_id: Uuid,
    /// 发送时间
    pub sent_at: DateTime<Utc>,
}

impl RedisRoomMessage {
    /// 创建新的房间消息
    pub fn new(room_id: Uuid, message_type: RedisMessageType) -> Self {
        Self {
            room_id,
            message_type,
            message_id: Uuid::new_v4(),
            sent_at: Utc::now(),
        }
    }

    /// 从聊天事件创建房间消息
    pub fn from_chat_event(event: ChatEvent) -> Option<Self> {
        event
            .room_id()
            .map(|room_id| Self::new(room_id, RedisMessageType::ChatEvent(event)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_event_serialization() {
        let message =
            Message::new_text(Uuid::new_v4(), Uuid::new_v4(), "Hello World".to_string()).unwrap();

        let event = ChatEvent::MessageSent {
            message: message.clone(),
            room_id: Uuid::new_v4(),
            timestamp: Utc::now(),
        };

        // 测试序列化
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: ChatEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_chat_event_room_id_extraction() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let message_event = ChatEvent::MessageSent {
            message: Message::new_text(room_id, user_id, "Test".to_string()).unwrap(),
            room_id,
            timestamp: Utc::now(),
        };

        assert_eq!(message_event.room_id(), Some(room_id));

        let user_status_event = ChatEvent::UserStatusChanged {
            user_id,
            old_status: "Active".to_string(),
            new_status: "Offline".to_string(),
            timestamp: Utc::now(),
        };

        assert_eq!(user_status_event.room_id(), None);
    }

    #[test]
    fn test_event_persistence_rules() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let typing_event = ChatEvent::UserTyping {
            user_id,
            room_id,
            timestamp: Utc::now(),
        };

        assert!(!typing_event.should_persist());
        assert!(typing_event.should_broadcast());

        let message_event = ChatEvent::MessageSent {
            message: Message::new_text(room_id, user_id, "Test".to_string()).unwrap(),
            room_id,
            timestamp: Utc::now(),
        };

        assert!(message_event.should_persist());
        assert!(message_event.should_broadcast());
    }

    #[test]
    fn test_redis_room_message_creation() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let event = ChatEvent::UserJoinedRoom {
            user_id,
            room_id,
            timestamp: Utc::now(),
        };

        let room_message = RedisRoomMessage::from_chat_event(event.clone()).unwrap();

        assert_eq!(room_message.room_id, room_id);

        if let RedisMessageType::ChatEvent(chat_event) = room_message.message_type {
            assert_eq!(chat_event, event);
        } else {
            panic!("Expected ChatEvent");
        }
    }

    #[test]
    fn test_room_changes_serialization() {
        let changes = RoomChanges {
            name: Some("New Room Name".to_string()),
            description: Some(Some("New description".to_string())),
            max_members: Some(Some(100)),
            password_changed: true,
        };

        let json = serde_json::to_string(&changes).unwrap();
        let deserialized: RoomChanges = serde_json::from_str(&json).unwrap();

        assert_eq!(changes, deserialized);
    }
}
