//! 消息实体定义
//!
//! 包含消息的核心信息和相关操作。

use crate::errors::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 消息类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// 文本消息
    Text,
    /// 图片消息
    Image {
        url: String,
        thumbnail: Option<String>,
    },
    /// 文件消息
    File {
        url: String,
        filename: String,
        size: u64,
    },
    /// 表情消息
    Emoji { emoji_id: String },
}

impl Default for MessageType {
    fn default() -> Self {
        Self::Text
    }
}

/// 消息状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    /// 已发送
    Sent,
    /// 已送达
    Delivered,
    /// 已读
    Read,
    /// 已删除
    Deleted,
    /// 被撤回
    Recalled,
}

impl Default for MessageStatus {
    fn default() -> Self {
        Self::Sent
    }
}

/// 消息附件信息
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageAttachment {
    /// 文件名
    pub filename: String,
    /// 文件URL
    pub url: String,
    /// 文件大小（字节）
    pub size: u64,
    /// MIME类型
    pub mime_type: String,
    /// 缩略图URL（可选，用于图片和视频）
    pub thumbnail_url: Option<String>,
}

impl MessageAttachment {
    /// 创建新的消息附件
    pub fn new(
        filename: impl Into<String>,
        url: impl Into<String>,
        size: u64,
        mime_type: impl Into<String>,
        thumbnail_url: Option<String>,
    ) -> DomainResult<Self> {
        let filename = filename.into();
        let url = url.into();
        let mime_type = mime_type.into();

        if filename.is_empty() {
            return Err(DomainError::validation_error("filename", "文件名不能为空"));
        }

        if url.is_empty() {
            return Err(DomainError::validation_error("url", "文件URL不能为空"));
        }

        if mime_type.is_empty() {
            return Err(DomainError::validation_error(
                "mime_type",
                "MIME类型不能为空",
            ));
        }

        // 验证文件大小限制（100MB）
        if size > 100 * 1024 * 1024 {
            return Err(DomainError::validation_error(
                "size",
                "文件大小不能超过100MB",
            ));
        }

        Ok(Self {
            filename,
            url,
            size,
            mime_type,
            thumbnail_url,
        })
    }
}

/// 消息实体
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// 消息唯一ID
    pub id: Uuid,
    /// 聊天室ID
    pub room_id: Uuid,
    /// 发送者ID
    pub sender_id: Uuid,
    /// 消息类型
    pub message_type: MessageType,
    /// 消息内容
    pub content: String,
    /// 回复的消息ID（可选）
    pub reply_to_id: Option<Uuid>,
    /// 是否为机器人消息
    pub is_bot_message: bool,
    /// 消息状态
    pub status: MessageStatus,
    /// 发送时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: Option<DateTime<Utc>>,
}

impl Message {
    /// 创建新的文本消息
    pub fn new_text(room_id: Uuid, user_id: Uuid, content: String) -> DomainResult<Self> {
        Self::validate_content(&content)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id: user_id,
            message_type: MessageType::Text,
            content,
            reply_to_id: None,
            is_bot_message: false,
            status: MessageStatus::Sent,
            created_at: now,
            updated_at: None,
        })
    }

    /// 创建新的回复消息
    pub fn new_reply(
        room_id: Uuid,
        user_id: Uuid,
        content: String,
        reply_to_id: Uuid,
    ) -> DomainResult<Self> {
        Self::validate_content(&content)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id: user_id,
            message_type: MessageType::Text,
            content,
            reply_to_id: Some(reply_to_id),
            is_bot_message: false,
            status: MessageStatus::Sent,
            created_at: now,
            updated_at: None,
        })
    }

    /// 创建新的机器人消息
    pub fn new_bot_message(room_id: Uuid, user_id: Uuid, content: String) -> DomainResult<Self> {
        Self::validate_content(&content)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id: user_id,
            message_type: MessageType::Text,
            content,
            reply_to_id: None,
            is_bot_message: true,
            status: MessageStatus::Sent,
            created_at: now,
            updated_at: None,
        })
    }

    /// 创建新的图片消息
    pub fn new_image(
        room_id: Uuid,
        user_id: Uuid,
        content: String,
        url: String,
        thumbnail: Option<String>,
    ) -> DomainResult<Self> {
        Self::validate_content(&content)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id: user_id,
            message_type: MessageType::Image { url, thumbnail },
            content,
            reply_to_id: None,
            is_bot_message: false,
            status: MessageStatus::Sent,
            created_at: now,
            updated_at: None,
        })
    }

    /// 创建新的文件消息
    pub fn new_file(
        room_id: Uuid,
        user_id: Uuid,
        content: String,
        url: String,
        filename: String,
        size: u64,
    ) -> DomainResult<Self> {
        Self::validate_content(&content)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id: user_id,
            message_type: MessageType::File {
                url,
                filename,
                size,
            },
            content,
            reply_to_id: None,
            is_bot_message: false,
            status: MessageStatus::Sent,
            created_at: now,
            updated_at: None,
        })
    }

    /// 更新消息内容
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = Some(Utc::now());
    }

    /// 创建具有指定ID的消息（用于从数据库加载）
    pub fn with_id(
        id: Uuid,
        room_id: Uuid,
        sender_id: Uuid,
        message_type: MessageType,
        content: impl Into<String>,
        reply_to_id: Option<Uuid>,
        is_bot_message: bool,
        status: MessageStatus,
        created_at: DateTime<Utc>,
        updated_at: Option<DateTime<Utc>>,
    ) -> DomainResult<Self> {
        let content = content.into();

        // 验证消息内容
        Self::validate_content(&content)?;

        Ok(Self {
            id,
            room_id,
            sender_id,
            message_type,
            content,
            reply_to_id,
            is_bot_message,
            status,
            created_at,
            updated_at,
        })
    }

    /// 编辑消息内容
    pub fn edit_content(&mut self, new_content: impl Into<String>) -> DomainResult<()> {
        // 只有文本消息可以编辑
        if self.message_type != MessageType::Text {
            return Err(DomainError::business_rule_violation("只有文本消息可以编辑"));
        }

        // 已删除或撤回的消息不能编辑
        if matches!(
            self.status,
            MessageStatus::Deleted | MessageStatus::Recalled
        ) {
            return Err(DomainError::business_rule_violation(
                "已删除或撤回的消息不能编辑",
            ));
        }

        let new_content = new_content.into();
        Self::validate_content(&new_content)?;

        self.content = new_content;
        self.updated_at = Some(Utc::now());

        Ok(())
    }

    /// 更新消息状态
    pub fn update_status(&mut self, new_status: MessageStatus) {
        self.status = new_status;
        self.updated_at = Some(Utc::now());
    }

    /// 撤回消息
    pub fn recall(&mut self) -> DomainResult<()> {
        // 已删除的消息不能撤回
        if self.status == MessageStatus::Deleted {
            return Err(DomainError::business_rule_violation("已删除的消息不能撤回"));
        }

        self.status = MessageStatus::Recalled;
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    /// 软删除消息
    pub fn soft_delete(&mut self) -> DomainResult<()> {
        self.status = MessageStatus::Deleted;
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    /// 检查消息是否可见
    pub fn is_visible(&self) -> bool {
        !matches!(
            self.status,
            MessageStatus::Deleted | MessageStatus::Recalled
        )
    }

    /// 检查消息是否可编辑
    pub fn is_editable(&self) -> bool {
        self.message_type == MessageType::Text
            && !matches!(
                self.status,
                MessageStatus::Deleted | MessageStatus::Recalled
            )
    }

    /// 检查是否为机器人消息
    pub fn is_bot_message(&self) -> bool {
        self.is_bot_message
    }

    /// 检查是否为回复消息
    pub fn is_reply(&self) -> bool {
        self.reply_to_id.is_some()
    }

    /// 获取消息的简短预览（用于通知等）
    pub fn get_preview(&self, max_length: usize) -> String {
        if self.content.len() <= max_length {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_length])
        }
    }

    /// 验证消息内容
    fn validate_content(content: &str) -> DomainResult<()> {
        if content.trim().is_empty() {
            return Err(DomainError::validation_error("content", "消息内容不能为空"));
        }

        if content.len() > 10000 {
            return Err(DomainError::validation_error(
                "content",
                "消息内容不能超过10000个字符",
            ));
        }

        // 检查敏感词（简化版）
        if content.contains("敏感词") {
            return Err(DomainError::validation_error("content", "消息包含敏感内容"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_message_creation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message = Message::new_text(room_id, sender_id, "Hello World".to_string()).unwrap();

        assert_eq!(message.room_id, room_id);
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.message_type, MessageType::Text);
        assert_eq!(message.content, "Hello World");
        assert!(message.reply_to_id.is_none());
        assert_eq!(message.status, MessageStatus::Sent);
        assert!(!message.is_bot_message);
    }

    #[test]
    fn test_image_message_creation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message = Message::new_image(
            room_id,
            sender_id,
            "Check this image".to_string(),
            "https://example.com/test.jpg".to_string(),
            Some("https://example.com/thumb.jpg".to_string()),
        )
        .unwrap();

        assert_eq!(message.room_id, room_id);
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.content, "Check this image");
        if let MessageType::Image { url, thumbnail } = &message.message_type {
            assert_eq!(url, "https://example.com/test.jpg");
            assert_eq!(
                thumbnail,
                &Some("https://example.com/thumb.jpg".to_string())
            );
        } else {
            panic!("Expected Image message type");
        }
    }

    #[test]
    fn test_file_message_creation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message = Message::new_file(
            room_id,
            sender_id,
            "Here's the document".to_string(),
            "https://example.com/document.pdf".to_string(),
            "document.pdf".to_string(),
            2048,
        )
        .unwrap();

        assert_eq!(message.room_id, room_id);
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.content, "Here's the document");
        if let MessageType::File {
            url,
            filename,
            size,
        } = &message.message_type
        {
            assert_eq!(url, "https://example.com/document.pdf");
            assert_eq!(filename, "document.pdf");
            assert_eq!(*size, 2048);
        } else {
            panic!("Expected File message type");
        }
    }

    #[test]
    fn test_bot_message_creation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4(); // 机器人也有用户ID
        let message =
            Message::new_bot_message(room_id, sender_id, "Welcome to the room!".to_string())
                .unwrap();

        assert_eq!(message.room_id, room_id);
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.message_type, MessageType::Text);
        assert_eq!(message.content, "Welcome to the room!");
        assert!(message.is_bot_message);
        assert!(message.is_bot_message());
    }

    #[test]
    fn test_message_content_validation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();

        // 有效内容
        assert!(Message::new_text(room_id, sender_id, "Valid message".to_string()).is_ok());
        assert!(Message::new_text(room_id, sender_id, "A".repeat(1000)).is_ok());

        // 无效内容
        assert!(Message::new_text(room_id, sender_id, "".to_string()).is_err());
        assert!(Message::new_text(room_id, sender_id, "A".repeat(10001)).is_err());
    }

    #[test]
    fn test_emoji_message_creation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();

        // 测试表情消息创建（使用new_text创建，然后检查类型）
        let message = Message::new_text(room_id, sender_id, "😀".to_string()).unwrap();

        assert_eq!(message.room_id, room_id);
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.content, "😀");
        assert_eq!(message.message_type, MessageType::Text);
    }

    #[test]
    fn test_message_editing() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let mut message =
            Message::new_text(room_id, sender_id, "Original content".to_string()).unwrap();

        // 测试编辑文本消息
        assert!(message.edit_content("Updated content").is_ok());
        assert_eq!(message.content, "Updated content");
        assert!(message.updated_at.is_some());

        // 测试无效编辑
        assert!(message.edit_content("").is_err());

        // 测试已删除消息的编辑
        message.soft_delete().unwrap();
        assert!(message.edit_content("New content").is_err());
    }

    #[test]
    fn test_message_status_operations() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let mut message =
            Message::new_text(room_id, sender_id, "Test message".to_string()).unwrap();

        // 测试状态更新
        message.update_status(MessageStatus::Delivered);
        assert_eq!(message.status, MessageStatus::Delivered);

        message.update_status(MessageStatus::Read);
        assert_eq!(message.status, MessageStatus::Read);

        // 测试撤回
        assert!(message.recall().is_ok());
        assert_eq!(message.status, MessageStatus::Recalled);
        assert!(!message.is_visible());

        // 测试删除
        let mut message2 =
            Message::new_text(room_id, sender_id, "Test message 2".to_string()).unwrap();
        assert!(message2.soft_delete().is_ok());
        assert_eq!(message2.status, MessageStatus::Deleted);
        assert!(!message2.is_visible());
    }

    #[test]
    fn test_reply_message() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let original_message =
            Message::new_text(room_id, sender_id, "Original message".to_string()).unwrap();
        let reply_message = Message::new_reply(
            room_id,
            sender_id,
            "Reply to original".to_string(),
            original_message.id,
        )
        .unwrap();

        assert!(reply_message.is_reply());
        assert_eq!(reply_message.reply_to_id, Some(original_message.id));
    }

    #[test]
    fn test_message_preview() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message = Message::new_text(
            room_id,
            sender_id,
            "This is a long message content".to_string(),
        )
        .unwrap();

        assert_eq!(message.get_preview(10), "This is a ...");
        assert_eq!(message.get_preview(100), "This is a long message content");
    }

    #[test]
    fn test_message_serialization() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message = Message::new_text(room_id, sender_id, "Test message".to_string()).unwrap();

        // 测试序列化
        let json = serde_json::to_string(&message).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(message, deserialized);
    }
}
