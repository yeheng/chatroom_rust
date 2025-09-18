//! 通知实体定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// 通知实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// 通知ID
    pub id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 通知类型
    pub notification_type: String,
    /// 通知标题
    pub title: String,
    /// 通知内容
    pub content: String,
    /// 优先级 (low, normal, high, urgent)
    pub priority: String,
    /// 是否已读
    pub is_read: bool,
    /// 是否已忽略
    pub is_dismissed: bool,
    /// 元数据
    pub metadata: JsonValue,
    /// 过期时间
    pub expires_at: Option<DateTime<Utc>>,
    /// 模板ID
    pub template_id: Option<Uuid>,
    /// 来源ID
    pub source_id: Option<Uuid>,
    /// 分组键
    pub group_key: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 阅读时间
    pub read_at: Option<DateTime<Utc>>,
    /// 忽略时间
    pub dismissed_at: Option<DateTime<Utc>>,
}

/// 通知优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl std::fmt::Display for NotificationPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotificationPriority::Low => write!(f, "low"),
            NotificationPriority::Normal => write!(f, "normal"),
            NotificationPriority::High => write!(f, "high"),
            NotificationPriority::Urgent => write!(f, "urgent"),
        }
    }
}

impl From<&str> for NotificationPriority {
    fn from(s: &str) -> Self {
        match s {
            "low" => NotificationPriority::Low,
            "high" => NotificationPriority::High,
            "urgent" => NotificationPriority::Urgent,
            _ => NotificationPriority::Normal,
        }
    }
}

/// 通知类型常量
pub mod notification_types {
    pub const MESSAGE_RECEIVED: &str = "message_received";
    pub const MENTION_RECEIVED: &str = "mention_received";
    pub const ROOM_INVITE: &str = "room_invite";
    pub const ROOM_JOIN: &str = "room_join";
    pub const ROOM_LEAVE: &str = "room_leave";
    pub const SYSTEM_NOTICE: &str = "system_notice";
    pub const FRIEND_REQUEST: &str = "friend_request";
    pub const FILE_SHARED: &str = "file_shared";
    pub const REMINDER: &str = "reminder";
    pub const ANNOUNCEMENT: &str = "announcement";
}

impl Notification {
    /// 创建新通知
    pub fn new(
        user_id: Uuid,
        notification_type: String,
        title: String,
        content: String,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            user_id,
            notification_type,
            title,
            content,
            priority: NotificationPriority::Normal.to_string(),
            is_read: false,
            is_dismissed: false,
            metadata: JsonValue::Object(serde_json::Map::new()),
            expires_at: None,
            template_id: None,
            source_id: None,
            group_key: None,
            created_at: now,
            read_at: None,
            dismissed_at: None,
        }
    }

    /// 创建带ID的通知
    pub fn with_id(
        id: Uuid,
        user_id: Uuid,
        notification_type: String,
        title: String,
        content: String,
        priority: String,
        is_read: bool,
        is_dismissed: bool,
        metadata: JsonValue,
        expires_at: Option<DateTime<Utc>>,
        template_id: Option<Uuid>,
        source_id: Option<Uuid>,
        group_key: Option<String>,
        created_at: DateTime<Utc>,
        read_at: Option<DateTime<Utc>>,
        dismissed_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            user_id,
            notification_type,
            title,
            content,
            priority,
            is_read,
            is_dismissed,
            metadata,
            expires_at,
            template_id,
            source_id,
            group_key,
            created_at,
            read_at,
            dismissed_at,
        }
    }

    /// 设置优先级
    pub fn set_priority(&mut self, priority: NotificationPriority) {
        self.priority = priority.to_string();
    }

    /// 获取优先级
    pub fn priority(&self) -> NotificationPriority {
        NotificationPriority::from(self.priority.as_str())
    }

    /// 标记为已读
    pub fn mark_as_read(&mut self) {
        if !self.is_read {
            self.is_read = true;
            self.read_at = Some(Utc::now());
        }
    }

    /// 标记为未读
    pub fn mark_as_unread(&mut self) {
        self.is_read = false;
        self.read_at = None;
    }

    /// 忽略通知
    pub fn dismiss(&mut self) {
        if !self.is_dismissed {
            self.is_dismissed = true;
            self.dismissed_at = Some(Utc::now());
        }
    }

    /// 取消忽略
    pub fn undismiss(&mut self) {
        self.is_dismissed = false;
        self.dismissed_at = None;
    }

    /// 设置过期时间
    pub fn set_expiry(&mut self, expires_at: DateTime<Utc>) {
        self.expires_at = Some(expires_at);
    }

    /// 设置元数据
    pub fn set_metadata(&mut self, metadata: JsonValue) {
        self.metadata = metadata;
    }

    /// 添加元数据字段
    pub fn add_metadata_field(&mut self, key: String, value: JsonValue) {
        if let JsonValue::Object(ref mut map) = self.metadata {
            map.insert(key, value);
        }
    }

    /// 设置分组键
    pub fn set_group_key(&mut self, group_key: String) {
        self.group_key = Some(group_key);
    }

    /// 设置来源ID
    pub fn set_source_id(&mut self, source_id: Uuid) {
        self.source_id = Some(source_id);
    }

    /// 检查是否已过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }

    /// 检查是否为高优先级
    pub fn is_high_priority(&self) -> bool {
        matches!(
            self.priority(),
            NotificationPriority::High | NotificationPriority::Urgent
        )
    }

    /// 检查是否需要立即处理
    pub fn requires_immediate_attention(&self) -> bool {
        self.priority() == NotificationPriority::Urgent && !self.is_read && !self.is_dismissed
    }
}