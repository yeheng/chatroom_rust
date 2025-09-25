use crate::errors::DomainError;
use crate::value_objects::{PasswordHash, RoomId, Timestamp, UserId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChatRoomVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChatRoom {
    pub id: RoomId,
    pub name: String,
    pub owner_id: UserId,
    pub visibility: ChatRoomVisibility,
    #[serde(skip_serializing)] // 房间密码不暴露给客户端
    pub password: Option<PasswordHash>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub is_closed: bool,
}

impl ChatRoom {
    pub fn new_public(
        id: RoomId,
        name: impl Into<String>,
        owner_id: UserId,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        let name = Self::validate_name(name.into())?;
        Ok(Self {
            id,
            name,
            owner_id,
            visibility: ChatRoomVisibility::Public,
            password: None,
            created_at,
            updated_at: created_at,
            is_closed: false,
        })
    }

    pub fn new_private(
        id: RoomId,
        name: impl Into<String>,
        owner_id: UserId,
        password: PasswordHash,
        created_at: Timestamp,
    ) -> Result<Self, DomainError> {
        let name = Self::validate_name(name.into())?;
        Ok(Self {
            id,
            name,
            owner_id,
            visibility: ChatRoomVisibility::Private,
            password: Some(password),
            created_at,
            updated_at: created_at,
            is_closed: false,
        })
    }

    pub fn rename(&mut self, name: impl Into<String>, now: Timestamp) -> Result<(), DomainError> {
        let name = Self::validate_name(name.into())?;
        self.name = name;
        self.updated_at = now;
        Ok(())
    }

    pub fn change_owner(&mut self, owner_id: UserId, now: Timestamp) {
        self.owner_id = owner_id;
        self.updated_at = now;
    }

    pub fn set_private(&mut self, password: PasswordHash, now: Timestamp) {
        self.visibility = ChatRoomVisibility::Private;
        self.password = Some(password);
        self.updated_at = now;
    }

    pub fn set_public(&mut self, now: Timestamp) {
        self.visibility = ChatRoomVisibility::Public;
        self.password = None;
        self.updated_at = now;
    }

    pub fn close(&mut self, now: Timestamp) {
        self.is_closed = true;
        self.updated_at = now;
    }

    pub fn reopen(&mut self, now: Timestamp) {
        self.is_closed = false;
        self.updated_at = now;
    }

    fn validate_name(name: String) -> Result<String, DomainError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(DomainError::invalid_argument(
                "room_name",
                "cannot be empty",
            ));
        }
        if trimmed.len() > 60 {
            return Err(DomainError::invalid_argument("room_name", "too long"));
        }
        Ok(trimmed.to_owned())
    }
}
