//! 用户在线时长统计系统
//!
//! 追踪和分析用户活动时间、在线状态和使用情况

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 用户会话记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSession {
    /// 会话ID
    pub id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 会话开始时间
    pub started_at: DateTime<Utc>,
    /// 会话结束时间（可选，活跃会话为None）
    pub ended_at: Option<DateTime<Utc>>,
    /// 会话状态
    pub status: SessionStatus,
    /// 连接类型
    pub connection_type: ConnectionType,
    /// 客户端信息
    pub client_info: ClientInfo,
    /// IP地址
    pub ip_address: Option<String>,
    /// 会话元数据
    pub metadata: HashMap<String, String>,
}

/// 会话状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// 活跃中
    Active,
    /// 已结束（正常）
    Ended,
    /// 超时
    Timeout,
    /// 异常断开
    Disconnected,
    /// 强制结束
    Terminated,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Ended => "ended",
            SessionStatus::Timeout => "timeout",
            SessionStatus::Disconnected => "disconnected",
            SessionStatus::Terminated => "terminated",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(SessionStatus::Active),
            "ended" => Some(SessionStatus::Ended),
            "timeout" => Some(SessionStatus::Timeout),
            "disconnected" => Some(SessionStatus::Disconnected),
            "terminated" => Some(SessionStatus::Terminated),
            _ => None,
        }
    }
}

/// 连接类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionType {
    /// WebSocket连接
    WebSocket,
    /// HTTP API
    HttpApi,
    /// 移动应用
    Mobile,
    /// 桌面应用
    Desktop,
    /// 网页端
    Web,
    /// 其他
    Other(String),
}

impl ConnectionType {
    pub fn as_str(&self) -> String {
        match self {
            ConnectionType::WebSocket => "websocket".to_string(),
            ConnectionType::HttpApi => "http_api".to_string(),
            ConnectionType::Mobile => "mobile".to_string(),
            ConnectionType::Desktop => "desktop".to_string(),
            ConnectionType::Web => "web".to_string(),
            ConnectionType::Other(name) => name.clone(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "websocket" => ConnectionType::WebSocket,
            "http_api" => ConnectionType::HttpApi,
            "mobile" => ConnectionType::Mobile,
            "desktop" => ConnectionType::Desktop,
            "web" => ConnectionType::Web,
            _ => ConnectionType::Other(s.to_string()),
        }
    }
}

/// 客户端信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientInfo {
    /// 用户代理
    pub user_agent: Option<String>,
    /// 设备类型
    pub device_type: DeviceType,
    /// 操作系统
    pub os: Option<String>,
    /// 浏览器信息
    pub browser: Option<String>,
    /// 应用版本
    pub app_version: Option<String>,
}

/// 设备类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    /// 桌面设备
    Desktop,
    /// 移动设备
    Mobile,
    /// 平板设备
    Tablet,
    /// 未知设备
    Unknown,
}

impl DeviceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceType::Desktop => "desktop",
            DeviceType::Mobile => "mobile",
            DeviceType::Tablet => "tablet",
            DeviceType::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "desktop" => Some(DeviceType::Desktop),
            "mobile" => Some(DeviceType::Mobile),
            "tablet" => Some(DeviceType::Tablet),
            "unknown" => Some(DeviceType::Unknown),
            _ => None,
        }
    }
}

/// 每日在线统计
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyOnlineStatistics {
    /// 统计ID
    pub id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 统计日期
    pub date: NaiveDate,
    /// 总在线时长（分钟）
    pub total_online_minutes: i32,
    /// 活跃时长（分钟）
    pub active_minutes: i32,
    /// 空闲时长（分钟）
    pub idle_minutes: i32,
    /// 会话数量
    pub session_count: i32,
    /// 首次上线时间
    pub first_online_at: Option<DateTime<Utc>>,
    /// 最后上线时间
    pub last_online_at: Option<DateTime<Utc>>,
    /// 消息发送数量
    pub messages_sent: i32,
    /// 房间访问数量
    pub rooms_visited: i32,
    /// 设备类型分布
    pub device_usage: HashMap<String, i32>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 月度在线统计
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonthlyOnlineStatistics {
    /// 统计ID
    pub id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 年份
    pub year: i32,
    /// 月份 (1-12)
    pub month: u8,
    /// 总在线时长（分钟）
    pub total_online_minutes: i32,
    /// 活跃时长（分钟）
    pub active_minutes: i32,
    /// 平均每日在线时长（分钟）
    pub avg_daily_minutes: i32,
    /// 活跃天数
    pub active_days: i32,
    /// 总会话数量
    pub total_sessions: i32,
    /// 平均会话时长（分钟）
    pub avg_session_minutes: i32,
    /// 消息发送总数
    pub total_messages_sent: i32,
    /// 房间访问总数
    pub total_rooms_visited: i32,
    /// 最活跃时段分布（小时 -> 分钟数）
    pub hourly_distribution: HashMap<u8, i32>,
    /// 设备使用统计
    pub device_usage: HashMap<String, i32>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 用户活动记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserActivity {
    /// 活动ID
    pub id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 会话ID
    pub session_id: Uuid,
    /// 活动类型
    pub activity_type: ActivityType,
    /// 活动时间
    pub occurred_at: DateTime<Utc>,
    /// 房间ID（如果适用）
    pub room_id: Option<Uuid>,
    /// 活动详情
    pub details: HashMap<String, String>,
}

/// 活动类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityType {
    /// 登录
    Login,
    /// 登出
    Logout,
    /// 发送消息
    SendMessage,
    /// 加入房间
    JoinRoom,
    /// 离开房间
    LeaveRoom,
    /// 创建房间
    CreateRoom,
    /// 文件上传
    UploadFile,
    /// 状态变更
    StatusChange,
    /// 心跳
    Heartbeat,
    /// 自定义活动
    Custom(String),
}

impl ActivityType {
    pub fn as_str(&self) -> String {
        match self {
            ActivityType::Login => "login".to_string(),
            ActivityType::Logout => "logout".to_string(),
            ActivityType::SendMessage => "send_message".to_string(),
            ActivityType::JoinRoom => "join_room".to_string(),
            ActivityType::LeaveRoom => "leave_room".to_string(),
            ActivityType::CreateRoom => "create_room".to_string(),
            ActivityType::UploadFile => "upload_file".to_string(),
            ActivityType::StatusChange => "status_change".to_string(),
            ActivityType::Heartbeat => "heartbeat".to_string(),
            ActivityType::Custom(name) => name.clone(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "login" => ActivityType::Login,
            "logout" => ActivityType::Logout,
            "send_message" => ActivityType::SendMessage,
            "join_room" => ActivityType::JoinRoom,
            "leave_room" => ActivityType::LeaveRoom,
            "create_room" => ActivityType::CreateRoom,
            "upload_file" => ActivityType::UploadFile,
            "status_change" => ActivityType::StatusChange,
            "heartbeat" => ActivityType::Heartbeat,
            _ => ActivityType::Custom(s.to_string()),
        }
    }
}

/// 在线时长报告
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnlineTimeReport {
    /// 用户ID
    pub user_id: Uuid,
    /// 报告时间范围
    pub period: TimePeriod,
    /// 总在线时长（分钟）
    pub total_online_minutes: i32,
    /// 活跃时长（分钟）
    pub active_minutes: i32,
    /// 空闲时长（分钟）
    pub idle_minutes: i32,
    /// 会话统计
    pub session_stats: SessionStatistics,
    /// 活动统计
    pub activity_stats: ActivityStatistics,
    /// 时段分布
    pub time_distribution: TimeDistribution,
    /// 设备使用统计
    pub device_stats: DeviceStatistics,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

/// 时间周期
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimePeriod {
    /// 开始时间
    pub start: DateTime<Utc>,
    /// 结束时间
    pub end: DateTime<Utc>,
    /// 周期类型
    pub period_type: PeriodType,
}

/// 周期类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeriodType {
    /// 每日
    Daily,
    /// 每周
    Weekly,
    /// 每月
    Monthly,
    /// 每季度
    Quarterly,
    /// 每年
    Yearly,
    /// 自定义
    Custom,
}

/// 会话统计
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStatistics {
    /// 总会话数
    pub total_sessions: i32,
    /// 平均会话时长（分钟）
    pub avg_session_minutes: i32,
    /// 最长会话时长（分钟）
    pub max_session_minutes: i32,
    /// 最短会话时长（分钟）
    pub min_session_minutes: i32,
    /// 会话类型分布
    pub connection_type_distribution: HashMap<String, i32>,
}

/// 活动统计
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityStatistics {
    /// 总活动数
    pub total_activities: i32,
    /// 消息发送数量
    pub messages_sent: i32,
    /// 房间访问数量
    pub rooms_visited: i32,
    /// 文件上传数量
    pub files_uploaded: i32,
    /// 活动类型分布
    pub activity_type_distribution: HashMap<String, i32>,
}

/// 时段分布
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeDistribution {
    /// 按小时分布（0-23）
    pub hourly: HashMap<u8, i32>,
    /// 按星期分布（0=周日, 1=周一, ...）
    pub weekly: HashMap<u8, i32>,
    /// 按月份分布（1-12）
    pub monthly: HashMap<u8, i32>,
}

/// 设备统计
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceStatistics {
    /// 设备类型分布
    pub device_types: HashMap<String, i32>,
    /// 操作系统分布
    pub operating_systems: HashMap<String, i32>,
    /// 浏览器分布
    pub browsers: HashMap<String, i32>,
    /// 连接类型分布
    pub connection_types: HashMap<String, i32>,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            user_agent: None,
            device_type: DeviceType::Unknown,
            os: None,
            browser: None,
            app_version: None,
        }
    }
}

impl UserSession {
    /// 创建新的用户会话
    pub fn new(
        user_id: Uuid,
        connection_type: ConnectionType,
        client_info: ClientInfo,
        ip_address: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            started_at: Utc::now(),
            ended_at: None,
            status: SessionStatus::Active,
            connection_type,
            client_info,
            ip_address,
            metadata: HashMap::new(),
        }
    }

    /// 结束会话
    pub fn end(&mut self, status: SessionStatus) {
        self.ended_at = Some(Utc::now());
        self.status = status;
    }

    /// 获取会话持续时长
    pub fn duration(&self) -> Option<Duration> {
        let end_time = self.ended_at.unwrap_or_else(Utc::now);
        Some(end_time - self.started_at)
    }

    /// 获取会话持续时长（分钟）
    pub fn duration_minutes(&self) -> i32 {
        self.duration().map(|d| d.num_minutes() as i32).unwrap_or(0)
    }

    /// 检查会话是否仍然活跃
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Active)
    }

    /// 检查会话是否超时
    pub fn is_timeout(&self, timeout_minutes: i32) -> bool {
        if !self.is_active() {
            return false;
        }

        let elapsed = Utc::now() - self.started_at;
        elapsed.num_minutes() > timeout_minutes as i64
    }

    /// 添加元数据
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

impl DailyOnlineStatistics {
    /// 创建新的每日统计
    pub fn new(user_id: Uuid, date: NaiveDate) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            date,
            total_online_minutes: 0,
            active_minutes: 0,
            idle_minutes: 0,
            session_count: 0,
            first_online_at: None,
            last_online_at: None,
            messages_sent: 0,
            rooms_visited: 0,
            device_usage: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 添加会话数据
    pub fn add_session(&mut self, session: &UserSession) {
        let duration_minutes = session.duration_minutes();

        self.total_online_minutes += duration_minutes;
        self.session_count += 1;

        // 更新首次和最后上线时间
        if self.first_online_at.is_none() || session.started_at < self.first_online_at.unwrap() {
            self.first_online_at = Some(session.started_at);
        }

        let end_time = session.ended_at.unwrap_or_else(Utc::now);
        if self.last_online_at.is_none() || end_time > self.last_online_at.unwrap() {
            self.last_online_at = Some(end_time);
        }

        // 统计设备使用
        let device_key = session.client_info.device_type.as_str().to_string();
        *self.device_usage.entry(device_key).or_insert(0) += duration_minutes;

        self.updated_at = Utc::now();
    }

    /// 添加活动数据
    pub fn add_activity(&mut self, activity: &UserActivity) {
        match activity.activity_type {
            ActivityType::SendMessage => {
                self.messages_sent += 1;
            }
            ActivityType::JoinRoom => {
                self.rooms_visited += 1;
            }
            _ => {}
        }

        self.updated_at = Utc::now();
    }

    /// 计算活动率
    pub fn activity_rate(&self) -> f64 {
        if self.total_online_minutes == 0 {
            0.0
        } else {
            (self.active_minutes as f64) / (self.total_online_minutes as f64)
        }
    }
}

impl MonthlyOnlineStatistics {
    /// 创建新的月度统计
    pub fn new(user_id: Uuid, year: i32, month: u8) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            year,
            month,
            total_online_minutes: 0,
            active_minutes: 0,
            avg_daily_minutes: 0,
            active_days: 0,
            total_sessions: 0,
            avg_session_minutes: 0,
            total_messages_sent: 0,
            total_rooms_visited: 0,
            hourly_distribution: HashMap::new(),
            device_usage: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// 基于每日统计计算月度统计
    pub fn calculate_from_daily(&mut self, daily_stats: &[DailyOnlineStatistics]) {
        self.active_days = daily_stats.len() as i32;

        if self.active_days == 0 {
            return;
        }

        // 聚合统计数据
        for daily in daily_stats {
            self.total_online_minutes += daily.total_online_minutes;
            self.active_minutes += daily.active_minutes;
            self.total_sessions += daily.session_count;
            self.total_messages_sent += daily.messages_sent;
            self.total_rooms_visited += daily.rooms_visited;

            // 聚合设备使用统计
            for (device, minutes) in &daily.device_usage {
                *self.device_usage.entry(device.clone()).or_insert(0) += minutes;
            }
        }

        // 计算平均值
        self.avg_daily_minutes = self.total_online_minutes / self.active_days;
        if self.total_sessions > 0 {
            self.avg_session_minutes = self.total_online_minutes / self.total_sessions;
        }

        self.updated_at = Utc::now();
    }
}

impl UserActivity {
    /// 创建新的用户活动记录
    pub fn new(
        user_id: Uuid,
        session_id: Uuid,
        activity_type: ActivityType,
        room_id: Option<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            session_id,
            activity_type,
            occurred_at: Utc::now(),
            room_id,
            details: HashMap::new(),
        }
    }

    /// 添加活动详情
    pub fn add_detail(&mut self, key: String, value: String) {
        self.details.insert(key, value);
    }
}

impl TimePeriod {
    /// 创建每日周期
    pub fn daily(date: NaiveDate) -> Self {
        let start = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end = date.and_hms_opt(23, 59, 59).unwrap().and_utc();

        Self {
            start,
            end,
            period_type: PeriodType::Daily,
        }
    }

    /// 创建自定义周期
    pub fn custom(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            start,
            end,
            period_type: PeriodType::Custom,
        }
    }

    /// 获取周期持续时长
    pub fn duration(&self) -> Duration {
        self.end - self.start
    }

    /// 获取周期天数
    pub fn days(&self) -> i64 {
        self.duration().num_days()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_session_creation() {
        let user_id = Uuid::new_v4();
        let client_info = ClientInfo::default();
        let session = UserSession::new(
            user_id,
            ConnectionType::WebSocket,
            client_info,
            Some("127.0.0.1".to_string()),
        );

        assert_eq!(session.user_id, user_id);
        assert_eq!(session.connection_type, ConnectionType::WebSocket);
        assert_eq!(session.status, SessionStatus::Active);
        assert!(session.is_active());
        assert!(session.ended_at.is_none());
    }

    #[test]
    fn test_session_duration() {
        let user_id = Uuid::new_v4();
        let mut session =
            UserSession::new(user_id, ConnectionType::Web, ClientInfo::default(), None);

        // 模拟会话开始时间为1小时前
        session.started_at = Utc::now() - Duration::hours(1);

        // 结束会话
        session.end(SessionStatus::Ended);

        let duration = session.duration().unwrap();
        assert!(duration.num_minutes() >= 59); // 约1小时
        assert!(!session.is_active());
    }

    #[test]
    fn test_daily_statistics() {
        let user_id = Uuid::new_v4();
        let date = Utc::now().date_naive();
        let mut daily_stats = DailyOnlineStatistics::new(user_id, date);

        // 创建一个测试会话
        let mut session = UserSession::new(
            user_id,
            ConnectionType::Mobile,
            ClientInfo {
                device_type: DeviceType::Mobile,
                ..Default::default()
            },
            None,
        );

        // 设置会话持续30分钟
        session.started_at = Utc::now() - Duration::minutes(30);
        session.end(SessionStatus::Ended);

        // 添加到统计
        daily_stats.add_session(&session);

        assert_eq!(daily_stats.total_online_minutes, 30);
        assert_eq!(daily_stats.session_count, 1);
        assert!(daily_stats.first_online_at.is_some());
        assert!(daily_stats.last_online_at.is_some());
        assert_eq!(daily_stats.device_usage.get("mobile"), Some(&30));
    }

    #[test]
    fn test_activity_creation() {
        let user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let room_id = Some(Uuid::new_v4());

        let mut activity =
            UserActivity::new(user_id, session_id, ActivityType::SendMessage, room_id);

        activity.add_detail("message_content".to_string(), "Hello, world!".to_string());

        assert_eq!(activity.user_id, user_id);
        assert_eq!(activity.session_id, session_id);
        assert_eq!(activity.room_id, room_id);
        assert_eq!(activity.activity_type, ActivityType::SendMessage);
        assert_eq!(
            activity.details.get("message_content"),
            Some(&"Hello, world!".to_string())
        );
    }

    #[test]
    fn test_time_period() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let period = TimePeriod::daily(date);

        assert_eq!(period.period_type, PeriodType::Daily);
        assert_eq!(period.days(), 0); // 同一天
        assert_eq!(period.start.date_naive(), date);
        assert_eq!(period.end.date_naive(), date);
    }
}
