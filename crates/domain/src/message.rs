use crate::errors::DomainError;
use crate::value_objects::{MessageContent, MessageId, RoomId, Timestamp, UserId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "message_type")]
pub enum MessageType {
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MessageRevision {
    pub content: MessageContent,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub room_id: RoomId,
    pub sender_id: UserId,
    pub content: MessageContent,
    pub message_type: MessageType,
    pub reply_to: Option<MessageId>,
    pub created_at: Timestamp,
    pub last_revision: Option<MessageRevision>,
    #[serde(skip_serializing)] // 删除标记不暴露给客户端
    pub is_deleted: bool,
}

impl Message {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: MessageId,
        room_id: RoomId,
        sender_id: UserId,
        content: MessageContent,
        message_type: MessageType,
        reply_to: Option<MessageId>,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            room_id,
            sender_id,
            content,
            message_type,
            reply_to,
            created_at,
            last_revision: None,
            is_deleted: false,
        })
    }

    pub fn edit(&mut self, new_content: MessageContent, at: Timestamp) -> Result<(), DomainError> {
        if self.is_deleted {
            return Err(DomainError::OperationNotAllowed);
        }
        self.last_revision = Some(MessageRevision {
            content: self.content.clone(),
            updated_at: at,
        });
        self.content = new_content;
        Ok(())
    }

    pub fn mark_deleted(&mut self) {
        self.is_deleted = true;
    }
}
