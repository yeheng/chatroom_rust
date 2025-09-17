//! 用户Repository接口定义

use crate::entities::user::{User, UserStatus};
use crate::errors::DomainResult;
use crate::repositories::{Pagination, PaginatedResult, QueryFilter, SortConfig};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 用户统计信息
#[derive(Debug, Clone)]
pub struct UserStatistics {
    pub total_users: u64,
    pub active_users: u64,
    pub inactive_users: u64,
    pub suspended_users: u64,
    pub users_registered_today: u64,
    pub users_online: u64,
}

/// 用户搜索参数
#[derive(Debug, Clone, Default)]
pub struct UserSearchParams {
    pub username: Option<String>,
    pub email: Option<String>,
    pub status: Option<UserStatus>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub last_active_after: Option<DateTime<Utc>>,
}

/// 用户Repository接口
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// 创建新用户
    async fn create(&self, user: &User) -> DomainResult<User>;

    /// 根据ID查找用户
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<User>>;

    /// 根据用户名查找用户
    async fn find_by_username(&self, username: &str) -> DomainResult<Option<User>>;

    /// 根据邮箱查找用户
    async fn find_by_email(&self, email: &str) -> DomainResult<Option<User>>;

    /// 更新用户信息
    async fn update(&self, user: &User) -> DomainResult<User>;

    /// 更新密码哈希
    async fn update_password_hash(&self, user_id: Uuid, password_hash: &str) -> DomainResult<()>;

    /// 更新用户状态
    async fn update_status(&self, user_id: Uuid, status: UserStatus) -> DomainResult<()>;

    /// 更新最后活跃时间
    async fn update_last_active(&self, user_id: Uuid, last_active_at: DateTime<Utc>) -> DomainResult<()>;

    /// 软删除用户
    async fn soft_delete(&self, user_id: Uuid) -> DomainResult<()>;

    /// 根据条件搜索用户
    async fn search(
        &self,
        params: &UserSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<User>>;

    /// 获取用户统计信息
    async fn get_statistics(&self) -> DomainResult<UserStatistics>;

    /// 检查用户名是否存在
    async fn username_exists(&self, username: &str) -> DomainResult<bool>;

    /// 检查邮箱是否存在
    async fn email_exists(&self, email: &str) -> DomainResult<bool>;

    /// 批量获取用户
    async fn find_by_ids(&self, ids: &[Uuid]) -> DomainResult<Vec<User>>;

    /// 获取最近注册的用户
    async fn find_recent_users(&self, limit: u32) -> DomainResult<Vec<User>>;

    /// 获取在线用户
    async fn find_online_users(&self, pagination: Pagination) -> DomainResult<PaginatedResult<User>>;

    /// 根据角色查找用户（企业版功能）
    #[cfg(feature = "enterprise")]
    async fn find_by_role(&self, role_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<User>>;

    /// 根据组织查找用户（企业版功能）
    #[cfg(feature = "enterprise")]
    async fn find_by_organization(&self, org_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<User>>;
}