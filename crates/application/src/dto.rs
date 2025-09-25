use domain::{ChatRoom, ChatRoomVisibility, Message, MessageType, RoomMember, RoomRole, Timestamp, User, UserStatus};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub status: UserStatus,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl From<&User> for UserDto {
    fn from(user: &User) -> Self {
        Self {
            id: Uuid::from(user.id),
            username: user.username.as_str().to_owned(),
            email: user.email.as_str().to_owned(),
            status: user.status.clone(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomDto {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
    pub visibility: ChatRoomVisibility,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub is_closed: bool,
}

impl From<&ChatRoom> for RoomDto {
    fn from(room: &ChatRoom) -> Self {
        Self {
            id: Uuid::from(room.id),
            name: room.name.clone(),
            owner_id: Uuid::from(room.owner_id),
            visibility: room.visibility.clone(),
            created_at: room.created_at,
            updated_at: room.updated_at,
            is_closed: room.is_closed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMemberDto {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub role: RoomRole,
    pub joined_at: Timestamp,
    pub last_read_message: Option<Uuid>,
}

impl From<&RoomMember> for RoomMemberDto {
    fn from(member: &RoomMember) -> Self {
        Self {
            room_id: Uuid::from(member.room_id),
            user_id: Uuid::from(member.user_id),
            role: member.role.clone(),
            joined_at: member.joined_at,
            last_read_message: member.last_read_message.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDto {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to: Option<Uuid>,
    pub created_at: Timestamp,
    pub last_revision_at: Option<Timestamp>,
    pub is_deleted: bool,
}

impl From<&Message> for MessageDto {
    fn from(message: &Message) -> Self {
        Self {
            id: Uuid::from(message.id),
            room_id: Uuid::from(message.room_id),
            sender_id: Uuid::from(message.sender_id),
            content: message.content.as_str().to_owned(),
            message_type: message.message_type.clone(),
            reply_to: message.reply_to.map(Into::into),
            created_at: message.created_at,
            last_revision_at: message.last_revision.as_ref().map(|rev| rev.updated_at),
            is_deleted: message.is_deleted,
        }
    }
}
