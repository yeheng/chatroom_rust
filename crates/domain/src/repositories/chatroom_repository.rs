//! 聊天室Repository接口定义

use crate::entities::chatroom::{ChatRoom, ChatRoomStatus};
use crate::errors::DomainResult;
use crate::repositories::{PaginatedResult, Pagination, SortConfig};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 聊天室统计信息
#[derive(Debug, Clone)]
pub struct ChatRoomStatistics {
    pub total_rooms: u64,
    pub active_rooms: u64,
    pub private_rooms: u64,
    pub public_rooms: u64,
    pub rooms_created_today: u64,
    pub rooms_with_activity_today: u64,
    pub avg_members_per_room: f64,
}

/// 聊天室搜索参数
#[derive(Debug, Clone, Default)]
pub struct ChatRoomSearchParams {
    pub name: Option<String>,
    pub description: Option<String>,
    pub owner_id: Option<Uuid>,
    pub is_private: Option<bool>,
    pub status: Option<ChatRoomStatus>,
    pub has_password: Option<bool>,
    pub min_members: Option<u32>,
    pub max_members: Option<u32>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub last_activity_after: Option<DateTime<Utc>>,
}

/// 房间活动统计
#[derive(Debug, Clone)]
pub struct RoomActivityStats {
    pub room_id: Uuid,
    pub message_count: u64,
    pub active_members: u64,
    pub last_message_at: Option<DateTime<Utc>>,
    pub peak_concurrent_members: u32,
}

/// 聊天室Repository接口
#[async_trait]
pub trait ChatRoomRepository: Send + Sync {
    /// 创建新聊天室
    async fn create(&self, room: &ChatRoom) -> DomainResult<ChatRoom>;

    /// 根据ID查找聊天室
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<ChatRoom>>;

    /// 根据名称查找聊天室
    async fn find_by_name(&self, name: &str) -> DomainResult<Option<ChatRoom>>;

    /// 更新聊天室信息
    async fn update(&self, room: &ChatRoom) -> DomainResult<ChatRoom>;

    /// 更新聊天室成员数量
    async fn update_member_count(&self, room_id: Uuid, member_count: u32) -> DomainResult<()>;

    /// 更新聊天室状态
    async fn update_status(&self, room_id: Uuid, status: ChatRoomStatus) -> DomainResult<()>;

    /// 更新最后活跃时间
    async fn update_last_activity(
        &self,
        room_id: Uuid,
        last_activity_at: DateTime<Utc>,
    ) -> DomainResult<()>;

    /// 设置房间密码
    async fn set_password(&self, room_id: Uuid, password_hash: Option<&str>) -> DomainResult<()>;

    /// 软删除聊天室
    async fn soft_delete(&self, room_id: Uuid) -> DomainResult<()>;

    /// 根据条件搜索聊天室
    async fn search(
        &self,
        params: &ChatRoomSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<ChatRoom>>;

    /// 获取用户拥有的聊天室
    async fn find_by_owner(
        &self,
        owner_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>>;

    /// 获取用户参与的聊天室
    async fn find_by_member(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>>;

    /// 获取公开聊天室列表
    async fn find_public_rooms(
        &self,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>>;

    /// 获取最活跃的聊天室
    async fn find_most_active(&self, limit: u32) -> DomainResult<Vec<ChatRoom>>;

    /// 获取最近创建的聊天室
    async fn find_recent_rooms(&self, limit: u32) -> DomainResult<Vec<ChatRoom>>;

    /// 获取聊天室统计信息
    async fn get_statistics(&self) -> DomainResult<ChatRoomStatistics>;

    /// 批量获取聊天室
    async fn find_by_ids(&self, ids: &[Uuid]) -> DomainResult<Vec<ChatRoom>>;

    /// 检查聊天室名称是否存在
    async fn name_exists(&self, name: &str) -> DomainResult<bool>;

    /// 验证房间密码
    async fn verify_password(&self, room_id: Uuid, password: &str) -> DomainResult<bool>;

    /// 获取房间活动统计
    async fn get_activity_stats(
        &self,
        room_id: Uuid,
        date_from: DateTime<Utc>,
        date_to: DateTime<Utc>,
    ) -> DomainResult<RoomActivityStats>;

    /// 清理不活跃的房间
    async fn cleanup_inactive_rooms(&self, inactive_days: u32) -> DomainResult<u64>;

    /// 根据组织查找聊天室（企业版功能）
    #[cfg(feature = "enterprise")]
    async fn find_by_organization(
        &self,
        org_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<ChatRoom>>;
}
