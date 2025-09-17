//! 系统统计和监控Repository接口定义

use crate::errors::DomainResult;
use crate::repositories::{Pagination, PaginatedResult, QueryFilter, SortConfig};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

/// 每日统计实体
#[derive(Debug, Clone)]
pub struct DailyStats {
    pub id: Uuid,
    pub stat_date: NaiveDate,
    pub total_users: i32,
    pub active_users: i32,
    pub new_users: i32,
    pub total_rooms: i32,
    pub active_rooms: i32,
    pub new_rooms: i32,
    pub total_messages: i32,
    pub new_messages: i32,
    pub total_files: i32,
    pub new_files: i32,
    pub storage_used_bytes: i64,
    pub avg_session_duration_minutes: Option<i32>,
    pub peak_concurrent_users: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 系统指标实体
#[derive(Debug, Clone)]
pub struct SystemMetric {
    pub id: Uuid,
    pub metric_name: String,
    pub metric_value: f64,
    pub metric_unit: Option<String>,
    pub tags: JsonValue,
    pub timestamp: DateTime<Utc>,
    pub server_id: Option<String>,
    pub instance_id: Option<String>,
}

/// 在线用户实体
#[derive(Debug, Clone)]
pub struct OnlineUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub room_id: Option<Uuid>,
    pub status: String, // online, away, busy, invisible
    pub last_seen_at: DateTime<Utc>,
    pub device_type: String, // web, mobile, desktop, tablet, bot
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub location_data: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 房间活动统计实体
#[derive(Debug, Clone)]
pub struct RoomActivityStats {
    pub id: Uuid,
    pub room_id: Uuid,
    pub stat_date: NaiveDate,
    pub message_count: Option<i32>,
    pub active_users_count: Option<i32>,
    pub peak_concurrent_users: Option<i32>,
    pub avg_response_time_seconds: Option<i32>,
    pub file_uploads_count: Option<i32>,
    pub total_file_size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 错误日志实体
#[derive(Debug, Clone)]
pub struct ErrorLog {
    pub id: Uuid,
    pub error_type: String,
    pub error_code: Option<String>,
    pub error_message: String,
    pub stack_trace: Option<String>,
    pub user_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub request_path: Option<String>,
    pub request_method: Option<String>,
    pub request_params: JsonValue,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub server_id: Option<String>,
    pub severity: String, // debug, info, warning, error, critical
    pub created_at: DateTime<Utc>,
}

/// 系统健康状态
#[derive(Debug, Clone)]
pub struct SystemHealthStatus {
    pub overall_status: String, // healthy, warning, critical, unknown
    pub cpu_usage: Option<f64>,
    pub memory_usage: Option<f64>,
    pub disk_usage: Option<f64>,
    pub error_count: i64,
    pub critical_error_count: i64,
    pub checked_at: DateTime<Utc>,
}

/// 实时统计信息
#[derive(Debug, Clone)]
pub struct RealtimeStats {
    pub current_online_users: i64,
    pub active_rooms: i64,
    pub messages_last_hour: i64,
    pub active_sessions: i64,
    pub unread_notifications: i64,
    pub bytes_uploaded_today: Option<i64>,
}

/// 每日统计Repository接口
#[async_trait]
pub trait DailyStatsRepository: Send + Sync {
    /// 生成每日统计
    async fn generate_daily_stats(&self, target_date: NaiveDate) -> DomainResult<DailyStats>;

    /// 获取指定日期的统计
    async fn find_by_date(&self, date: NaiveDate) -> DomainResult<Option<DailyStats>>;

    /// 获取日期范围内的统计
    async fn find_by_date_range(&self, start_date: NaiveDate, end_date: NaiveDate) -> DomainResult<Vec<DailyStats>>;

    /// 获取最近的统计数据
    async fn find_recent(&self, days: u32) -> DomainResult<Vec<DailyStats>>;

    /// 更新每日统计
    async fn update(&self, stats: &DailyStats) -> DomainResult<DailyStats>;

    /// 获取月度汇总统计
    async fn get_monthly_summary(&self, year: i32, month: u32) -> DomainResult<DailyStats>;

    /// 获取年度汇总统计
    async fn get_yearly_summary(&self, year: i32) -> DomainResult<DailyStats>;
}

/// 系统指标Repository接口
#[async_trait]
pub trait SystemMetricRepository: Send + Sync {
    /// 记录系统指标
    async fn record_metric(&self, metric: &SystemMetric) -> DomainResult<SystemMetric>;

    /// 批量记录指标
    async fn record_metrics_batch(&self, metrics: &[SystemMetric]) -> DomainResult<Vec<SystemMetric>>;

    /// 获取指定指标的最新值
    async fn get_latest_metric(&self, metric_name: &str, server_id: Option<&str>) -> DomainResult<Option<SystemMetric>>;

    /// 获取指标时间序列数据
    async fn get_metric_timeseries(
        &self,
        metric_name: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        server_id: Option<&str>,
    ) -> DomainResult<Vec<SystemMetric>>;

    /// 获取指标平均值
    async fn get_metric_average(
        &self,
        metric_name: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        server_id: Option<&str>,
    ) -> DomainResult<Option<f64>>;

    /// 获取指标统计信息（最大值、最小值、平均值）
    async fn get_metric_stats(
        &self,
        metric_name: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        server_id: Option<&str>,
    ) -> DomainResult<(f64, f64, f64)>; // (min, max, avg)

    /// 清理旧的指标数据
    async fn cleanup_old_metrics(&self, older_than: DateTime<Utc>) -> DomainResult<u64>;

    /// 获取所有指标名称
    async fn get_metric_names(&self) -> DomainResult<Vec<String>>;

    /// 获取服务器列表
    async fn get_server_ids(&self) -> DomainResult<Vec<String>>;
}

/// 在线用户Repository接口
#[async_trait]
pub trait OnlineUserRepository: Send + Sync {
    /// 用户上线
    async fn user_online(&self, user: &OnlineUser) -> DomainResult<OnlineUser>;

    /// 用户下线
    async fn user_offline(&self, user_id: Uuid) -> DomainResult<()>;

    /// 更新用户状态
    async fn update_status(&self, user_id: Uuid, status: &str) -> DomainResult<()>;

    /// 更新最后在线时间
    async fn update_last_seen(&self, user_id: Uuid, last_seen_at: DateTime<Utc>) -> DomainResult<()>;

    /// 设置用户所在房间
    async fn set_user_room(&self, user_id: Uuid, room_id: Option<Uuid>) -> DomainResult<()>;

    /// 获取在线用户列表
    async fn find_online_users(&self, pagination: Pagination) -> DomainResult<PaginatedResult<OnlineUser>>;

    /// 获取房间在线用户
    async fn find_online_in_room(&self, room_id: Uuid) -> DomainResult<Vec<OnlineUser>>;

    /// 统计在线用户数
    async fn count_online(&self) -> DomainResult<u64>;

    /// 统计房间在线用户数
    async fn count_online_in_room(&self, room_id: Uuid) -> DomainResult<u64>;

    /// 根据设备类型统计在线用户
    async fn count_by_device_type(&self) -> DomainResult<HashMap<String, u64>>;

    /// 清理不活跃的在线记录
    async fn cleanup_inactive(&self, inactive_minutes: u32) -> DomainResult<u64>;
}

/// 房间活动统计Repository接口
#[async_trait]
pub trait RoomActivityStatsRepository: Send + Sync {
    /// 更新房间活动统计
    async fn update_room_activity(&self, stats: &RoomActivityStats) -> DomainResult<RoomActivityStats>;

    /// 获取房间活动统计
    async fn find_by_room_and_date(&self, room_id: Uuid, date: NaiveDate) -> DomainResult<Option<RoomActivityStats>>;

    /// 获取房间活动统计历史
    async fn find_by_room(
        &self,
        room_id: Uuid,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> DomainResult<Vec<RoomActivityStats>>;

    /// 获取最活跃的房间
    async fn find_most_active_rooms(&self, date: NaiveDate, limit: u32) -> DomainResult<Vec<RoomActivityStats>>;

    /// 增加房间消息计数
    async fn increment_message_count(&self, room_id: Uuid, date: NaiveDate, count: i32) -> DomainResult<()>;

    /// 更新房间峰值在线用户数
    async fn update_peak_users(&self, room_id: Uuid, date: NaiveDate, peak_users: i32) -> DomainResult<()>;
}

/// 错误日志Repository接口
#[async_trait]
pub trait ErrorLogRepository: Send + Sync {
    /// 记录错误日志
    async fn log_error(&self, error_log: &ErrorLog) -> DomainResult<ErrorLog>;

    /// 根据严重级别查找错误
    async fn find_by_severity(
        &self,
        severity: &str,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ErrorLog>>;

    /// 根据错误类型查找错误
    async fn find_by_error_type(
        &self,
        error_type: &str,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ErrorLog>>;

    /// 获取最近的错误日志
    async fn find_recent(&self, limit: u32) -> DomainResult<Vec<ErrorLog>>;

    /// 统计错误数量
    async fn count_errors(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        severity: Option<&str>,
    ) -> DomainResult<u64>;

    /// 获取错误趋势
    async fn get_error_trend(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        interval_hours: u32,
    ) -> DomainResult<Vec<(DateTime<Utc>, u64)>>;

    /// 清理旧的错误日志
    async fn cleanup_old_logs(&self, older_than: DateTime<Utc>) -> DomainResult<u64>;

    /// 根据服务器ID查找错误
    async fn find_by_server(&self, server_id: &str, pagination: Pagination) -> DomainResult<PaginatedResult<ErrorLog>>;
}

/// 系统健康监控Repository接口
#[async_trait]
pub trait SystemHealthRepository: Send + Sync {
    /// 获取系统健康状态
    async fn get_health_status(&self) -> DomainResult<SystemHealthStatus>;

    /// 获取实时统计信息
    async fn get_realtime_stats(&self) -> DomainResult<RealtimeStats>;

    /// 清理旧数据
    async fn cleanup_old_data(&self) -> DomainResult<HashMap<String, u64>>;
}