use crate::value_objects::{MessageId, RoomId, Timestamp, UserId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "room_role")]
pub enum RoomRole {
    Owner,
    Admin,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RoomMember {
    pub room_id: RoomId,
    pub user_id: UserId,
    pub role: RoomRole,
    pub joined_at: Timestamp,
    pub last_read_message: Option<MessageId>,
}

impl RoomMember {
    pub fn new(room_id: RoomId, user_id: UserId, role: RoomRole, joined_at: Timestamp) -> Self {
        Self {
            room_id,
            user_id,
            role,
            joined_at,
            last_read_message: None,
        }
    }

    pub fn promote(&mut self, role: RoomRole) {
        self.role = role;
    }

    pub fn record_last_read(&mut self, message_id: MessageId) {
        self.last_read_message = Some(message_id);
    }
}
