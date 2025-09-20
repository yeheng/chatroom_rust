//! 通知系统Repository接口定义

use crate::entities::Notification;
use crate::errors::DomainResult;
use crate::repositories::{PaginatedResult, Pagination, SortConfig};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// 通知设置实体
#[derive(Debug, Clone)]
pub struct NotificationSettings {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub is_enabled: bool,
    pub push_enabled: bool,
    pub email_enabled: bool,
    pub sound_enabled: bool,
    pub priority_threshold: String,
    pub quiet_hours_start: Option<chrono::NaiveTime>,
    pub quiet_hours_end: Option<chrono::NaiveTime>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 通知模板实体
#[derive(Debug, Clone)]
pub struct NotificationTemplate {
    pub id: Uuid,
    pub notification_type: String,
    pub title_template: String,
    pub message_template: String,
    pub default_priority: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 通知发送日志实体
#[derive(Debug, Clone)]
pub struct NotificationDeliveryLog {
    pub id: Uuid,
    pub notification_id: Uuid,
    pub delivery_method: String, // push, email, websocket, sms
    pub status: String,          // pending, sent, failed, delivered, read
    pub error_message: Option<String>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// 通知统计信息
#[derive(Debug, Clone)]
pub struct NotificationStatistics {
    pub total_notifications: u64,
    pub unread_notifications: u64,
    pub notifications_by_type: std::collections::HashMap<String, u64>,
    pub notifications_by_priority: std::collections::HashMap<String, u64>,
    pub notifications_today: u64,
    pub read_rate: f64,
    pub avg_read_time_minutes: f64,
}

/// 通知搜索参数
#[derive(Debug, Clone, Default)]
pub struct NotificationSearchParams {
    pub user_id: Option<Uuid>,
    pub notification_type: Option<String>,
    pub priority: Option<String>,
    pub is_read: Option<bool>,
    pub is_dismissed: Option<bool>,
    pub related_user_id: Option<Uuid>,
    pub related_room_id: Option<Uuid>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

/// 通知Repository接口
#[async_trait]
pub trait NotificationRepository: Send + Sync {
    /// 创建通知
    async fn create(&self, notification: &Notification) -> DomainResult<Notification>;

    /// 根据ID查找通知
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<Notification>>;

    /// 获取用户通知列表
    async fn find_by_user(
        &self,
        user_id: Uuid,
        pagination: Pagination,
        include_read: bool,
    ) -> DomainResult<PaginatedResult<Notification>>;

    /// 标记通知为已读
    async fn mark_as_read(&self, notification_id: Uuid) -> DomainResult<()>;

    /// 批量标记为已读
    async fn mark_multiple_as_read(&self, notification_ids: &[Uuid]) -> DomainResult<u64>;

    /// 标记用户所有通知为已读
    async fn mark_all_as_read(&self, user_id: Uuid) -> DomainResult<u64>;

    /// 标记特定类型通知为已读
    async fn mark_type_as_read(&self, user_id: Uuid, notification_type: &str) -> DomainResult<u64>;

    /// 忽略通知
    async fn dismiss(&self, notification_id: Uuid) -> DomainResult<()>;

    /// 删除通知
    async fn delete(&self, notification_id: Uuid) -> DomainResult<bool>;

    /// 获取未读通知数量
    async fn count_unread(&self, user_id: Uuid) -> DomainResult<u64>;

    /// 根据类型获取未读通知数量
    async fn count_unread_by_type(
        &self,
        user_id: Uuid,
        notification_type: &str,
    ) -> DomainResult<u64>;

    /// 根据条件搜索通知
    async fn search(
        &self,
        params: &NotificationSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<Notification>>;

    /// 获取通知统计信息
    async fn get_statistics(&self, user_id: Uuid) -> DomainResult<NotificationStatistics>;

    /// 清理过期通知
    async fn cleanup_expired(&self) -> DomainResult<u64>;

    /// 根据模板创建通知
    async fn create_from_template(
        &self,
        user_id: Uuid,
        template_type: &str,
        data: &JsonValue,
        related_user_id: Option<Uuid>,
        related_room_id: Option<Uuid>,
        related_message_id: Option<Uuid>,
    ) -> DomainResult<Option<Notification>>;

    /// 获取高优先级未读通知
    async fn find_high_priority_unread(
        &self,
        user_id: Uuid,
        limit: u32,
    ) -> DomainResult<Vec<Notification>>;

    /// 批量创建通知
    async fn create_batch(&self, notifications: &[Notification])
        -> DomainResult<Vec<Notification>>;
}

/// 通知设置Repository接口
#[async_trait]
pub trait NotificationSettingsRepository: Send + Sync {
    /// 创建或更新通知设置
    async fn upsert(&self, settings: &NotificationSettings) -> DomainResult<NotificationSettings>;

    /// 获取用户通知设置
    async fn find_by_user(&self, user_id: Uuid) -> DomainResult<Vec<NotificationSettings>>;

    /// 获取用户特定类型的通知设置
    async fn find_by_user_and_type(
        &self,
        user_id: Uuid,
        notification_type: &str,
    ) -> DomainResult<Option<NotificationSettings>>;

    /// 检查是否应该发送通知
    async fn should_send_notification(
        &self,
        user_id: Uuid,
        notification_type: &str,
        priority: &str,
    ) -> DomainResult<bool>;

    /// 删除通知设置
    async fn delete(&self, user_id: Uuid, notification_type: &str) -> DomainResult<bool>;

    /// 重置为默认设置
    async fn reset_to_default(&self, user_id: Uuid) -> DomainResult<Vec<NotificationSettings>>;
}

/// 通知模板Repository接口
#[async_trait]
pub trait NotificationTemplateRepository: Send + Sync {
    /// 创建模板
    async fn create(&self, template: &NotificationTemplate) -> DomainResult<NotificationTemplate>;

    /// 根据类型查找模板
    async fn find_by_type(
        &self,
        notification_type: &str,
    ) -> DomainResult<Option<NotificationTemplate>>;

    /// 获取所有活跃模板
    async fn find_active(&self) -> DomainResult<Vec<NotificationTemplate>>;

    /// 更新模板
    async fn update(&self, template: &NotificationTemplate) -> DomainResult<NotificationTemplate>;

    /// 停用模板
    async fn deactivate(&self, notification_type: &str) -> DomainResult<()>;

    /// 删除模板
    async fn delete(&self, notification_type: &str) -> DomainResult<bool>;
}

/// 通知发送日志Repository接口
#[async_trait]
pub trait NotificationDeliveryLogRepository: Send + Sync {
    /// 记录发送日志
    async fn log_delivery(
        &self,
        log: &NotificationDeliveryLog,
    ) -> DomainResult<NotificationDeliveryLog>;

    /// 更新发送状态
    async fn update_status(
        &self,
        log_id: Uuid,
        status: &str,
        error_message: Option<&str>,
    ) -> DomainResult<()>;

    /// 获取通知的发送日志
    async fn find_by_notification(
        &self,
        notification_id: Uuid,
    ) -> DomainResult<Vec<NotificationDeliveryLog>>;

    /// 获取发送统计
    async fn get_delivery_stats(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> DomainResult<std::collections::HashMap<String, u64>>;

    /// 清理旧的发送日志
    async fn cleanup_old_logs(&self, older_than: DateTime<Utc>) -> DomainResult<u64>;
}
