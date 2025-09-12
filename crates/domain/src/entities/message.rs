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
    Image,
    /// 文件消息
    File,
    /// 系统消息
    System,
    /// 表情消息
    Emoji,
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
    /// 消息附件（可选）
    pub attachment: Option<MessageAttachment>,
    /// 回复的消息ID（可选）
    pub reply_to_id: Option<Uuid>,
    /// 消息状态
    pub status: MessageStatus,
    /// 发送时间
    pub sent_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 编辑时间（可选）
    pub edited_at: Option<DateTime<Utc>>,
    /// 是否已编辑
    pub is_edited: bool,
}

impl Message {
    /// 创建新的文本消息
    pub fn new_text(
        room_id: Uuid,
        sender_id: Uuid,
        content: impl Into<String>,
        reply_to_id: Option<Uuid>,
    ) -> DomainResult<Self> {
        let content = content.into();
        Self::validate_text_content(&content)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id,
            message_type: MessageType::Text,
            content,
            attachment: None,
            reply_to_id,
            status: MessageStatus::Sent,
            sent_at: now,
            updated_at: now,
            edited_at: None,
            is_edited: false,
        })
    }

    /// 创建新的图片消息
    pub fn new_image(
        room_id: Uuid,
        sender_id: Uuid,
        content: impl Into<String>,
        attachment: MessageAttachment,
        reply_to_id: Option<Uuid>,
    ) -> DomainResult<Self> {
        let content = content.into();

        // 验证是否为图片类型
        if !attachment.mime_type.starts_with("image/") {
            return Err(DomainError::validation_error(
                "attachment",
                "附件必须是图片类型",
            ));
        }

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id,
            message_type: MessageType::Image,
            content,
            attachment: Some(attachment),
            reply_to_id,
            status: MessageStatus::Sent,
            sent_at: now,
            updated_at: now,
            edited_at: None,
            is_edited: false,
        })
    }

    /// 创建新的文件消息
    pub fn new_file(
        room_id: Uuid,
        sender_id: Uuid,
        content: impl Into<String>,
        attachment: MessageAttachment,
        reply_to_id: Option<Uuid>,
    ) -> DomainResult<Self> {
        let content = content.into();

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id,
            message_type: MessageType::File,
            content,
            attachment: Some(attachment),
            reply_to_id,
            status: MessageStatus::Sent,
            sent_at: now,
            updated_at: now,
            edited_at: None,
            is_edited: false,
        })
    }

    /// 创建新的系统消息
    pub fn new_system(room_id: Uuid, content: impl Into<String>) -> DomainResult<Self> {
        let content = content.into();
        Self::validate_system_content(&content)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            room_id,
            sender_id: Uuid::nil(), // 系统消息使用nil UUID
            message_type: MessageType::System,
            content,
            attachment: None,
            reply_to_id: None,
            status: MessageStatus::Sent,
            sent_at: now,
            updated_at: now,
            edited_at: None,
            is_edited: false,
        })
    }

    /// 创建具有指定ID的消息（用于从数据库加载）
    pub fn with_id(
        id: Uuid,
        room_id: Uuid,
        sender_id: Uuid,
        message_type: MessageType,
        content: impl Into<String>,
        attachment: Option<MessageAttachment>,
        reply_to_id: Option<Uuid>,
        status: MessageStatus,
        sent_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        edited_at: Option<DateTime<Utc>>,
        is_edited: bool,
    ) -> DomainResult<Self> {
        let content = content.into();

        // 验证消息内容
        match message_type {
            MessageType::Text | MessageType::Image | MessageType::File => {
                Self::validate_text_content(&content)?;
            }
            MessageType::System => {
                Self::validate_system_content(&content)?;
            }
            _ => {}
        }

        // 验证附件与消息类型的一致性
        match message_type {
            MessageType::Image | MessageType::File => {
                if attachment.is_none() {
                    return Err(DomainError::validation_error(
                        "attachment",
                        "图片和文件消息必须包含附件",
                    ));
                }
            }
            MessageType::Text | MessageType::System | MessageType::Emoji => {
                if attachment.is_some() {
                    return Err(DomainError::validation_error(
                        "attachment",
                        "文本和系统消息不应包含附件",
                    ));
                }
            }
        }

        Ok(Self {
            id,
            room_id,
            sender_id,
            message_type,
            content,
            attachment,
            reply_to_id,
            status,
            sent_at,
            updated_at,
            edited_at,
            is_edited,
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
        Self::validate_text_content(&new_content)?;

        self.content = new_content;
        self.is_edited = true;
        self.edited_at = Some(Utc::now());
        self.updated_at = Utc::now();

        Ok(())
    }

    /// 更新消息状态
    pub fn update_status(&mut self, new_status: MessageStatus) {
        self.status = new_status;
        self.updated_at = Utc::now();
    }

    /// 撤回消息
    pub fn recall(&mut self) -> DomainResult<()> {
        // 已删除的消息不能撤回
        if self.status == MessageStatus::Deleted {
            return Err(DomainError::business_rule_violation("已删除的消息不能撤回"));
        }

        // 系统消息不能撤回
        if self.message_type == MessageType::System {
            return Err(DomainError::business_rule_violation("系统消息不能撤回"));
        }

        self.status = MessageStatus::Recalled;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 软删除消息
    pub fn soft_delete(&mut self) -> DomainResult<()> {
        // 系统消息不能删除
        if self.message_type == MessageType::System {
            return Err(DomainError::business_rule_violation("系统消息不能删除"));
        }

        self.status = MessageStatus::Deleted;
        self.updated_at = Utc::now();
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

    /// 检查是否为系统消息
    pub fn is_system_message(&self) -> bool {
        self.message_type == MessageType::System
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

    /// 验证文本消息内容
    fn validate_text_content(content: &str) -> DomainResult<()> {
        if content.is_empty() {
            return Err(DomainError::validation_error("content", "消息内容不能为空"));
        }

        if content.len() > 10000 {
            return Err(DomainError::validation_error(
                "content",
                "消息内容不能超过10000个字符",
            ));
        }

        Ok(())
    }

    /// 验证系统消息内容
    fn validate_system_content(content: &str) -> DomainResult<()> {
        if content.is_empty() {
            return Err(DomainError::validation_error(
                "content",
                "系统消息内容不能为空",
            ));
        }

        if content.len() > 1000 {
            return Err(DomainError::validation_error(
                "content",
                "系统消息内容不能超过1000个字符",
            ));
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
        let message = Message::new_text(room_id, sender_id, "Hello World", None).unwrap();

        assert_eq!(message.room_id, room_id);
        assert_eq!(message.sender_id, sender_id);
        assert_eq!(message.message_type, MessageType::Text);
        assert_eq!(message.content, "Hello World");
        assert!(message.attachment.is_none());
        assert!(message.reply_to_id.is_none());
        assert_eq!(message.status, MessageStatus::Sent);
        assert!(!message.is_edited);
    }

    #[test]
    fn test_image_message_creation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let attachment = MessageAttachment::new(
            "test.jpg",
            "https://example.com/test.jpg",
            1024,
            "image/jpeg",
            Some("https://example.com/thumb.jpg".to_string()),
        )
        .unwrap();

        let message = Message::new_image(
            room_id,
            sender_id,
            "Check this image",
            attachment.clone(),
            None,
        )
        .unwrap();

        assert_eq!(message.message_type, MessageType::Image);
        assert_eq!(message.attachment, Some(attachment));
    }

    #[test]
    fn test_file_message_creation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let attachment = MessageAttachment::new(
            "document.pdf",
            "https://example.com/document.pdf",
            2048,
            "application/pdf",
            None,
        )
        .unwrap();

        let message = Message::new_file(
            room_id,
            sender_id,
            "Here's the document",
            attachment.clone(),
            None,
        )
        .unwrap();

        assert_eq!(message.message_type, MessageType::File);
        assert_eq!(message.attachment, Some(attachment));
    }

    #[test]
    fn test_system_message_creation() {
        let room_id = Uuid::new_v4();
        let message = Message::new_system(room_id, "User joined the room").unwrap();

        assert_eq!(message.room_id, room_id);
        assert_eq!(message.sender_id, Uuid::nil());
        assert_eq!(message.message_type, MessageType::System);
        assert_eq!(message.content, "User joined the room");
        assert!(message.is_system_message());
    }

    #[test]
    fn test_message_content_validation() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();

        // 有效内容
        assert!(Message::new_text(room_id, sender_id, "Valid message", None).is_ok());
        assert!(Message::new_text(room_id, sender_id, "A".repeat(1000), None).is_ok());

        // 无效内容
        assert!(Message::new_text(room_id, sender_id, "", None).is_err());
        assert!(Message::new_text(room_id, sender_id, "A".repeat(10001), None).is_err());
    }

    #[test]
    fn test_message_attachment_validation() {
        // 有效附件
        assert!(MessageAttachment::new(
            "test.jpg",
            "https://example.com/test.jpg",
            1024,
            "image/jpeg",
            None
        )
        .is_ok());

        // 无效附件
        assert!(MessageAttachment::new(
            "",
            "https://example.com/test.jpg",
            1024,
            "image/jpeg",
            None
        )
        .is_err());
        assert!(MessageAttachment::new("test.jpg", "", 1024, "image/jpeg", None).is_err());
        assert!(
            MessageAttachment::new("test.jpg", "https://example.com/test.jpg", 1024, "", None)
                .is_err()
        );
        assert!(MessageAttachment::new(
            "test.jpg",
            "https://example.com/test.jpg",
            101 * 1024 * 1024,
            "image/jpeg",
            None
        )
        .is_err());
    }

    #[test]
    fn test_message_editing() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let mut message = Message::new_text(room_id, sender_id, "Original content", None).unwrap();

        // 测试编辑文本消息
        assert!(message.edit_content("Updated content").is_ok());
        assert_eq!(message.content, "Updated content");
        assert!(message.is_edited);
        assert!(message.edited_at.is_some());

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
        let mut message = Message::new_text(room_id, sender_id, "Test message", None).unwrap();

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
        let mut message2 = Message::new_text(room_id, sender_id, "Test message 2", None).unwrap();
        assert!(message2.soft_delete().is_ok());
        assert_eq!(message2.status, MessageStatus::Deleted);
        assert!(!message2.is_visible());
    }

    #[test]
    fn test_reply_message() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let original_message =
            Message::new_text(room_id, sender_id, "Original message", None).unwrap();
        let reply_message = Message::new_text(
            room_id,
            sender_id,
            "Reply to original",
            Some(original_message.id),
        )
        .unwrap();

        assert!(reply_message.is_reply());
        assert_eq!(reply_message.reply_to_id, Some(original_message.id));
    }

    #[test]
    fn test_message_preview() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message =
            Message::new_text(room_id, sender_id, "This is a long message content", None).unwrap();

        assert_eq!(message.get_preview(10), "This is a ...");
        assert_eq!(message.get_preview(100), "This is a long message content");
    }

    #[test]
    fn test_message_serialization() {
        let room_id = Uuid::new_v4();
        let sender_id = Uuid::new_v4();
        let message = Message::new_text(room_id, sender_id, "Test message", None).unwrap();

        // 测试序列化
        let json = serde_json::to_string(&message).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(message, deserialized);
    }
}
