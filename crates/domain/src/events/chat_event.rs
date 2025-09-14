//! 聊天相关的领域事件
//!
//! 定义聊天系统中的各种业务事件，支持事件驱动架构

use crate::entities::{chatroom::ChatRoom, message::Message};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 聊天相关的领域事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatEvent {
    /// 消息发送事件
    MessageSent { message: Message, room_id: Uuid },

    /// 用户加入房间事件
    UserJoined {
        user_id: Uuid,
        room_id: Uuid,
        username: String,
    },

    /// 用户离开房间事件
    UserLeft { user_id: Uuid, room_id: Uuid },

    /// 房间创建事件
    RoomCreated { room: ChatRoom },

    /// 组织禁止事件
    OrganizationBanned {
        organization_id: Uuid,
        banned_by: Uuid,
        affected_users: Vec<Uuid>,
    },

    /// 用户在线时长更新事件
    UserOnlineTimeUpdated {
        user_id: Uuid,
        session_id: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        duration_seconds: u64,
        disconnect_reason: String,
    },

    /// 用户心跳事件
    UserActivityHeartbeat {
        user_id: Uuid,
        session_id: String,
        timestamp: DateTime<Utc>,
        room_ids: Vec<Uuid>,
        server_instance_id: String,
    },
}

impl ChatEvent {
    /// 创建消息发送事件
    pub fn message_sent(message: Message, room_id: Uuid) -> Self {
        ChatEvent::MessageSent { message, room_id }
    }

    /// 创建用户加入房间事件
    pub fn user_joined(user_id: Uuid, room_id: Uuid, username: String) -> Self {
        ChatEvent::UserJoined {
            user_id,
            room_id,
            username,
        }
    }

    /// 创建用户离开房间事件
    pub fn user_left(user_id: Uuid, room_id: Uuid) -> Self {
        ChatEvent::UserLeft { user_id, room_id }
    }

    /// 创建房间创建事件
    pub fn room_created(room: ChatRoom) -> Self {
        ChatEvent::RoomCreated { room }
    }

    /// 创建组织禁止事件
    pub fn organization_banned(
        organization_id: Uuid,
        banned_by: Uuid,
        affected_users: Vec<Uuid>,
    ) -> Self {
        ChatEvent::OrganizationBanned {
            organization_id,
            banned_by,
            affected_users,
        }
    }

    /// 创建用户在线时长更新事件
    pub fn user_online_time_updated(
        user_id: Uuid,
        session_id: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        duration_seconds: u64,
        disconnect_reason: String,
    ) -> Self {
        ChatEvent::UserOnlineTimeUpdated {
            user_id,
            session_id,
            start_time,
            end_time,
            duration_seconds,
            disconnect_reason,
        }
    }

    /// 创建用户心跳事件
    pub fn user_activity_heartbeat(
        user_id: Uuid,
        session_id: String,
        timestamp: DateTime<Utc>,
        room_ids: Vec<Uuid>,
        server_instance_id: String,
    ) -> Self {
        ChatEvent::UserActivityHeartbeat {
            user_id,
            session_id,
            timestamp,
            room_ids,
            server_instance_id,
        }
    }

    /// 获取事件类型名称
    pub fn event_type(&self) -> &'static str {
        match self {
            ChatEvent::MessageSent { .. } => "MessageSent",
            ChatEvent::UserJoined { .. } => "UserJoined",
            ChatEvent::UserLeft { .. } => "UserLeft",
            ChatEvent::RoomCreated { .. } => "RoomCreated",
            ChatEvent::OrganizationBanned { .. } => "OrganizationBanned",
            ChatEvent::UserOnlineTimeUpdated { .. } => "UserOnlineTimeUpdated",
            ChatEvent::UserActivityHeartbeat { .. } => "UserActivityHeartbeat",
        }
    }

    /// 获取事件中的用户ID（如果有）
    pub fn user_id(&self) -> Option<Uuid> {
        match self {
            ChatEvent::MessageSent { message, .. } => Some(message.sender_id),
            ChatEvent::UserJoined { user_id, .. }
            | ChatEvent::UserLeft { user_id, .. }
            | ChatEvent::UserOnlineTimeUpdated { user_id, .. }
            | ChatEvent::UserActivityHeartbeat { user_id, .. } => Some(*user_id),
            ChatEvent::RoomCreated { room } => Some(room.owner_id),
            ChatEvent::OrganizationBanned { banned_by, .. } => Some(*banned_by),
        }
    }

    /// 获取事件中的房间ID（如果有）
    pub fn room_id(&self) -> Option<Uuid> {
        match self {
            ChatEvent::MessageSent { room_id, .. }
            | ChatEvent::UserJoined { room_id, .. }
            | ChatEvent::UserLeft { room_id, .. } => Some(*room_id),
            ChatEvent::RoomCreated { room } => Some(room.id),
            _ => None,
        }
    }

    /// 获取事件时间戳
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            ChatEvent::MessageSent { message, .. } => message.created_at,
            ChatEvent::RoomCreated { room } => room.created_at,
            ChatEvent::UserOnlineTimeUpdated { end_time, .. } => *end_time,
            ChatEvent::UserActivityHeartbeat { timestamp, .. } => *timestamp,
            _ => Utc::now(), // 对于没有明确时间戳的事件，使用当前时间
        }
    }

    /// 检查事件是否涉及特定用户
    pub fn involves_user(&self, user_id: Uuid) -> bool {
        match self {
            ChatEvent::MessageSent { message, .. } => message.sender_id == user_id,
            ChatEvent::UserJoined { user_id: uid, .. }
            | ChatEvent::UserLeft { user_id: uid, .. }
            | ChatEvent::UserOnlineTimeUpdated { user_id: uid, .. }
            | ChatEvent::UserActivityHeartbeat { user_id: uid, .. } => *uid == user_id,
            ChatEvent::RoomCreated { room } => room.owner_id == user_id,
            ChatEvent::OrganizationBanned {
                banned_by,
                affected_users,
                ..
            } => *banned_by == user_id || affected_users.contains(&user_id),
        }
    }

    /// 检查事件是否涉及特定房间
    pub fn involves_room(&self, room_id: Uuid) -> bool {
        match self {
            ChatEvent::MessageSent { room_id: rid, .. }
            | ChatEvent::UserJoined { room_id: rid, .. }
            | ChatEvent::UserLeft { room_id: rid, .. } => *rid == room_id,
            ChatEvent::RoomCreated { room } => room.id == room_id,
            ChatEvent::UserActivityHeartbeat { room_ids, .. } => room_ids.contains(&room_id),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{
        chatroom::ChatRoom,
        message::Message,
        user::{User, UserStatus},
    };
    use chrono::Duration;

    fn create_test_user(name: &str) -> User {
        User {
            id: Uuid::new_v4(),
            username: name.to_string(),
            email: format!("{}@example.com", name),
            display_name: Some(name.to_string()),
            avatar_url: None,
            status: UserStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_activity_at: Some(Utc::now()),
        }
    }

    fn create_test_room(owner_id: Uuid) -> ChatRoom {
        ChatRoom::new_public(
            "Test Room".to_string(),
            owner_id,
            Some("Test room description".to_string()),
            None,
        )
        .unwrap()
    }

    fn create_test_message(sender_id: Uuid, room_id: Uuid) -> Message {
        Message::new_text(room_id, sender_id, "Hello, world!".to_string()).unwrap()
    }

    #[test]
    fn test_message_sent_event() {
        let user = create_test_user("alice");
        let room = create_test_room(user.id);
        let message = create_test_message(user.id, room.id);

        let event = ChatEvent::message_sent(message.clone(), room.id);

        assert_eq!(event.event_type(), "MessageSent");
        assert_eq!(event.user_id(), Some(user.id));
        assert_eq!(event.room_id(), Some(room.id));
        assert!(event.involves_user(user.id));
        assert!(event.involves_room(room.id));
        assert!(!event.involves_user(Uuid::new_v4()));

        match event {
            ChatEvent::MessageSent {
                message: msg,
                room_id,
            } => {
                assert_eq!(msg.id, message.id);
                assert_eq!(room_id, room.id);
            }
            _ => panic!("Expected MessageSent event"),
        }
    }

    #[test]
    fn test_user_joined_event() {
        let user = create_test_user("bob");
        let room = create_test_room(Uuid::new_v4());

        let event = ChatEvent::user_joined(user.id, room.id, user.username.clone());

        assert_eq!(event.event_type(), "UserJoined");
        assert_eq!(event.user_id(), Some(user.id));
        assert_eq!(event.room_id(), Some(room.id));
        assert!(event.involves_user(user.id));
        assert!(event.involves_room(room.id));

        match event {
            ChatEvent::UserJoined {
                user_id,
                room_id,
                username,
            } => {
                assert_eq!(user_id, user.id);
                assert_eq!(room_id, room.id);
                assert_eq!(username, user.username);
            }
            _ => panic!("Expected UserJoined event"),
        }
    }

    #[test]
    fn test_user_left_event() {
        let user = create_test_user("charlie");
        let room = create_test_room(Uuid::new_v4());

        let event = ChatEvent::user_left(user.id, room.id);

        assert_eq!(event.event_type(), "UserLeft");
        assert_eq!(event.user_id(), Some(user.id));
        assert_eq!(event.room_id(), Some(room.id));
        assert!(event.involves_user(user.id));
        assert!(event.involves_room(room.id));
    }

    #[test]
    fn test_room_created_event() {
        let creator = create_test_user("david");
        let room = create_test_room(creator.id);

        let event = ChatEvent::room_created(room.clone());

        assert_eq!(event.event_type(), "RoomCreated");
        assert_eq!(event.user_id(), Some(creator.id));
        assert_eq!(event.room_id(), Some(room.id));
        assert!(event.involves_user(creator.id));
        assert!(event.involves_room(room.id));

        match event {
            ChatEvent::RoomCreated { room: r } => {
                assert_eq!(r.id, room.id);
                assert_eq!(r.owner_id, creator.id);
            }
            _ => panic!("Expected RoomCreated event"),
        }
    }

    #[test]
    fn test_organization_banned_event() {
        let organization_id = Uuid::new_v4();
        let banned_by = Uuid::new_v4();
        let affected_users = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

        let event =
            ChatEvent::organization_banned(organization_id, banned_by, affected_users.clone());

        assert_eq!(event.event_type(), "OrganizationBanned");
        assert_eq!(event.user_id(), Some(banned_by));
        assert!(event.involves_user(banned_by));

        for &user_id in &affected_users {
            assert!(event.involves_user(user_id));
        }

        match event {
            ChatEvent::OrganizationBanned {
                organization_id: org_id,
                banned_by: banned,
                affected_users: affected,
            } => {
                assert_eq!(org_id, organization_id);
                assert_eq!(banned, banned_by);
                assert_eq!(affected, affected_users);
            }
            _ => panic!("Expected OrganizationBanned event"),
        }
    }

    #[test]
    fn test_user_online_time_updated_event() {
        let user_id = Uuid::new_v4();
        let session_id = "session_123".to_string();
        let start_time = Utc::now() - Duration::hours(2);
        let end_time = Utc::now();
        let duration_seconds = 7200; // 2 hours
        let disconnect_reason = "normal_logout".to_string();

        let event = ChatEvent::user_online_time_updated(
            user_id,
            session_id.clone(),
            start_time,
            end_time,
            duration_seconds,
            disconnect_reason.clone(),
        );

        assert_eq!(event.event_type(), "UserOnlineTimeUpdated");
        assert_eq!(event.user_id(), Some(user_id));
        assert!(event.involves_user(user_id));
        assert_eq!(event.timestamp(), end_time);

        match event {
            ChatEvent::UserOnlineTimeUpdated {
                user_id: uid,
                session_id: sid,
                start_time: st,
                end_time: et,
                duration_seconds: ds,
                disconnect_reason: dr,
            } => {
                assert_eq!(uid, user_id);
                assert_eq!(sid, session_id);
                assert_eq!(st, start_time);
                assert_eq!(et, end_time);
                assert_eq!(ds, duration_seconds);
                assert_eq!(dr, disconnect_reason);
            }
            _ => panic!("Expected UserOnlineTimeUpdated event"),
        }
    }

    #[test]
    fn test_user_activity_heartbeat_event() {
        let user_id = Uuid::new_v4();
        let session_id = "session_456".to_string();
        let timestamp = Utc::now();
        let room_ids = vec![Uuid::new_v4(), Uuid::new_v4()];
        let server_instance_id = "server_01".to_string();

        let event = ChatEvent::user_activity_heartbeat(
            user_id,
            session_id.clone(),
            timestamp,
            room_ids.clone(),
            server_instance_id.clone(),
        );

        assert_eq!(event.event_type(), "UserActivityHeartbeat");
        assert_eq!(event.user_id(), Some(user_id));
        assert!(event.involves_user(user_id));
        assert_eq!(event.timestamp(), timestamp);

        for &room_id in &room_ids {
            assert!(event.involves_room(room_id));
        }

        match event {
            ChatEvent::UserActivityHeartbeat {
                user_id: uid,
                session_id: sid,
                timestamp: ts,
                room_ids: rids,
                server_instance_id: siid,
            } => {
                assert_eq!(uid, user_id);
                assert_eq!(sid, session_id);
                assert_eq!(ts, timestamp);
                assert_eq!(rids, room_ids);
                assert_eq!(siid, server_instance_id);
            }
            _ => panic!("Expected UserActivityHeartbeat event"),
        }
    }

    #[test]
    fn test_event_serialization() {
        let user = create_test_user("eve");
        let room = create_test_room(user.id);
        let message = create_test_message(user.id, room.id);

        let event = ChatEvent::message_sent(message, room.id);

        // 测试序列化
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: ChatEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_type(), event.event_type());
        assert_eq!(deserialized.user_id(), event.user_id());
        assert_eq!(deserialized.room_id(), event.room_id());
    }

    #[test]
    fn test_event_filtering() {
        let user1 = create_test_user("alice");
        let user2 = create_test_user("bob");
        let room1 = create_test_room(user1.id);
        let room2 = create_test_room(user2.id);
        let message = create_test_message(user1.id, room1.id);

        let events = vec![
            ChatEvent::message_sent(message, room1.id),
            ChatEvent::user_joined(user2.id, room1.id, user2.username.clone()),
            ChatEvent::user_left(user1.id, room2.id),
            ChatEvent::room_created(room2),
        ];

        // 测试用户过滤
        let user1_events: Vec<_> = events
            .iter()
            .filter(|e| e.involves_user(user1.id))
            .collect();
        assert_eq!(user1_events.len(), 2);

        // 测试房间过滤
        let room1_events: Vec<_> = events
            .iter()
            .filter(|e| e.involves_room(room1.id))
            .collect();
        assert_eq!(room1_events.len(), 2);
    }
}
