//! 聊天室查询处理器
//!
//! 处理聊天室相关的查询：查找房间、获取消息历史等

use crate::errors::ApplicationResult;
use crate::cqrs::{
    QueryHandler,
    queries::*,
    dtos::*,
};
use domain::entities::chatroom::ChatRoom;
use domain::entities::message::Message;
use domain::entities::room_member::RoomMember;
use super::chatroom_command_handler::{ChatRoomRepository, MessageRepository, RoomMemberRepository};
use std::sync::Arc;
use async_trait::async_trait;

/// 聊天室查询处理器
pub struct ChatRoomQueryHandler {
    room_repository: Arc<dyn ChatRoomRepository>,
    message_repository: Arc<dyn MessageRepository>,
    member_repository: Arc<dyn RoomMemberRepository>,
}

impl ChatRoomQueryHandler {
    pub fn new(
        room_repository: Arc<dyn ChatRoomRepository>,
        message_repository: Arc<dyn MessageRepository>,
        member_repository: Arc<dyn RoomMemberRepository>,
    ) -> Self {
        Self {
            room_repository,
            message_repository,
            member_repository,
        }
    }

    /// 将 ChatRoom 实体转换为 ChatRoomDto
    fn room_to_dto(&self, room: ChatRoom) -> ChatRoomDto {
        ChatRoomDto {
            id: room.id,
            name: room.name,
            description: room.description,
            owner_id: room.owner_id,
            is_private: room.is_private,
            member_count: room.member_count,
            max_members: room.max_members,
            created_at: room.created_at,
            updated_at: room.updated_at,
        }
    }

    /// 将 Message 实体转换为 MessageDto
    fn message_to_dto(&self, message: Message) -> MessageDto {
        MessageDto {
            id: message.id,
            room_id: message.room_id,
            user_id: message.sender_id,
            username: "Unknown".to_string(), // 实际应该从用户服务获取
            display_name: None,
            avatar_url: None,
            content: message.content,
            message_type: message.message_type,
            reply_to_message_id: message.reply_to_id,
            reply_to_username: None, // 实际应该根据 reply_to_id 获取
            created_at: message.created_at,
            updated_at: message.updated_at.unwrap_or(message.created_at),
            edited: message.updated_at.is_some(),
        }
    }

    /// 将 RoomMember 实体转换为 RoomMemberDto
    fn member_to_dto(&self, member: RoomMember) -> RoomMemberDto {
        RoomMemberDto {
            user_id: member.user_id,
            username: "Unknown".to_string(), // 实际应该从用户服务获取
            display_name: None,
            avatar_url: None,
            role: member.role,
            joined_at: member.joined_at,
            last_active_at: None, // 实际应该从用户活动记录获取
        }
    }
}

#[async_trait]
impl QueryHandler<GetChatRoomByIdQuery> for ChatRoomQueryHandler {
    async fn handle(&self, query: GetChatRoomByIdQuery) -> ApplicationResult<Option<ChatRoomDto>> {
        let room = self.room_repository.find_by_id(query.room_id).await?;
        Ok(room.map(|r| self.room_to_dto(r)))
    }
}

#[async_trait]
impl QueryHandler<GetRoomMessagesQuery> for ChatRoomQueryHandler {
    async fn handle(&self, query: GetRoomMessagesQuery) -> ApplicationResult<Vec<MessageDto>> {
        let messages = self.message_repository
            .find_by_room_id(query.room_id, query.limit)
            .await?;

        let message_dtos: Vec<MessageDto> = messages
            .into_iter()
            .map(|m| self.message_to_dto(m))
            .collect();

        Ok(message_dtos)
    }
}

#[async_trait]
impl QueryHandler<GetRoomMembersQuery> for ChatRoomQueryHandler {
    async fn handle(&self, query: GetRoomMembersQuery) -> ApplicationResult<Vec<RoomMemberDto>> {
        let members = self.member_repository.find_by_room_id(query.room_id).await?;

        let member_dtos: Vec<RoomMemberDto> = members
            .into_iter()
            .map(|m| self.member_to_dto(m))
            .collect();

        Ok(member_dtos)
    }
}

#[async_trait]
impl QueryHandler<GetUserRoomsQuery> for ChatRoomQueryHandler {
    async fn handle(&self, query: GetUserRoomsQuery) -> ApplicationResult<Vec<ChatRoomDto>> {
        let rooms = self.room_repository.find_by_user_id(query.user_id).await?;

        let room_dtos: Vec<ChatRoomDto> = rooms
            .into_iter()
            .map(|r| self.room_to_dto(r))
            .collect();

        Ok(room_dtos)
    }
}

#[async_trait]
impl QueryHandler<SearchPublicRoomsQuery> for ChatRoomQueryHandler {
    async fn handle(&self, query: SearchPublicRoomsQuery) -> ApplicationResult<Vec<ChatRoomDto>> {
        // 简化实现：暂时返回空列表
        // 实际应该根据 keyword、limit、offset 进行搜索
        let _keyword = query.keyword;
        let _limit = query.limit;
        let _offset = query.offset;

        Ok(Vec::new())
    }
}

#[async_trait]
impl QueryHandler<GetChatRoomDetailQuery> for ChatRoomQueryHandler {
    async fn handle(&self, query: GetChatRoomDetailQuery) -> ApplicationResult<Option<ChatRoomDetailDto>> {
        let room = match self.room_repository.find_by_id(query.room_id).await? {
            Some(room) => room,
            None => return Ok(None),
        };

        // 获取房间成员
        let members = self.member_repository.find_by_room_id(query.room_id).await?;
        let member_dtos: Vec<RoomMemberDto> = members
            .into_iter()
            .map(|m| self.member_to_dto(m))
            .collect();

        // 获取最近消息
        let recent_messages = self.message_repository
            .find_by_room_id(query.room_id, Some(10))
            .await?;
        let message_dtos: Vec<MessageDto> = recent_messages
            .into_iter()
            .map(|m| self.message_to_dto(m))
            .collect();

        let detail = ChatRoomDetailDto {
            room: self.room_to_dto(room),
            members: member_dtos,
            recent_messages: message_dtos,
            settings: RoomSettingsDto {
                allow_guests: true,
                message_retention_days: None,
                max_message_length: 10000,
                file_upload_enabled: false,
                announcement: None,
            },
        };

        Ok(Some(detail))
    }
}