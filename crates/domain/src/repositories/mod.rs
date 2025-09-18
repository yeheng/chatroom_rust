//! Repository接口定义
//!
//! 定义数据访问层的抽象接口，遵循清洁架构原则，内层定义接口，外层实现接口。

use crate::errors::DomainResult;
use async_trait::async_trait;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// 重新导出所有Repository特征
pub use user_repository::{UserRepository, UserSearchParams, UserStatistics};
pub use chatroom_repository::{ChatRoomRepository, ChatRoomSearchParams, ChatRoomStatistics, RoomActivityStats};
pub use message_repository::{MessageRepository, MessageSearchParams, MessageStatistics};
pub use room_member_repository::{RoomMemberRepository, RoomMemberSearchParams, RoomMemberStatistics, MemberPermissions};
pub use session_repository::{SessionRepository};
pub use file_upload_repository::{
    FileUploadRepository, FileSearchParams, FileStatistics,
    FileShare, FileShareRepository, FileAccessLog, FileAccessLogRepository
};
pub use notification_repository::{
    NotificationRepository, NotificationSearchParams, NotificationStatistics
};
pub use statistics_repository::{
    DailyStats, DailyStatsRepository, SystemMetric, SystemMetricRepository,
    OnlineUser, OnlineUserRepository, RoomActivityStats as StatisticsRoomActivityStats,
    RoomActivityStatsRepository, ErrorLog, ErrorLogRepository,
    SystemHealthStatus, RealtimeStats, SystemHealthRepository
};

/// 简化的统计Repository接口 - 用于基础统计功能
#[async_trait]
pub trait StatisticsRepository: Send + Sync {
    /// 获取系统统计信息
    async fn get_system_statistics(&self) -> DomainResult<HashMap<String, u64>>;

    /// 获取用户活动统计
    async fn get_user_activity_stats(&self, user_id: Uuid, date_from: DateTime<Utc>, date_to: DateTime<Utc>) -> DomainResult<HashMap<String, u64>>;

    /// 获取房间活动统计
    async fn get_room_activity_stats(&self, room_id: Uuid, date_from: DateTime<Utc>, date_to: DateTime<Utc>) -> DomainResult<HashMap<String, u64>>;

    /// 记录用户在线时长
    async fn record_user_online_time(&self, user_id: Uuid, duration_minutes: u64) -> DomainResult<()>;

    /// 获取热门房间
    async fn get_popular_rooms(&self, limit: u32) -> DomainResult<Vec<(Uuid, String, u64)>>;

    /// 获取活跃用户
    async fn get_active_users(&self, limit: u32) -> DomainResult<Vec<(Uuid, String, u64)>>;

    /// 清理旧统计数据
    async fn cleanup_old_statistics(&self, days_old: u32) -> DomainResult<u64>;
}

// RoomActivityStats 已在上面导出

pub mod user_repository;
pub mod chatroom_repository;
pub mod message_repository;
pub mod room_member_repository;
pub mod session_repository;
pub mod file_upload_repository;
pub mod notification_repository;
pub mod statistics_repository;

/// 分页参数
#[derive(Debug, Clone)]
pub struct Pagination {
    pub page: u32,
    pub page_size: u32,
    pub offset: u64,
    pub limit: u64,
}

impl Pagination {
    pub fn new(page: u32, page_size: u32) -> Self {
        let offset = ((page.saturating_sub(1)) * page_size) as u64;
        let limit = page_size as u64;
        Self { page, page_size, offset, limit }
    }

    pub fn default_page() -> Self {
        Self::new(1, 20)
    }
}

/// 分页结果
#[derive(Debug, Clone)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total_count: u64,
    pub page: u32,
    pub page_size: u32,
    pub has_next: bool,
    pub has_prev: bool,
}

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total_count: u64, pagination: Pagination) -> Self {
        let has_next = (pagination.page as u64 * pagination.page_size as u64) < total_count;
        let has_prev = pagination.page > 1;

        Self {
            items,
            total_count,
            page: pagination.page,
            page_size: pagination.page_size,
            has_next,
            has_prev,
        }
    }
}

/// 排序配置
#[derive(Debug, Clone)]
pub struct SortConfig {
    pub field: String,
    pub ascending: bool,
}

impl SortConfig {
    pub fn new(field: impl Into<String>, ascending: bool) -> Self {
        Self {
            field: field.into(),
            ascending,
        }
    }

    pub fn desc(field: impl Into<String>) -> Self {
        Self::new(field, false)
    }

    pub fn asc(field: impl Into<String>) -> Self {
        Self::new(field, true)
    }
}

/// 查询过滤器
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    pub conditions: HashMap<String, QueryCondition>,
}

impl QueryFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_condition(mut self, field: impl Into<String>, condition: QueryCondition) -> Self {
        self.conditions.insert(field.into(), condition);
        self
    }

    pub fn equals(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.conditions.insert(field.into(), QueryCondition::Equals(value.into()));
        self
    }

    pub fn contains(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.conditions.insert(field.into(), QueryCondition::Contains(value.into()));
        self
    }
}

/// 查询条件
#[derive(Debug, Clone)]
pub enum QueryCondition {
    Equals(String),
    NotEquals(String),
    Contains(String),
    GreaterThan(String),
    LessThan(String),
    Between(String, String),
    In(Vec<String>),
    IsNull,
    IsNotNull,
}

/// 通用Repository特征
#[async_trait]
pub trait Repository<T, ID> {
    /// 根据ID查找实体
    async fn find_by_id(&self, id: ID) -> DomainResult<Option<T>>;

    /// 保存实体（插入或更新）
    async fn save(&self, entity: &T) -> DomainResult<T>;

    /// 删除实体
    async fn delete_by_id(&self, id: ID) -> DomainResult<bool>;

    /// 检查实体是否存在
    async fn exists(&self, id: ID) -> DomainResult<bool>;
}