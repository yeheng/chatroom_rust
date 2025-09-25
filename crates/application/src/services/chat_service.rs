use std::sync::Arc;

use domain::{
    self, ChatRoom, ChatRoomRepository, ChatRoomVisibility, DomainError, Message, MessageContent,
    MessageId, MessageRepository, MessageType, RoomId, RoomMember, RoomMemberRepository, RoomRole,
    UserId,
};
use uuid::Uuid;

use crate::{
    broadcaster::{MessageBroadcast, MessageBroadcaster},
    clock::Clock,
    error::ApplicationError,
    password::PasswordHasher,
};

#[derive(Debug, Clone)]
pub struct CreateRoomRequest {
    pub name: String,
    pub owner_id: Uuid,
    pub visibility: ChatRoomVisibility,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JoinRoomRequest {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LeaveRoomRequest {
    pub room_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct SendMessageRequest {
    pub room_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to: Option<Uuid>,
}

pub struct ChatServiceDependencies {
    pub room_repository: Arc<dyn ChatRoomRepository>,
    pub member_repository: Arc<dyn RoomMemberRepository>,
    pub message_repository: Arc<dyn MessageRepository>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub clock: Arc<dyn Clock>,
    pub broadcaster: Arc<dyn MessageBroadcaster>,
}

pub struct ChatService {
    deps: ChatServiceDependencies,
}

impl ChatService {
    pub fn new(deps: ChatServiceDependencies) -> Self {
        Self { deps }
    }

    pub async fn create_room(
        &self,
        request: CreateRoomRequest,
    ) -> Result<ChatRoom, ApplicationError> {
        let owner_id = UserId::from(request.owner_id);
        let now = self.deps.clock.now();
        let room_id = RoomId::from(Uuid::new_v4());

        let room = match request.visibility {
            ChatRoomVisibility::Public => {
                ChatRoom::new_public(room_id, request.name, owner_id, now)?
            }
            ChatRoomVisibility::Private => {
                let password = request.password.ok_or(DomainError::RoomIsPrivate)?;
                let hashed = self.deps.password_hasher.hash(&password).await?;
                ChatRoom::new_private(room_id, request.name, owner_id, hashed, now)?
            }
        };

        let stored = self.deps.room_repository.create(room).await?;
        let owner_member = RoomMember::new(stored.id, stored.owner_id, RoomRole::Owner, now);
        self.deps.member_repository.upsert(owner_member).await?;

        Ok(stored)
    }

    pub async fn join_room(&self, request: JoinRoomRequest) -> Result<(), ApplicationError> {
        let room_id = RoomId::from(request.room_id);
        let user_id = UserId::from(request.user_id);

        let room = self
            .deps
            .room_repository
            .find_by_id(room_id)
            .await?
            .ok_or(DomainError::RoomNotFound)?;

        if room.is_closed {
            return Err(DomainError::RoomClosed.into());
        }

        if self
            .deps
            .member_repository
            .find(room_id, user_id)
            .await?
            .is_some()
        {
            return Err(DomainError::UserAlreadyInRoom.into());
        }

        if matches!(room.visibility, ChatRoomVisibility::Private) {
            let password = request.password.ok_or(DomainError::RoomIsPrivate)?;
            let hashed = room.password.as_ref().ok_or(DomainError::RoomIsPrivate)?;
            let valid = self.deps.password_hasher.verify(&password, hashed).await?;
            if !valid {
                return Err(DomainError::RoomIsPrivate.into());
            }
        }

        let member = RoomMember::new(room_id, user_id, RoomRole::Member, self.deps.clock.now());
        self.deps.member_repository.upsert(member).await?;
        Ok(())
    }

    pub async fn leave_room(&self, request: LeaveRoomRequest) -> Result<(), ApplicationError> {
        let room_id = RoomId::from(request.room_id);
        let user_id = UserId::from(request.user_id);

        let member_exists = self
            .deps
            .member_repository
            .find(room_id, user_id)
            .await?
            .is_some();
        if !member_exists {
            return Err(DomainError::UserNotInRoom.into());
        }

        self.deps.member_repository.remove(room_id, user_id).await?;
        Ok(())
    }

    pub async fn send_message(
        &self,
        request: SendMessageRequest,
    ) -> Result<Message, ApplicationError> {
        let room_id = RoomId::from(request.room_id);
        let sender_id = UserId::from(request.sender_id);

        let room = self
            .deps
            .room_repository
            .find_by_id(room_id)
            .await?
            .ok_or(DomainError::RoomNotFound)?;

        if room.is_closed {
            return Err(DomainError::RoomClosed.into());
        }

        self.deps
            .member_repository
            .find(room_id, sender_id)
            .await?
            .ok_or(DomainError::UserNotInRoom)?;

        let content = MessageContent::new(request.content)?;
        let reply_to = request.reply_to.map(MessageId::from);
        let now = self.deps.clock.now();

        let message = Message::new(
            MessageId::from(Uuid::new_v4()),
            room_id,
            sender_id,
            content,
            request.message_type,
            reply_to,
            now,
        )?;

        let stored = self.deps.message_repository.create(message).await?;

        self.deps
            .broadcaster
            .broadcast(MessageBroadcast {
                room_id,
                message: stored.clone(),
            })
            .await?;

        Ok(stored)
    }

    pub async fn get_history(
        &self,
        room_id: Uuid,
        limit: u32,
        before: Option<Uuid>,
    ) -> Result<Vec<Message>, ApplicationError> {
        let room_id = RoomId::from(room_id);
        let before = before.map(MessageId::from);

        let records = self
            .deps
            .message_repository
            .list_recent(room_id, limit, before)
            .await?;

        Ok(records)
    }
}
