use crate::value_objects::{MessageId, RoomId, Timestamp, UserId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "room_role")]
#[sqlx(rename_all = "lowercase")]
pub enum RoomRole {
    #[sqlx(rename = "owner")]
    Owner,
    #[sqlx(rename = "admin")]
    Admin,
    #[sqlx(rename = "member")]
    Member,
}

impl RoomRole {
    /// 检查角色是否有管理员权限（可以访问房间统计信息）
    pub fn has_admin_access(&self) -> bool {
        matches!(self, RoomRole::Owner | RoomRole::Admin)
    }

    /// 检查角色是否为房间所有者
    pub fn is_owner(&self) -> bool {
        matches!(self, RoomRole::Owner)
    }

    /// 检查角色是否可以管理房间成员
    pub fn can_manage_members(&self) -> bool {
        matches!(self, RoomRole::Owner | RoomRole::Admin)
    }

    /// 检查角色是否可以删除消息
    pub fn can_delete_messages(&self) -> bool {
        matches!(self, RoomRole::Owner | RoomRole::Admin)
    }
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
