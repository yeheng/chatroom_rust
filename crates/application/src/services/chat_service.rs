use std::sync::Arc;

use domain::{
    self, ChatRoom, ChatRoomVisibility, DomainError, Message, MessageContent, MessageId,
    MessageType, RoomId, RoomMember, RoomRole, UserId,
};
use uuid::Uuid;

use crate::{
    broadcaster::{MessageBroadcast, MessageBroadcaster},
    clock::Clock,
    error::ApplicationError,
    password::PasswordHasher,
    repository::{ChatRoomRepository, MessageRepository, RoomMemberRepository},
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
pub struct InviteMemberRequest {
    pub room_id: Uuid,
    pub inviter_id: Uuid, // 邀请人（从JWT获取）
    pub invitee_id: Uuid, // 被邀请人
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RemoveMemberRequest {
    pub room_id: Uuid,
    pub operator_id: Uuid,    // 操作者（从JWT获取）
    pub target_user_id: Uuid, // 被踢的用户
}

#[derive(Debug, Clone)]
pub struct UpdateRoomRequest {
    pub room_id: Uuid,
    pub operator_id: Uuid, // 操作者（从JWT获取）
    pub name: Option<String>,
    pub visibility: Option<ChatRoomVisibility>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteRoomRequest {
    pub room_id: Uuid,
    pub operator_id: Uuid, // 操作者（从JWT获取）
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

    // 权限检查方法
    async fn check_admin_permission(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<RoomMember, ApplicationError> {
        let member = self
            .deps
            .member_repository
            .find(room_id, user_id)
            .await?
            .ok_or(DomainError::UserNotInRoom)?;

        match member.role {
            RoomRole::Owner | RoomRole::Admin => Ok(member),
            RoomRole::Member => Err(DomainError::OperationNotAllowed.into()),
        }
    }

    async fn check_owner_permission(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<RoomMember, ApplicationError> {
        let member = self
            .deps
            .member_repository
            .find(room_id, user_id)
            .await?
            .ok_or(DomainError::UserNotInRoom)?;

        match member.role {
            RoomRole::Owner => Ok(member),
            RoomRole::Admin | RoomRole::Member => Err(DomainError::OperationNotAllowed.into()),
        }
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
            .broadcast(MessageBroadcast::chat(room_id, stored.clone()))
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

    // 邀请用户加入房间（替代join_room）
    pub async fn invite_member(
        &self,
        request: InviteMemberRequest,
    ) -> Result<(), ApplicationError> {
        let room_id = RoomId::from(request.room_id);
        let inviter_id = UserId::from(request.inviter_id);
        let invitee_id = UserId::from(request.invitee_id);

        // 检查邀请人是否有权限（owner或admin才能邀请）
        self.check_admin_permission(room_id, inviter_id).await?;

        // 检查房间是否存在且开放
        let room = self
            .deps
            .room_repository
            .find_by_id(room_id)
            .await?
            .ok_or(DomainError::RoomNotFound)?;

        if room.is_closed {
            return Err(DomainError::RoomClosed.into());
        }

        // 检查被邀请人是否已经在房间里
        if self
            .deps
            .member_repository
            .find(room_id, invitee_id)
            .await?
            .is_some()
        {
            return Err(DomainError::UserAlreadyInRoom.into());
        }

        // 验证私有房间密码
        if matches!(room.visibility, ChatRoomVisibility::Private) {
            let password = request.password.ok_or(DomainError::RoomIsPrivate)?;
            let hashed = room.password.as_ref().ok_or(DomainError::RoomIsPrivate)?;
            let valid = self.deps.password_hasher.verify(&password, hashed).await?;
            if !valid {
                return Err(DomainError::RoomIsPrivate.into());
            }
        }

        let member = RoomMember::new(room_id, invitee_id, RoomRole::Member, self.deps.clock.now());
        self.deps.member_repository.upsert(member).await?;
        Ok(())
    }

    // 踢出用户（只有owner和admin可以）
    pub async fn remove_member(
        &self,
        request: RemoveMemberRequest,
    ) -> Result<(), ApplicationError> {
        let room_id = RoomId::from(request.room_id);
        let operator_id = UserId::from(request.operator_id);
        let target_user_id = UserId::from(request.target_user_id);

        // 检查操作者权限
        let operator = self.check_admin_permission(room_id, operator_id).await?;

        // 检查目标用户是否在房间里
        let target_member = self
            .deps
            .member_repository
            .find(room_id, target_user_id)
            .await?
            .ok_or(DomainError::UserNotInRoom)?;

        // 权限检查：admin不能踢owner
        if matches!(target_member.role, RoomRole::Owner) {
            return Err(DomainError::OperationNotAllowed.into());
        }

        // admin不能踢admin，只有owner可以踢admin
        if matches!(target_member.role, RoomRole::Admin) && matches!(operator.role, RoomRole::Admin)
        {
            return Err(DomainError::OperationNotAllowed.into());
        }

        // 不能踢自己
        if operator_id == target_user_id {
            return Err(DomainError::OperationNotAllowed.into());
        }

        self.deps
            .member_repository
            .remove(room_id, target_user_id)
            .await?;
        Ok(())
    }

    // 更新房间信息（只有owner和admin可以）
    pub async fn update_room(
        &self,
        request: UpdateRoomRequest,
    ) -> Result<ChatRoom, ApplicationError> {
        let room_id = RoomId::from(request.room_id);
        let operator_id = UserId::from(request.operator_id);

        // 检查操作者权限
        self.check_admin_permission(room_id, operator_id).await?;

        // 获取现有房间
        let mut room = self
            .deps
            .room_repository
            .find_by_id(room_id)
            .await?
            .ok_or(DomainError::RoomNotFound)?;

        // 更新房间信息
        if let Some(name) = request.name {
            room.name = name;
        }

        if let Some(visibility) = request.visibility {
            // 如果改为私有房间，需要密码
            if matches!(visibility, ChatRoomVisibility::Private) && request.password.is_none() {
                return Err(DomainError::RoomIsPrivate.into());
            }
            room.visibility = visibility;
        }

        if let Some(password) = request.password {
            let hashed = self.deps.password_hasher.hash(&password).await?;
            room.password = Some(hashed);
        }

        let updated = self.deps.room_repository.update(room).await?;
        Ok(updated)
    }

    // 删除房间（只有owner可以）
    pub async fn delete_room(&self, request: DeleteRoomRequest) -> Result<(), ApplicationError> {
        let room_id = RoomId::from(request.room_id);
        let operator_id = UserId::from(request.operator_id);

        // 检查操作者权限（只有owner可以删除房间）
        self.check_owner_permission(room_id, operator_id).await?;

        // 验证房间存在
        self.deps
            .room_repository
            .find_by_id(room_id)
            .await?
            .ok_or(DomainError::RoomNotFound)?;

        // 删除房间（这会级联删除成员和消息）
        self.deps.room_repository.delete(room_id).await?;
        Ok(())
    }
}
