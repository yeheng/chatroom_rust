//! 机器人消息系统
//!
//! 支持自动化消息、系统通知和智能机器人功能

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 机器人实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bot {
    /// 机器人ID
    pub id: Uuid,
    /// 机器人名称
    pub name: String,
    /// 机器人描述
    pub description: Option<String>,
    /// 机器人类型
    pub bot_type: BotType,
    /// 机器人状态
    pub status: BotStatus,
    /// 创建者ID
    pub created_by: Uuid,
    /// 所属组织ID（可选）
    pub organization_id: Option<Uuid>,
    /// 机器人配置
    pub config: BotConfig,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 机器人类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotType {
    /// 系统机器人（系统通知、警告等）
    System,
    /// 聊天机器人（互动式对话）
    Chat,
    /// 通知机器人（定时通知、提醒）
    Notification,
    /// 监控机器人（系统监控、状态报告）
    Monitor,
    /// 工作流机器人（自动化任务）
    Workflow,
    /// 自定义机器人
    Custom(String),
}

impl BotType {
    pub fn as_str(&self) -> String {
        match self {
            BotType::System => "system".to_string(),
            BotType::Chat => "chat".to_string(),
            BotType::Notification => "notification".to_string(),
            BotType::Monitor => "monitor".to_string(),
            BotType::Workflow => "workflow".to_string(),
            BotType::Custom(name) => name.clone(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "system" => BotType::System,
            "chat" => BotType::Chat,
            "notification" => BotType::Notification,
            "monitor" => BotType::Monitor,
            "workflow" => BotType::Workflow,
            _ => BotType::Custom(s.to_string()),
        }
    }
}

/// 机器人状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotStatus {
    /// 活跃
    Active,
    /// 暂停
    Paused,
    /// 已禁用
    Disabled,
    /// 维护中
    Maintenance,
    /// 已删除
    Deleted,
}

impl BotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotStatus::Active => "active",
            BotStatus::Paused => "paused",
            BotStatus::Disabled => "disabled",
            BotStatus::Maintenance => "maintenance",
            BotStatus::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(BotStatus::Active),
            "paused" => Some(BotStatus::Paused),
            "disabled" => Some(BotStatus::Disabled),
            "maintenance" => Some(BotStatus::Maintenance),
            "deleted" => Some(BotStatus::Deleted),
            _ => None,
        }
    }
}

/// 机器人配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotConfig {
    /// 头像URL
    pub avatar_url: Option<String>,
    /// 自动回复配置
    pub auto_reply: AutoReplyConfig,
    /// 触发器配置
    pub triggers: Vec<BotTrigger>,
    /// 限流配置
    pub rate_limit: RateLimitConfig,
    /// 权限配置
    pub permissions: BotPermissions,
    /// 扩展配置
    pub extensions: HashMap<String, String>,
}

/// 自动回复配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoReplyConfig {
    /// 是否启用自动回复
    pub enabled: bool,
    /// 默认回复消息
    pub default_reply: Option<String>,
    /// 关键词回复映射
    pub keyword_replies: HashMap<String, String>,
    /// 回复延迟（秒）
    pub delay_seconds: u32,
}

/// 机器人触发器
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotTrigger {
    /// 触发器ID
    pub id: Uuid,
    /// 触发器名称
    pub name: String,
    /// 触发器类型
    pub trigger_type: TriggerType,
    /// 触发条件
    pub conditions: Vec<TriggerCondition>,
    /// 触发动作
    pub actions: Vec<BotAction>,
    /// 是否启用
    pub enabled: bool,
}

/// 触发器类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriggerType {
    /// 关键词触发
    Keyword,
    /// 定时触发
    Schedule,
    /// 事件触发
    Event,
    /// 用户进入房间
    UserJoin,
    /// 用户离开房间
    UserLeave,
    /// 消息计数触发
    MessageCount,
    /// 自定义触发
    Custom(String),
}

/// 触发条件
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerCondition {
    /// 条件类型
    pub condition_type: String,
    /// 条件值
    pub value: String,
    /// 操作符（equals, contains, regex等）
    pub operator: String,
}

/// 机器人动作
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotAction {
    /// 动作类型
    pub action_type: ActionType,
    /// 动作参数
    pub parameters: HashMap<String, String>,
    /// 延迟执行时间（秒）
    pub delay_seconds: Option<u32>,
}

/// 动作类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    /// 发送消息
    SendMessage,
    /// 发送私信
    SendPrivateMessage,
    /// 创建聊天室
    CreateRoom,
    /// 邀请用户
    InviteUser,
    /// 踢出用户
    KickUser,
    /// 发送通知
    SendNotification,
    /// 调用API
    CallApi,
    /// 执行脚本
    ExecuteScript,
    /// 自定义动作
    Custom(String),
}

impl ActionType {
    pub fn as_str(&self) -> String {
        match self {
            ActionType::SendMessage => "send_message".to_string(),
            ActionType::SendPrivateMessage => "send_private_message".to_string(),
            ActionType::CreateRoom => "create_room".to_string(),
            ActionType::InviteUser => "invite_user".to_string(),
            ActionType::KickUser => "kick_user".to_string(),
            ActionType::SendNotification => "send_notification".to_string(),
            ActionType::CallApi => "call_api".to_string(),
            ActionType::ExecuteScript => "execute_script".to_string(),
            ActionType::Custom(action) => action.clone(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "send_message" => ActionType::SendMessage,
            "send_private_message" => ActionType::SendPrivateMessage,
            "create_room" => ActionType::CreateRoom,
            "invite_user" => ActionType::InviteUser,
            "kick_user" => ActionType::KickUser,
            "send_notification" => ActionType::SendNotification,
            "call_api" => ActionType::CallApi,
            "execute_script" => ActionType::ExecuteScript,
            _ => ActionType::Custom(s.to_string()),
        }
    }
}

/// 限流配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// 每分钟最大消息数
    pub max_messages_per_minute: u32,
    /// 每小时最大消息数
    pub max_messages_per_hour: u32,
    /// 每天最大消息数
    pub max_messages_per_day: u32,
    /// 是否启用限流
    pub enabled: bool,
}

/// 机器人权限
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotPermissions {
    /// 可以发送消息的房间列表（空表示所有房间）
    pub allowed_rooms: Vec<Uuid>,
    /// 禁止发送消息的房间列表
    pub blocked_rooms: Vec<Uuid>,
    /// 可以操作的用户类型
    pub user_permissions: UserPermissionLevel,
    /// 可以执行的系统操作
    pub system_permissions: Vec<String>,
}

/// 用户权限级别
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserPermissionLevel {
    /// 只能与普通用户交互
    Normal,
    /// 可以与管理员交互
    Admin,
    /// 可以与所有用户交互
    All,
}

/// 机器人消息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotMessage {
    /// 消息ID
    pub id: Uuid,
    /// 机器人ID
    pub bot_id: Uuid,
    /// 消息内容
    pub content: String,
    /// 消息类型
    pub message_type: BotMessageType,
    /// 目标房间ID（可选）
    pub room_id: Option<Uuid>,
    /// 目标用户ID（私信时使用）
    pub target_user_id: Option<Uuid>,
    /// 消息状态
    pub status: BotMessageStatus,
    /// 发送时间
    pub sent_at: Option<DateTime<Utc>>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 重试次数
    pub retry_count: u32,
    /// 错误信息
    pub error_message: Option<String>,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 机器人消息类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotMessageType {
    /// 文本消息
    Text,
    /// 通知消息
    Notification,
    /// 警告消息
    Warning,
    /// 错误消息
    Error,
    /// 系统消息
    System,
    /// 富文本消息
    RichText,
    /// 图片消息
    Image,
    /// 文件消息
    File,
}

impl BotMessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotMessageType::Text => "text",
            BotMessageType::Notification => "notification",
            BotMessageType::Warning => "warning",
            BotMessageType::Error => "error",
            BotMessageType::System => "system",
            BotMessageType::RichText => "rich_text",
            BotMessageType::Image => "image",
            BotMessageType::File => "file",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "text" => Some(BotMessageType::Text),
            "notification" => Some(BotMessageType::Notification),
            "warning" => Some(BotMessageType::Warning),
            "error" => Some(BotMessageType::Error),
            "system" => Some(BotMessageType::System),
            "rich_text" => Some(BotMessageType::RichText),
            "image" => Some(BotMessageType::Image),
            "file" => Some(BotMessageType::File),
            _ => None,
        }
    }
}

/// 机器人消息状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotMessageStatus {
    /// 等待发送
    Pending,
    /// 正在发送
    Sending,
    /// 已发送
    Sent,
    /// 发送失败
    Failed,
    /// 已取消
    Cancelled,
}

impl BotMessageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotMessageStatus::Pending => "pending",
            BotMessageStatus::Sending => "sending",
            BotMessageStatus::Sent => "sent",
            BotMessageStatus::Failed => "failed",
            BotMessageStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(BotMessageStatus::Pending),
            "sending" => Some(BotMessageStatus::Sending),
            "sent" => Some(BotMessageStatus::Sent),
            "failed" => Some(BotMessageStatus::Failed),
            "cancelled" => Some(BotMessageStatus::Cancelled),
            _ => None,
        }
    }
}

/// 机器人活动记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BotActivity {
    /// 活动ID
    pub id: Uuid,
    /// 机器人ID
    pub bot_id: Uuid,
    /// 活动类型
    pub activity_type: BotActivityType,
    /// 活动描述
    pub description: String,
    /// 相关房间ID
    pub room_id: Option<Uuid>,
    /// 相关用户ID
    pub user_id: Option<Uuid>,
    /// 活动时间
    pub occurred_at: DateTime<Utc>,
    /// 活动结果
    pub result: BotActivityResult,
    /// 活动数据
    pub data: HashMap<String, String>,
}

/// 机器人活动类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotActivityType {
    /// 消息发送
    MessageSent,
    /// 触发器激活
    TriggerActivated,
    /// 用户交互
    UserInteraction,
    /// 系统事件处理
    SystemEventHandled,
    /// 错误处理
    ErrorHandling,
    /// 配置更新
    ConfigUpdated,
}

/// 机器人活动结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BotActivityResult {
    /// 成功
    Success,
    /// 失败
    Failed,
    /// 部分成功
    PartialSuccess,
    /// 跳过
    Skipped,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            avatar_url: None,
            auto_reply: AutoReplyConfig::default(),
            triggers: Vec::new(),
            rate_limit: RateLimitConfig::default(),
            permissions: BotPermissions::default(),
            extensions: HashMap::new(),
        }
    }
}

impl Default for AutoReplyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_reply: None,
            keyword_replies: HashMap::new(),
            delay_seconds: 0,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_messages_per_minute: 10,
            max_messages_per_hour: 100,
            max_messages_per_day: 1000,
            enabled: true,
        }
    }
}

impl Default for BotPermissions {
    fn default() -> Self {
        Self {
            allowed_rooms: Vec::new(),
            blocked_rooms: Vec::new(),
            user_permissions: UserPermissionLevel::Normal,
            system_permissions: Vec::new(),
        }
    }
}

impl Bot {
    /// 创建新的机器人
    pub fn new(
        name: String,
        description: Option<String>,
        bot_type: BotType,
        created_by: Uuid,
        organization_id: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            bot_type,
            status: BotStatus::Active,
            created_by,
            organization_id,
            config: BotConfig::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 检查机器人是否可以发送消息到指定房间
    pub fn can_send_to_room(&self, room_id: Uuid) -> bool {
        if !matches!(self.status, BotStatus::Active) {
            return false;
        }

        // 检查是否在禁止列表中
        if self.config.permissions.blocked_rooms.contains(&room_id) {
            return false;
        }

        // 如果允许列表为空，表示可以发送到所有房间
        if self.config.permissions.allowed_rooms.is_empty() {
            return true;
        }

        // 检查是否在允许列表中
        self.config.permissions.allowed_rooms.contains(&room_id)
    }

    /// 更新机器人配置
    pub fn update_config(&mut self, config: BotConfig) {
        self.config = config;
        self.updated_at = Utc::now();
    }

    /// 启用机器人
    pub fn enable(&mut self) {
        self.status = BotStatus::Active;
        self.updated_at = Utc::now();
    }

    /// 暂停机器人
    pub fn pause(&mut self) {
        self.status = BotStatus::Paused;
        self.updated_at = Utc::now();
    }

    /// 禁用机器人
    pub fn disable(&mut self) {
        self.status = BotStatus::Disabled;
        self.updated_at = Utc::now();
    }

    /// 检查机器人是否活跃
    pub fn is_active(&self) -> bool {
        matches!(self.status, BotStatus::Active)
    }

    /// 添加触发器
    pub fn add_trigger(&mut self, trigger: BotTrigger) {
        self.config.triggers.push(trigger);
        self.updated_at = Utc::now();
    }

    /// 移除触发器
    pub fn remove_trigger(&mut self, trigger_id: Uuid) {
        self.config.triggers.retain(|t| t.id != trigger_id);
        self.updated_at = Utc::now();
    }
}

impl BotMessage {
    /// 创建新的机器人消息
    pub fn new(
        bot_id: Uuid,
        content: String,
        message_type: BotMessageType,
        room_id: Option<Uuid>,
        target_user_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            bot_id,
            content,
            message_type,
            room_id,
            target_user_id,
            status: BotMessageStatus::Pending,
            sent_at: None,
            created_at: Utc::now(),
            retry_count: 0,
            error_message: None,
            metadata: HashMap::new(),
        }
    }

    /// 标记消息为发送中
    pub fn mark_sending(&mut self) {
        self.status = BotMessageStatus::Sending;
    }

    /// 标记消息为已发送
    pub fn mark_sent(&mut self) {
        self.status = BotMessageStatus::Sent;
        self.sent_at = Some(Utc::now());
    }

    /// 标记消息为发送失败
    pub fn mark_failed(&mut self, error_message: String) {
        self.status = BotMessageStatus::Failed;
        self.error_message = Some(error_message);
        self.retry_count += 1;
    }

    /// 检查是否可以重试
    pub fn can_retry(&self, max_retries: u32) -> bool {
        matches!(self.status, BotMessageStatus::Failed) && self.retry_count < max_retries
    }

    /// 重置为待发送状态
    pub fn reset_for_retry(&mut self) {
        self.status = BotMessageStatus::Pending;
        self.error_message = None;
    }
}

impl BotTrigger {
    /// 创建新的触发器
    pub fn new(
        name: String,
        trigger_type: TriggerType,
        conditions: Vec<TriggerCondition>,
        actions: Vec<BotAction>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            trigger_type,
            conditions,
            actions,
            enabled: true,
        }
    }

    /// 检查触发器是否匹配条件
    pub fn matches(&self, context: &TriggerContext) -> bool {
        if !self.enabled {
            return false;
        }

        // 所有条件都必须满足
        self.conditions.iter().all(|condition| {
            context.check_condition(
                &condition.condition_type,
                &condition.value,
                &condition.operator,
            )
        })
    }
}

/// 触发器上下文
pub struct TriggerContext {
    /// 消息内容（如果有）
    pub message_content: Option<String>,
    /// 用户ID
    pub user_id: Option<Uuid>,
    /// 房间ID
    pub room_id: Option<Uuid>,
    /// 事件类型
    pub event_type: Option<String>,
    /// 扩展数据
    pub extra_data: HashMap<String, String>,
}

impl TriggerContext {
    /// 检查条件是否满足
    pub fn check_condition(&self, condition_type: &str, value: &str, operator: &str) -> bool {
        match condition_type {
            "message_content" => {
                if let Some(content) = &self.message_content {
                    match operator {
                        "equals" => content == value,
                        "contains" => content.contains(value),
                        "starts_with" => content.starts_with(value),
                        "ends_with" => content.ends_with(value),
                        _ => false,
                    }
                } else {
                    false
                }
            }
            "user_id" => {
                if let Some(user_id) = self.user_id {
                    user_id.to_string() == value
                } else {
                    false
                }
            }
            "room_id" => {
                if let Some(room_id) = self.room_id {
                    room_id.to_string() == value
                } else {
                    false
                }
            }
            "event_type" => {
                if let Some(event_type) = &self.event_type {
                    event_type == value
                } else {
                    false
                }
            }
            _ => {
                // 检查扩展数据
                if let Some(data_value) = self.extra_data.get(condition_type) {
                    match operator {
                        "equals" => data_value == value,
                        "contains" => data_value.contains(value),
                        _ => false,
                    }
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_creation() {
        let bot = Bot::new(
            "Test Bot".to_string(),
            Some("A test bot".to_string()),
            BotType::Chat,
            Uuid::new_v4(),
            None,
        );

        assert_eq!(bot.name, "Test Bot");
        assert_eq!(bot.bot_type, BotType::Chat);
        assert_eq!(bot.status, BotStatus::Active);
        assert!(bot.is_active());
    }

    #[test]
    fn test_bot_room_permissions() {
        let mut bot = Bot::new(
            "Test Bot".to_string(),
            None,
            BotType::System,
            Uuid::new_v4(),
            None,
        );

        let room1 = Uuid::new_v4();
        let room2 = Uuid::new_v4();

        // 默认情况下可以发送到所有房间
        assert!(bot.can_send_to_room(room1));
        assert!(bot.can_send_to_room(room2));

        // 添加到禁止列表
        bot.config.permissions.blocked_rooms.push(room1);
        assert!(!bot.can_send_to_room(room1));
        assert!(bot.can_send_to_room(room2));

        // 设置允许列表
        bot.config.permissions.allowed_rooms.push(room2);
        assert!(!bot.can_send_to_room(room1)); // 仍然被禁止
        assert!(bot.can_send_to_room(room2)); // 在允许列表中
    }

    #[test]
    fn test_bot_message_lifecycle() {
        let bot_id = Uuid::new_v4();
        let room_id = Some(Uuid::new_v4());
        let mut message = BotMessage::new(
            bot_id,
            "Hello, world!".to_string(),
            BotMessageType::Text,
            room_id,
            None,
        );

        assert_eq!(message.status, BotMessageStatus::Pending);
        assert!(message.sent_at.is_none());

        // 标记为发送中
        message.mark_sending();
        assert_eq!(message.status, BotMessageStatus::Sending);

        // 标记为已发送
        message.mark_sent();
        assert_eq!(message.status, BotMessageStatus::Sent);
        assert!(message.sent_at.is_some());

        // 创建另一个失败的消息
        let mut failed_message = BotMessage::new(
            bot_id,
            "Failed message".to_string(),
            BotMessageType::Text,
            room_id,
            None,
        );

        failed_message.mark_failed("Network error".to_string());
        assert_eq!(failed_message.status, BotMessageStatus::Failed);
        assert_eq!(failed_message.retry_count, 1);
        assert!(failed_message.can_retry(3));
    }

    #[test]
    fn test_trigger_matching() {
        let conditions = vec![TriggerCondition {
            condition_type: "message_content".to_string(),
            value: "hello".to_string(),
            operator: "contains".to_string(),
        }];

        let actions = vec![BotAction {
            action_type: ActionType::SendMessage,
            parameters: HashMap::new(),
            delay_seconds: None,
        }];

        let trigger = BotTrigger::new(
            "Hello Trigger".to_string(),
            TriggerType::Keyword,
            conditions,
            actions,
        );

        // 匹配的上下文
        let matching_context = TriggerContext {
            message_content: Some("hello world".to_string()),
            user_id: None,
            room_id: None,
            event_type: None,
            extra_data: HashMap::new(),
        };

        assert!(trigger.matches(&matching_context));

        // 不匹配的上下文
        let non_matching_context = TriggerContext {
            message_content: Some("goodbye world".to_string()),
            user_id: None,
            room_id: None,
            event_type: None,
            extra_data: HashMap::new(),
        };

        assert!(!trigger.matches(&non_matching_context));
    }
}
