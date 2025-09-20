//! 基于 CQRS 的聊天室应用服务
//!
//! 提供聊天室相关的高级业务逻辑

use crate::cqrs::{
    commands::{
        CreateChatRoomCommand, DeleteChatRoomCommand, DeleteMessageCommand, JoinChatRoomCommand,
        LeaveChatRoomCommand, SendMessageCommand, UpdateChatRoomCommand, UpdateMessageCommand,
    },
    dtos::{ChatRoomDetailDto, ChatRoomDto, MessageDto, RoomMemberDto},
    handlers::{ChatRoomCommandHandler, ChatRoomQueryHandler},
    queries::{
        GetChatRoomByIdQuery, GetChatRoomDetailQuery, GetRoomMembersQuery, GetRoomMessagesQuery,
        GetUserRoomsQuery, SearchPublicRoomsQuery,
    },
    CommandHandler, QueryHandler,
};
use crate::errors::ApplicationResult;
use domain::entities::message::MessageType;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// 基于 CQRS 的聊天室应用服务
pub struct CqrsChatRoomService {
    room_command_handler: Arc<ChatRoomCommandHandler>,
    room_query_handler: Arc<ChatRoomQueryHandler>,
}

impl CqrsChatRoomService {
    /// 创建新的聊天室服务实例
    pub fn new(
        room_command_handler: Arc<ChatRoomCommandHandler>,
        room_query_handler: Arc<ChatRoomQueryHandler>,
    ) -> Self {
        Self {
            room_command_handler,
            room_query_handler,
        }
    }

    /// 创建聊天室
    pub async fn create_room(
        &self,
        name: String,
        description: Option<String>,
        owner_id: Uuid,
        is_private: bool,
        password: Option<String>,
        max_members: Option<u32>,
    ) -> ApplicationResult<ChatRoomDto> {
        info!("创建聊天室: {} (所有者: {})", name, owner_id);

        let command = CreateChatRoomCommand {
            name,
            description,
            owner_id,
            is_private,
            password,
            max_members,
        };

        let room = self.room_command_handler.handle(command).await?;

        Ok(ChatRoomDto {
            id: room.id,
            name: room.name,
            description: room.description,
            owner_id: room.owner_id,
            is_private: room.is_private,
            member_count: room.member_count,
            max_members: room.max_members,
            created_at: room.created_at,
            updated_at: room.updated_at,
        })
    }

    /// 加入聊天室
    pub async fn join_room(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        password: Option<String>,
    ) -> ApplicationResult<()> {
        info!("用户 {} 加入房间 {}", user_id, room_id);

        let command = JoinChatRoomCommand {
            room_id,
            user_id,
            password,
        };

        self.room_command_handler.handle(command).await
    }

    /// 离开聊天室
    pub async fn leave_room(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<()> {
        info!("用户 {} 离开房间 {}", user_id, room_id);

        let command = LeaveChatRoomCommand { room_id, user_id };

        self.room_command_handler.handle(command).await
    }

    /// 发送消息
    pub async fn send_message(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        content: String,
        message_type: MessageType,
        reply_to_message_id: Option<Uuid>,
    ) -> ApplicationResult<MessageDto> {
        info!("用户 {} 在房间 {} 发送消息", user_id, room_id);

        let command = SendMessageCommand {
            room_id,
            user_id,
            content,
            message_type,
            reply_to_message_id,
        };

        let message = self.room_command_handler.handle(command).await?;

        Ok(MessageDto {
            id: message.id,
            room_id: message.room_id,
            user_id: message.sender_id,
            username: "Unknown".to_string(), // 实际应该从用户服务获取
            display_name: None,
            avatar_url: None,
            content: message.content,
            message_type: message.message_type,
            reply_to_message_id: message.reply_to_id,
            reply_to_username: None,
            created_at: message.created_at,
            updated_at: message.updated_at.unwrap_or(message.created_at),
            edited: message.updated_at.is_some(),
        })
    }

    /// 更新聊天室信息
    pub async fn update_room(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        max_members: Option<u32>,
    ) -> ApplicationResult<ChatRoomDto> {
        info!("更新聊天室: {}", room_id);

        let command = UpdateChatRoomCommand {
            room_id,
            user_id,
            name,
            description,
            is_private: None, // 不允许更改私密状态
            max_members,
        };

        let room = self.room_command_handler.handle(command).await?;

        Ok(ChatRoomDto {
            id: room.id,
            name: room.name,
            description: room.description,
            owner_id: room.owner_id,
            is_private: room.is_private,
            member_count: room.member_count,
            max_members: room.max_members,
            created_at: room.created_at,
            updated_at: room.updated_at,
        })
    }

    /// 删除聊天室
    pub async fn delete_room(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<()> {
        info!("删除聊天室: {}", room_id);

        let command = DeleteChatRoomCommand { room_id, user_id };

        self.room_command_handler.handle(command).await
    }

    /// 更新消息
    pub async fn update_message(
        &self,
        message_id: Uuid,
        user_id: Uuid,
        content: String,
    ) -> ApplicationResult<MessageDto> {
        info!("更新消息: {}", message_id);

        let command = UpdateMessageCommand {
            message_id,
            user_id,
            content,
        };

        let message = self.room_command_handler.handle(command).await?;

        Ok(MessageDto {
            id: message.id,
            room_id: message.room_id,
            user_id: message.sender_id,
            username: "Unknown".to_string(),
            display_name: None,
            avatar_url: None,
            content: message.content,
            message_type: message.message_type,
            reply_to_message_id: message.reply_to_id,
            reply_to_username: None,
            created_at: message.created_at,
            updated_at: message.updated_at.unwrap_or(message.created_at),
            edited: message.updated_at.is_some(),
        })
    }

    /// 删除消息
    pub async fn delete_message(&self, message_id: Uuid, user_id: Uuid) -> ApplicationResult<()> {
        info!("删除消息: {}", message_id);

        let command = DeleteMessageCommand {
            message_id,
            user_id,
        };

        self.room_command_handler.handle(command).await
    }

    /// 获取聊天室信息
    pub async fn get_room(&self, room_id: Uuid) -> ApplicationResult<Option<ChatRoomDto>> {
        let query = GetChatRoomByIdQuery { room_id };
        self.room_query_handler.handle(query).await
    }

    /// 获取聊天室详细信息
    pub async fn get_room_detail(
        &self,
        room_id: Uuid,
    ) -> ApplicationResult<Option<ChatRoomDetailDto>> {
        let query = GetChatRoomDetailQuery { room_id };
        self.room_query_handler.handle(query).await
    }

    /// 获取房间消息
    pub async fn get_room_messages(
        &self,
        room_id: Uuid,
        limit: Option<u32>,
    ) -> ApplicationResult<Vec<MessageDto>> {
        let query = GetRoomMessagesQuery {
            room_id,
            limit,
            offset: None,
            before: None,
            after: None,
        };
        self.room_query_handler.handle(query).await
    }

    /// 获取房间成员
    pub async fn get_room_members(&self, room_id: Uuid) -> ApplicationResult<Vec<RoomMemberDto>> {
        let query = GetRoomMembersQuery {
            room_id,
            limit: None,
            offset: None,
        };
        self.room_query_handler.handle(query).await
    }

    /// 获取用户的聊天室列表
    pub async fn get_user_rooms(&self, user_id: Uuid) -> ApplicationResult<Vec<ChatRoomDto>> {
        let query = GetUserRoomsQuery {
            user_id,
            limit: None,
            offset: None,
        };
        self.room_query_handler.handle(query).await
    }

    /// 搜索公开聊天室
    pub async fn search_public_rooms(
        &self,
        keyword: Option<String>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> ApplicationResult<Vec<ChatRoomDto>> {
        let query = SearchPublicRoomsQuery {
            keyword,
            limit,
            offset,
        };
        self.room_query_handler.handle(query).await
    }

    /// 验证用户是否在房间中
    pub async fn verify_user_in_room(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> ApplicationResult<bool> {
        // 通过获取房间成员列表来验证
        let members = self.get_room_members(room_id).await?;
        Ok(members.iter().any(|member| member.user_id == user_id))
    }

    /// 获取房间成员数量
    pub async fn get_room_member_count(&self, room_id: Uuid) -> ApplicationResult<u32> {
        match self.get_room(room_id).await? {
            Some(room) => Ok(room.member_count),
            None => Err(crate::errors::ApplicationError::NotFound(format!(
                "房间不存在: {}",
                room_id
            ))),
        }
    }
}
