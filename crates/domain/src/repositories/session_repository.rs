//! 会话管理Repository接口定义

use crate::errors::DomainResult;
use crate::repositories::{Pagination, PaginatedResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// 会话实体
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub refresh_token_hash: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_info: JsonValue,
    pub is_active: bool,
    pub session_type: String, // web, mobile, api, bot
}

/// 在线时长统计
#[derive(Debug, Clone)]
pub struct OnlineTimeStats {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: chrono::NaiveDate,
    pub total_seconds: i32,
    pub sessions_count: i32,
    pub first_session_at: Option<DateTime<Utc>>,
    pub last_session_at: Option<DateTime<Utc>>,
    pub device_types: JsonValue,
}

/// 用户活动日志
#[derive(Debug, Clone)]
pub struct UserActivityLog {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: Option<Uuid>,
    pub activity_type: String,
    pub activity_data: JsonValue,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 会话统计信息
#[derive(Debug, Clone)]
pub struct SessionStatistics {
    pub total_sessions: u64,
    pub active_sessions: u64,
    pub sessions_today: u64,
    pub unique_users_today: u64,
    pub avg_session_duration_minutes: f64,
    pub sessions_by_type: std::collections::HashMap<String, u64>,
}

/// 会话Repository接口
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// 创建新会话
    async fn create(&self, session: &Session) -> DomainResult<Session>;

    /// 根据ID查找会话
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<Session>>;

    /// 根据token hash查找会话
    async fn find_by_token_hash(&self, token_hash: &str) -> DomainResult<Option<Session>>;

    /// 根据refresh token hash查找会话
    async fn find_by_refresh_token(&self, refresh_token_hash: &str) -> DomainResult<Option<Session>>;

    /// 更新会话
    async fn update(&self, session: &Session) -> DomainResult<Session>;

    /// 更新最后访问时间
    async fn update_last_accessed(&self, session_id: Uuid, last_accessed_at: DateTime<Utc>) -> DomainResult<()>;

    /// 刷新token
    async fn refresh_token(&self, session_id: Uuid, new_token_hash: &str, new_refresh_token_hash: Option<&str>) -> DomainResult<()>;

    /// 使会话失效
    async fn invalidate(&self, session_id: Uuid) -> DomainResult<()>;

    /// 删除会话
    async fn delete(&self, session_id: Uuid) -> DomainResult<bool>;

    /// 获取用户的所有活跃会话
    async fn find_active_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<Session>>;

    /// 获取用户的所有会话（包括非活跃）
    async fn find_all_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<Session>>;

    /// 清理过期会话
    async fn cleanup_expired(&self) -> DomainResult<u64>;

    /// 统计活跃会话数
    async fn count_active(&self) -> DomainResult<u64>;

    /// 统计用户活跃会话数
    async fn count_active_by_user(&self, user_id: Uuid) -> DomainResult<u64>;

    /// 获取会话统计信息
    async fn get_statistics(&self) -> DomainResult<SessionStatistics>;

    /// 根据IP地址查找会话
    async fn find_by_ip(&self, ip_address: &str, pagination: Pagination) -> DomainResult<PaginatedResult<Session>>;

    /// 根据设备类型查找会话
    async fn find_by_device_type(&self, device_type: &str, pagination: Pagination) -> DomainResult<PaginatedResult<Session>>;

    /// 使用户所有会话失效
    async fn invalidate_all_user_sessions(&self, user_id: Uuid) -> DomainResult<u64>;

    /// 使除当前会话外的所有用户会话失效
    async fn invalidate_other_user_sessions(&self, user_id: Uuid, current_session_id: Uuid) -> DomainResult<u64>;
}

/// 在线时长统计Repository接口
#[async_trait]
pub trait OnlineTimeRepository: Send + Sync {
    /// 记录在线时长
    async fn record_online_time(
        &self,
        user_id: Uuid,
        session_start: DateTime<Utc>,
        session_end: DateTime<Utc>,
        device_type: &str,
    ) -> DomainResult<()>;

    /// 获取用户某日在线时长
    async fn find_by_user_and_date(&self, user_id: Uuid, date: chrono::NaiveDate) -> DomainResult<Option<OnlineTimeStats>>;

    /// 获取用户在线时长统计
    async fn find_by_user(
        &self,
        user_id: Uuid,
        start_date: chrono::NaiveDate,
        end_date: chrono::NaiveDate,
    ) -> DomainResult<Vec<OnlineTimeStats>>;

    /// 获取月度在线时长统计
    async fn get_monthly_stats(&self, user_id: Uuid, year: i32, month: u32) -> DomainResult<Vec<OnlineTimeStats>>;

    /// 获取用户总在线时长
    async fn get_total_online_time(&self, user_id: Uuid) -> DomainResult<i64>;

    /// 获取平均每日在线时长
    async fn get_average_daily_time(&self, user_id: Uuid, days: u32) -> DomainResult<f64>;
}

/// 用户活动日志Repository接口
#[async_trait]
pub trait UserActivityLogRepository: Send + Sync {
    /// 记录用户活动
    async fn log_activity(&self, activity: &UserActivityLog) -> DomainResult<UserActivityLog>;

    /// 获取用户活动历史
    async fn find_by_user(
        &self,
        user_id: Uuid,
        pagination: Pagination,
        activity_type: Option<&str>,
    ) -> DomainResult<PaginatedResult<UserActivityLog>>;

    /// 获取会话活动历史
    async fn find_by_session(
        &self,
        session_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<UserActivityLog>>;

    /// 根据活动类型统计
    async fn count_by_type(&self, activity_type: &str, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> DomainResult<u64>;

    /// 清理旧的活动日志
    async fn cleanup_old_logs(&self, older_than: DateTime<Utc>) -> DomainResult<u64>;

    /// 获取最近活动
    async fn find_recent_activities(&self, limit: u32) -> DomainResult<Vec<UserActivityLog>>;
}