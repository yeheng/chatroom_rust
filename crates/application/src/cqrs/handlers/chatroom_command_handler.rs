//! 聊天室命令处理器
//!
//! 处理聊天室相关的命令：创建、加入、离开、发送消息等

use crate::errors::{ApplicationResult, UserError};
use crate::cqrs::{
    CommandHandler,
    commands::*,
    EventBus,
};
use domain::entities::chatroom::{ChatRoom, ChatRoomStatus};
use domain::entities::message::Message;
use domain::entities::room_member::{RoomMember, MemberRole};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use async_trait::async_trait;
use bcrypt;
use tracing::info;

/// 聊天室仓储接口
#[async_trait]
pub trait ChatRoomRepository: Send + Sync {
    async fn save(&self, room: ChatRoom) -> ApplicationResult<ChatRoom>;
    async fn find_by_id(&self, room_id: Uuid) -> ApplicationResult<Option<ChatRoom>>;
    async fn delete(&self, room_id: Uuid) -> ApplicationResult<()>;
    async fn is_user_in_room(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<bool>;
    async fn get_member_count(&self, room_id: Uuid) -> ApplicationResult<u32>;
    async fn find_by_user_id(&self, user_id: Uuid) -> ApplicationResult<Vec<ChatRoom>>;
}

/// 消息仓储接口
#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn save(&self, message: Message) -> ApplicationResult<Message>;
    async fn find_by_id(&self, message_id: Uuid) -> ApplicationResult<Option<Message>>;
    async fn find_by_room_id(&self, room_id: Uuid, limit: Option<u32>) -> ApplicationResult<Vec<Message>>;
    async fn delete(&self, message_id: Uuid) -> ApplicationResult<()>;
}

/// 房间成员仓储接口
#[async_trait]
pub trait RoomMemberRepository: Send + Sync {
    async fn save(&self, member: RoomMember) -> ApplicationResult<RoomMember>;
    async fn find_by_room_and_user(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<Option<RoomMember>>;
    async fn find_by_room_id(&self, room_id: Uuid) -> ApplicationResult<Vec<RoomMember>>;
    async fn remove(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<()>;
}

/// 用户仓储接口引用
pub use super::user_command_handler::UserRepository;

/// 内存聊天室仓储实现
pub struct InMemoryChatRoomRepository {
    rooms: Arc<RwLock<HashMap<Uuid, ChatRoom>>>,
    user_rooms: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>, // user_id -> room_ids
}

impl Default for InMemoryChatRoomRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryChatRoomRepository {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            user_rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ChatRoomRepository for InMemoryChatRoomRepository {
    async fn save(&self, room: ChatRoom) -> ApplicationResult<ChatRoom> {
        let mut rooms = self.rooms.write().await;
        rooms.insert(room.id, room.clone());
        Ok(room)
    }

    async fn find_by_id(&self, room_id: Uuid) -> ApplicationResult<Option<ChatRoom>> {
        let rooms = self.rooms.read().await;
        Ok(rooms.get(&room_id).cloned())
    }

    async fn delete(&self, room_id: Uuid) -> ApplicationResult<()> {
        let mut rooms = self.rooms.write().await;
        if let Some(room) = rooms.get_mut(&room_id) {
            room.status = ChatRoomStatus::Deleted;
            room.updated_at = chrono::Utc::now();
            Ok(())
        } else {
            Err(crate::errors::ApplicationError::NotFound(format!("聊天室不存在: {}", room_id)).into())
        }
    }

    async fn is_user_in_room(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<bool> {
        let user_rooms = self.user_rooms.read().await;
        if let Some(room_ids) = user_rooms.get(&user_id) {
            Ok(room_ids.contains(&room_id))
        } else {
            Ok(false)
        }
    }

    async fn get_member_count(&self, room_id: Uuid) -> ApplicationResult<u32> {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(&room_id) {
            Ok(room.member_count)
        } else {
            Ok(0)
        }
    }

    async fn find_by_user_id(&self, user_id: Uuid) -> ApplicationResult<Vec<ChatRoom>> {
        let user_rooms = self.user_rooms.read().await;
        let rooms = self.rooms.read().await;

        if let Some(room_ids) = user_rooms.get(&user_id) {
            let user_rooms: Vec<ChatRoom> = room_ids
                .iter()
                .filter_map(|room_id| rooms.get(room_id).cloned())
                .filter(|room| room.status != ChatRoomStatus::Deleted)
                .collect();
            Ok(user_rooms)
        } else {
            Ok(Vec::new())
        }
    }
}

/// 内存消息仓储实现
pub struct InMemoryMessageRepository {
    messages: Arc<RwLock<HashMap<Uuid, Message>>>,
    room_messages: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>, // room_id -> message_ids
}

impl Default for InMemoryMessageRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryMessageRepository {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(HashMap::new())),
            room_messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl MessageRepository for InMemoryMessageRepository {
    async fn save(&self, message: Message) -> ApplicationResult<Message> {
        let mut messages = self.messages.write().await;
        let mut room_messages = self.room_messages.write().await;

        // 保存消息
        messages.insert(message.id, message.clone());

        // 更新房间消息索引
        room_messages
            .entry(message.room_id)
            .or_insert_with(Vec::new)
            .push(message.id);

        Ok(message)
    }

    async fn find_by_id(&self, message_id: Uuid) -> ApplicationResult<Option<Message>> {
        let messages = self.messages.read().await;
        Ok(messages.get(&message_id).cloned())
    }

    async fn find_by_room_id(&self, room_id: Uuid, limit: Option<u32>) -> ApplicationResult<Vec<Message>> {
        let messages = self.messages.read().await;
        let room_messages = self.room_messages.read().await;

        if let Some(message_ids) = room_messages.get(&room_id) {
            let mut room_messages: Vec<Message> = message_ids
                .iter()
                .rev() // 最新的消息在前
                .filter_map(|id| messages.get(id).cloned())
                .collect();

            if let Some(limit) = limit {
                room_messages.truncate(limit as usize);
            }

            Ok(room_messages)
        } else {
            Ok(Vec::new())
        }
    }

    async fn delete(&self, message_id: Uuid) -> ApplicationResult<()> {
        let mut messages = self.messages.write().await;
        messages.remove(&message_id);
        Ok(())
    }
}

/// 内存房间成员仓储实现
pub struct InMemoryRoomMemberRepository {
    members: Arc<RwLock<HashMap<(Uuid, Uuid), RoomMember>>>, // (room_id, user_id) -> member
    room_members: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>, // room_id -> user_ids
}

impl Default for InMemoryRoomMemberRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryRoomMemberRepository {
    pub fn new() -> Self {
        Self {
            members: Arc::new(RwLock::new(HashMap::new())),
            room_members: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_member(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<()> {
        let mut room_members = self.room_members.write().await;
        room_members
            .entry(room_id)
            .or_insert_with(Vec::new)
            .push(user_id);
        Ok(())
    }

    pub async fn remove_member(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<()> {
        let mut room_members = self.room_members.write().await;
        if let Some(members) = room_members.get_mut(&room_id) {
            members.retain(|&id| id != user_id);
        }
        Ok(())
    }
}

#[async_trait]
impl RoomMemberRepository for InMemoryRoomMemberRepository {
    async fn save(&self, member: RoomMember) -> ApplicationResult<RoomMember> {
        let mut members = self.members.write().await;
        let key = (member.room_id, member.user_id);
        members.insert(key, member.clone());
        Ok(member)
    }

    async fn find_by_room_and_user(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<Option<RoomMember>> {
        let members = self.members.read().await;
        let key = (room_id, user_id);
        Ok(members.get(&key).cloned())
    }

    async fn find_by_room_id(&self, room_id: Uuid) -> ApplicationResult<Vec<RoomMember>> {
        let members = self.members.read().await;
        let room_members: Vec<RoomMember> = members
            .iter()
            .filter(|((rid, _), _)| *rid == room_id)
            .map(|(_, member)| member.clone())
            .collect();
        Ok(room_members)
    }

    async fn remove(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<()> {
        let mut members = self.members.write().await;
        let key = (room_id, user_id);
        members.remove(&key);
        Ok(())
    }
}

/// 聊天室命令处理器
pub struct ChatRoomCommandHandler {
    room_repository: Arc<dyn ChatRoomRepository>,
    message_repository: Arc<dyn MessageRepository>,
    member_repository: Arc<dyn RoomMemberRepository>,
    user_repository: Arc<dyn UserRepository>,
    event_bus: Option<Arc<dyn EventBus>>,
}

impl ChatRoomCommandHandler {
    pub fn new(
        room_repository: Arc<dyn ChatRoomRepository>,
        message_repository: Arc<dyn MessageRepository>,
        member_repository: Arc<dyn RoomMemberRepository>,
        user_repository: Arc<dyn UserRepository>,
    ) -> Self {
        Self {
            room_repository,
            message_repository,
            member_repository,
            user_repository,
            event_bus: None,
        }
    }

    pub fn with_event_bus(mut self, event_bus: Arc<dyn EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// 验证房间名称
    fn validate_room_name(name: &str) -> ApplicationResult<()> {
        if name.is_empty() {
            return Err(crate::errors::ApplicationError::Validation("房间名称不能为空".to_string()));
        }

        if name.len() < 2 {
            return Err(crate::errors::ApplicationError::Validation("房间名称长度至少2个字符".to_string()));
        }

        if name.len() > 100 {
            return Err(crate::errors::ApplicationError::Validation("房间名称长度不能超过100个字符".to_string()));
        }

        Ok(())
    }

    /// 验证消息内容
    fn validate_message_content(content: &str) -> ApplicationResult<()> {
        if content.is_empty() {
            return Err(crate::errors::ApplicationError::Validation("消息内容不能为空".to_string()));
        }

        if content.len() > 10000 {
            return Err(crate::errors::ApplicationError::Validation("消息长度不能超过10000个字符".to_string()));
        }

        Ok(())
    }

    /// 哈希房间密码
    async fn hash_password(&self, password: &str) -> ApplicationResult<String> {
        let password = password.to_string();
        tokio::task::spawn_blocking(move || {
            bcrypt::hash(&password, bcrypt::DEFAULT_COST)
                .map_err(|e| crate::errors::ApplicationError::Infrastructure(format!("密码哈希失败: {}", e)))
        })
        .await
        .map_err(|e| {
            crate::errors::ApplicationError::Infrastructure(format!("密码哈希任务失败: {}", e))
        })?
    }

    /// 验证房间密码
    async fn verify_password(&self, password: &str, hash: &str) -> ApplicationResult<bool> {
        let password = password.to_string();
        let hash = hash.to_string();
        tokio::task::spawn_blocking(move || {
            bcrypt::verify(&password, &hash)
                .map_err(|e| crate::errors::ApplicationError::Infrastructure(format!("密码验证失败: {}", e)))
        })
        .await
        .map_err(|e| {
            crate::errors::ApplicationError::Infrastructure(format!("密码验证任务失败: {}", e))
        })?
    }
}

#[async_trait]
impl CommandHandler<CreateChatRoomCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: CreateChatRoomCommand) -> ApplicationResult<ChatRoom> {
        info!("处理创建聊天室命令: {}", command.name);

        // 验证输入
        Self::validate_room_name(&command.name)?;

        // 验证用户存在
        let _owner = self.user_repository
            .find_by_id(command.owner_id)
            .await?
            .ok_or_else(|| UserError::UserNotFound(command.owner_id))?;

        // 哈希密码（如果是私密房间）
        let password_hash = if command.is_private {
            if let Some(ref password) = command.password {
                Some(self.hash_password(password).await?)
            } else {
                return Err(crate::errors::ApplicationError::Validation("私密房间必须设置密码".to_string()));
            }
        } else {
            None
        };

        // 创建聊天室
        let room = if command.is_private {
            // 私密房间需要密码哈希
            let password = password_hash.as_ref()
                .ok_or_else(|| crate::errors::ApplicationError::Validation("私密房间需要密码".to_string()))?;
            ChatRoom::new_private(
                command.name,
                command.description,
                command.owner_id,
                password,
            )
        } else {
            ChatRoom::new_public(
                command.name,
                command.owner_id,
                command.description,
                command.max_members,
            )
        }
        .map_err(crate::errors::ApplicationError::from)?;

        // 保存聊天室
        let saved_room = self.room_repository.save(room).await?;

        // 创建房主成员记录
        let owner_member = RoomMember::new(
            saved_room.id,
            command.owner_id,
            MemberRole::Owner,
        );

        self.member_repository.save(owner_member).await?;

        info!("聊天室创建成功: {} ({})", saved_room.name, saved_room.id);
        Ok(saved_room)
    }
}

#[async_trait]
impl CommandHandler<JoinChatRoomCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: JoinChatRoomCommand) -> ApplicationResult<()> {
        info!("处理加入聊天室命令: 用户 {} 加入房间 {}", command.user_id, command.room_id);

        // 验证用户存在
        let _user = self.user_repository
            .find_by_id(command.user_id)
            .await?
            .ok_or_else(|| UserError::UserNotFound(command.user_id))?;

        // 获取聊天室
        let mut room = self.room_repository
            .find_by_id(command.room_id)
            .await?
            .ok_or_else(|| crate::errors::ApplicationError::NotFound(format!("聊天室不存在: {}", command.room_id)))?;

        // 检查房间状态
        if room.status != ChatRoomStatus::Active {
            return Err(crate::errors::ApplicationError::Validation("房间不可用".to_string()));
        }

        // 检查用户是否已在房间中
        if self.room_repository.is_user_in_room(command.room_id, command.user_id).await? {
            return Err(crate::errors::ApplicationError::Validation("用户已在房间中".to_string()));
        }

        // 验证私密房间密码
        if room.is_private {
            if let Some(ref password_hash) = room.password_hash {
                if let Some(ref password) = command.password {
                    let is_valid = self.verify_password(password, password_hash).await?;
                    if !is_valid {
                        return Err(crate::errors::ApplicationError::Validation("房间密码错误".to_string()));
                    }
                } else {
                    return Err(crate::errors::ApplicationError::Validation("私密房间需要密码".to_string()));
                }
            }
        }

        // 检查房间成员数量限制
        if let Some(max_members) = room.max_members {
            if room.member_count >= max_members {
                return Err(crate::errors::ApplicationError::Validation("房间已满".to_string()));
            }
        }

        // 创建成员记录
        let member = RoomMember::new(
            command.room_id,
            command.user_id,
            MemberRole::Member,
        );

        self.member_repository.save(member).await?;

        // 更新房间成员数量
        room.member_count += 1;
        room.updated_at = chrono::Utc::now();
        self.room_repository.save(room).await?;

        info!("用户 {} 成功加入房间 {}", command.user_id, command.room_id);
        Ok(())
    }
}

#[async_trait]
impl CommandHandler<LeaveChatRoomCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: LeaveChatRoomCommand) -> ApplicationResult<()> {
        info!("处理离开聊天室命令: 用户 {} 离开房间 {}", command.user_id, command.room_id);

        // 验证用户在房间中
        if !self.room_repository.is_user_in_room(command.room_id, command.user_id).await? {
            return Err(crate::errors::ApplicationError::Validation("用户不在房间中".to_string()));
        }

        // 获取聊天室
        let mut room = self.room_repository
            .find_by_id(command.room_id)
            .await?
            .ok_or_else(|| crate::errors::ApplicationError::NotFound(format!("聊天室不存在: {}", command.room_id)))?;

        // 检查是否是房主（房主不能直接离开，需要先转让或删除房间）
        if room.owner_id == command.user_id {
            return Err(crate::errors::ApplicationError::Validation("房主不能离开房间，请先转让房间或删除房间".to_string()));
        }

        // 移除成员记录
        self.member_repository.remove(command.room_id, command.user_id).await?;

        // 更新房间成员数量
        if room.member_count > 0 {
            room.member_count -= 1;
            room.updated_at = chrono::Utc::now();
            self.room_repository.save(room).await?;
        }

        info!("用户 {} 成功离开房间 {}", command.user_id, command.room_id);
        Ok(())
    }
}

#[async_trait]
impl CommandHandler<SendMessageCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: SendMessageCommand) -> ApplicationResult<Message> {
        info!("处理发送消息命令: 用户 {} 在房间 {} 发送消息", command.user_id, command.room_id);

        // 验证消息内容
        Self::validate_message_content(&command.content)?;

        // 验证用户在房间中
        if !self.room_repository.is_user_in_room(command.room_id, command.user_id).await? {
            return Err(crate::errors::ApplicationError::Validation("用户不在房间中".to_string()));
        }

        // 验证回复消息（如果有）
        if let Some(reply_to_id) = command.reply_to_message_id {
            let reply_message = self.message_repository
                .find_by_id(reply_to_id)
                .await?
                .ok_or_else(|| crate::errors::ApplicationError::NotFound(format!("回复的消息不存在: {}", reply_to_id)))?;

            // 确保回复的消息在同一个房间
            if reply_message.room_id != command.room_id {
                return Err(crate::errors::ApplicationError::Validation("只能回复同一房间的消息".to_string()));
            }
        }

        // 创建消息
        let message = if let Some(reply_to_id) = command.reply_to_message_id {
            Message::new_reply(
                command.room_id,
                command.user_id,
                command.content,
                reply_to_id,
            )
        } else {
            Message::new_text(
                command.room_id,
                command.user_id,
                command.content,
            )
        }
        .map_err(crate::errors::ApplicationError::from)?;

        // 保存消息
        let saved_message = self.message_repository.save(message).await?;

        info!("消息发送成功: {} ({})", saved_message.content, saved_message.id);
        Ok(saved_message)
    }
}

#[async_trait]
impl CommandHandler<UpdateChatRoomCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: UpdateChatRoomCommand) -> ApplicationResult<ChatRoom> {
        info!("处理更新聊天室命令: {}", command.room_id);

        // 获取聊天室
        let mut room = self.room_repository
            .find_by_id(command.room_id)
            .await?
            .ok_or_else(|| crate::errors::ApplicationError::NotFound(format!("聊天室不存在: {}", command.room_id)))?;

        // 验证权限（只有房主或管理员可以更新）
        let member = self.member_repository
            .find_by_room_and_user(command.room_id, command.user_id)
            .await?
            .ok_or_else(|| crate::errors::ApplicationError::Validation("用户不在房间中".to_string()))?;

        if member.role != MemberRole::Owner && member.role != MemberRole::Admin {
            return Err(crate::errors::ApplicationError::Validation("没有权限更新房间信息".to_string()));
        }

        // 更新房间信息
        let mut updated = false;

        if let Some(ref name) = command.name {
            Self::validate_room_name(name)?;
            room.name = name.clone();
            updated = true;
        }

        if let Some(ref description) = command.description {
            room.description = Some(description.clone());
            updated = true;
        }

        if let Some(max_members) = command.max_members {
            if max_members > 0 && (max_members as u32) < room.member_count {
                return Err(crate::errors::ApplicationError::Validation(
                    format!("最大成员数不能少于当前成员数: {}", room.member_count)
                ));
            }
            room.max_members = Some(max_members);
            updated = true;
        }

        if updated {
            room.updated_at = chrono::Utc::now();
            let saved_room = self.room_repository.save(room).await?;
            info!("聊天室更新成功: {}", saved_room.id);
            Ok(saved_room)
        } else {
            Ok(room)
        }
    }
}

#[async_trait]
impl CommandHandler<DeleteChatRoomCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: DeleteChatRoomCommand) -> ApplicationResult<()> {
        info!("处理删除聊天室命令: {}", command.room_id);

        // 获取聊天室
        let room = self.room_repository
            .find_by_id(command.room_id)
            .await?
            .ok_or_else(|| crate::errors::ApplicationError::NotFound(format!("聊天室不存在: {}", command.room_id)))?;

        // 验证权限（只有房主可以删除）
        if room.owner_id != command.user_id {
            return Err(crate::errors::ApplicationError::Validation("只有房主可以删除房间".to_string()));
        }

        // 软删除房间
        self.room_repository.delete(command.room_id).await?;

        // 移除所有成员记录
        let members = self.member_repository.find_by_room_id(command.room_id).await?;
        for member in members {
            self.member_repository.remove(command.room_id, member.user_id).await?;
        }

        info!("聊天室删除成功: {}", command.room_id);
        Ok(())
    }
}

#[async_trait]
impl CommandHandler<UpdateMessageCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: UpdateMessageCommand) -> ApplicationResult<Message> {
        info!("处理更新消息命令: {}", command.message_id);

        // 获取消息
        let mut message = self.message_repository
            .find_by_id(command.message_id)
            .await?
            .ok_or_else(|| crate::errors::ApplicationError::NotFound(format!("消息不存在: {}", command.message_id)))?;

        // 验证权限（只有消息发送者可以修改）
        if message.sender_id != command.user_id {
            return Err(crate::errors::ApplicationError::Validation("只能修改自己发送的消息".to_string()));
        }

        // 验证消息内容
        Self::validate_message_content(&command.content)?;

        // 更新消息内容
        message.content = command.content;
        message.updated_at = Some(chrono::Utc::now());

        // 保存更新的消息
        let updated_message = self.message_repository.save(message).await?;

        info!("消息更新成功: {}", command.message_id);
        Ok(updated_message)
    }
}

#[async_trait]
impl CommandHandler<DeleteMessageCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: DeleteMessageCommand) -> ApplicationResult<()> {
        info!("处理删除消息命令: {}", command.message_id);

        // 获取消息
        let message = self.message_repository
            .find_by_id(command.message_id)
            .await?
            .ok_or_else(|| crate::errors::ApplicationError::NotFound(format!("消息不存在: {}", command.message_id)))?;

        // 验证权限（消息发送者或房间管理员可以删除）
        let can_delete = if message.sender_id == command.user_id {
            // 消息发送者可以删除
            true
        } else {
            // 检查是否是房间管理员或房主
            if let Some(member) = self.member_repository
                .find_by_room_and_user(message.room_id, command.user_id)
                .await?
            {
                member.role == MemberRole::Owner || member.role == MemberRole::Admin
            } else {
                false
            }
        };

        if !can_delete {
            return Err(crate::errors::ApplicationError::Validation("没有权限删除此消息".to_string()));
        }

        // 删除消息
        self.message_repository.delete(command.message_id).await?;

        info!("消息删除成功: {}", command.message_id);
        Ok(())
    }
}