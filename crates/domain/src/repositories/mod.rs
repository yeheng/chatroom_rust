//! Repository接口定义
//!
//! 定义数据访问层的抽象接口，遵循清洁架构原则，内层定义接口，外层实现接口。

use crate::errors::DomainResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

// 重新导出所有Repository特征
pub use user_repository::*;
pub use chatroom_repository::*;
pub use message_repository::*;
pub use room_member_repository::*;
pub use session_repository::*;
pub use file_upload_repository::*;
pub use notification_repository::*;
pub use statistics_repository::*;

mod user_repository;
mod chatroom_repository;
mod message_repository;
mod room_member_repository;
mod session_repository;
mod file_upload_repository;
mod notification_repository;
mod statistics_repository;

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