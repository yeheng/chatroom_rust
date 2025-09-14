//! 聊天室服务接口定义
//!
//! 提供聊天室相关的业务逻辑服务接口

use crate::entities::{chatroom::ChatRoom, message::Message};
use crate::errors::DomainResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 聊天室服务接口
#[async_trait]
pub trait ChatRoomService: Send + Sync {
    /// 创建聊天室
    async fn create_room(&self, request: CreateRoomRequest) -> DomainResult<ChatRoom>;

    /// 加入聊天室
    async fn join_room(
        &self,
        user_id: Uuid,
        room_id: Uuid,
        password: Option<String>,
    ) -> DomainResult<()>;

    /// 离开聊天室
    async fn leave_room(&self, user_id: Uuid, room_id: Uuid) -> DomainResult<()>;

    /// 发送消息
    async fn send_message(&self, command: SendMessageCommand) -> DomainResult<Message>;

    /// 验证管理员权限
    async fn verify_admin_permission(&self, user_id: Uuid, room_id: Uuid) -> DomainResult<bool>;

    /// 发送机器人消息
    async fn send_bot_message(
        &self,
        admin_id: Uuid,
        room_id: Uuid,
        content: String,
    ) -> DomainResult<Message>;

    /// 获取用户房间列表
    async fn get_user_rooms(&self, user_id: Uuid) -> DomainResult<Vec<ChatRoom>>;

    /// 获取房间历史消息
    async fn get_room_history(&self, query: GetRoomHistoryQuery) -> DomainResult<Vec<Message>>;
}

/// 创建聊天室请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    /// 房间名称
    pub name: String,
    /// 房间描述
    pub description: Option<String>,
    /// 房主ID
    pub owner_id: Uuid,
    /// 是否为私人房间
    pub is_private: bool,
    /// 房间密码（私人房间）
    pub password: Option<String>,
}

impl CreateRoomRequest {
    /// 创建新的聊天室创建请求
    pub fn new(
        name: String,
        description: Option<String>,
        owner_id: Uuid,
        is_private: bool,
        password: Option<String>,
    ) -> Self {
        Self {
            name,
            description,
            owner_id,
            is_private,
            password,
        }
    }
}

/// 发送消息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageCommand {
    /// 房间ID
    pub room_id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 消息内容
    pub content: String,
    /// 是否为机器人消息
    pub is_bot_message: bool,
    /// 回复的消息ID
    pub reply_to_id: Option<Uuid>,
}

impl SendMessageCommand {
    /// 创建新的发送消息命令
    pub fn new(
        room_id: Uuid,
        user_id: Uuid,
        content: String,
        is_bot_message: bool,
        reply_to_id: Option<Uuid>,
    ) -> Self {
        Self {
            room_id,
            user_id,
            content,
            is_bot_message,
            reply_to_id,
        }
    }

    /// 创建用户消息命令
    pub fn user_message(room_id: Uuid, user_id: Uuid, content: String) -> Self {
        Self::new(room_id, user_id, content, false, None)
    }

    /// 创建机器人消息命令
    pub fn bot_message(room_id: Uuid, user_id: Uuid, content: String) -> Self {
        Self::new(room_id, user_id, content, true, None)
    }

    /// 创建回复消息命令
    pub fn reply_message(room_id: Uuid, user_id: Uuid, content: String, reply_to_id: Uuid) -> Self {
        Self::new(room_id, user_id, content, false, Some(reply_to_id))
    }
}

/// 获取房间历史消息查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRoomHistoryQuery {
    /// 房间ID
    pub room_id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 页码
    pub page: u32,
    /// 页大小
    pub page_size: u32,
    /// 在此消息ID之前
    pub before_message_id: Option<Uuid>,
}

impl GetRoomHistoryQuery {
    /// 创建新的房间历史查询
    pub fn new(room_id: Uuid, user_id: Uuid, page: u32, page_size: u32) -> Self {
        Self {
            room_id,
            user_id,
            page,
            page_size,
            before_message_id: None,
        }
    }

    /// 设置分页查询的锚点消息ID
    pub fn before_message(mut self, message_id: Uuid) -> Self {
        self.before_message_id = Some(message_id);
        self
    }

    /// 验证查询参数
    pub fn validate(&self) -> DomainResult<()> {
        if self.page_size == 0 || self.page_size > 100 {
            return Err(crate::errors::DomainError::validation_error(
                "page_size",
                "页大小必须在1-100之间",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_room_request_creation() {
        let request = CreateRoomRequest::new(
            "测试房间".to_string(),
            Some("房间描述".to_string()),
            Uuid::new_v4(),
            false,
            None,
        );

        assert_eq!(request.name, "测试房间");
        assert_eq!(request.description, Some("房间描述".to_string()));
        assert!(!request.is_private);
        assert_eq!(request.password, None);
    }

    #[test]
    fn test_send_message_command_user_message() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let content = "Hello, World!".to_string();

        let command = SendMessageCommand::user_message(room_id, user_id, content.clone());

        assert_eq!(command.room_id, room_id);
        assert_eq!(command.user_id, user_id);
        assert_eq!(command.content, content);
        assert!(!command.is_bot_message);
        assert_eq!(command.reply_to_id, None);
    }

    #[test]
    fn test_send_message_command_bot_message() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let content = "I am a bot!".to_string();

        let command = SendMessageCommand::bot_message(room_id, user_id, content.clone());

        assert_eq!(command.room_id, room_id);
        assert_eq!(command.user_id, user_id);
        assert_eq!(command.content, content);
        assert!(command.is_bot_message);
        assert_eq!(command.reply_to_id, None);
    }

    #[test]
    fn test_send_message_command_reply_message() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let reply_to_id = Uuid::new_v4();
        let content = "This is a reply".to_string();

        let command =
            SendMessageCommand::reply_message(room_id, user_id, content.clone(), reply_to_id);

        assert_eq!(command.room_id, room_id);
        assert_eq!(command.user_id, user_id);
        assert_eq!(command.content, content);
        assert!(!command.is_bot_message);
        assert_eq!(command.reply_to_id, Some(reply_to_id));
    }

    #[test]
    fn test_get_room_history_query_creation() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let query = GetRoomHistoryQuery::new(room_id, user_id, 1, 20);

        assert_eq!(query.room_id, room_id);
        assert_eq!(query.user_id, user_id);
        assert_eq!(query.page, 1);
        assert_eq!(query.page_size, 20);
        assert_eq!(query.before_message_id, None);
    }

    #[test]
    fn test_get_room_history_query_before_message() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();

        let query = GetRoomHistoryQuery::new(room_id, user_id, 1, 20).before_message(message_id);

        assert_eq!(query.before_message_id, Some(message_id));
    }

    #[test]
    fn test_get_room_history_query_validation() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // 有效的查询
        let valid_query = GetRoomHistoryQuery::new(room_id, user_id, 1, 20);
        assert!(valid_query.validate().is_ok());

        // 页大小为0
        let invalid_query = GetRoomHistoryQuery::new(room_id, user_id, 1, 0);
        assert!(invalid_query.validate().is_err());

        // 页大小超过100
        let invalid_query = GetRoomHistoryQuery::new(room_id, user_id, 1, 101);
        assert!(invalid_query.validate().is_err());

        // 边界值测试
        let boundary_query = GetRoomHistoryQuery::new(room_id, user_id, 1, 100);
        assert!(boundary_query.validate().is_ok());
    }
}
