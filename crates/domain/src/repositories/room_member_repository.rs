//! 房间成员Repository接口定义

use crate::entities::room_member::{RoomMember, RoomMemberRole};
use crate::errors::DomainResult;
use crate::repositories::{Pagination, PaginatedResult, QueryFilter, SortConfig};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// 房间成员统计信息
#[derive(Debug, Clone)]
pub struct RoomMemberStatistics {
    pub total_members: u64,
    pub admins: u64,
    pub moderators: u64,
    pub regular_members: u64,
    pub muted_members: u64,
    pub online_members: u64,
    pub members_joined_today: u64,
}

/// 房间成员搜索参数
#[derive(Debug, Clone, Default)]
pub struct RoomMemberSearchParams {
    pub room_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub role: Option<RoomMemberRole>,
    pub is_muted: Option<bool>,
    pub notifications_enabled: Option<bool>,
    pub joined_after: Option<DateTime<Utc>>,
    pub joined_before: Option<DateTime<Utc>>,
}

/// 成员权限检查结果
#[derive(Debug, Clone)]
pub struct MemberPermissions {
    pub can_send_messages: bool,
    pub can_edit_messages: bool,
    pub can_delete_messages: bool,
    pub can_kick_members: bool,
    pub can_ban_members: bool,
    pub can_change_roles: bool,
    pub can_edit_room_info: bool,
    pub can_delete_room: bool,
}

/// 房间成员Repository接口
#[async_trait]
pub trait RoomMemberRepository: Send + Sync {
    /// 添加成员到房间
    async fn add_member(&self, member: &RoomMember) -> DomainResult<RoomMember>;

    /// 根据房间ID和用户ID查找成员
    async fn find_by_room_and_user(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<Option<RoomMember>>;

    /// 更新成员信息
    async fn update(&self, member: &RoomMember) -> DomainResult<RoomMember>;

    /// 更新成员角色
    async fn update_role(&self, room_id: Uuid, user_id: Uuid, role: RoomMemberRole) -> DomainResult<()>;

    /// 设置成员静音状态
    async fn set_muted(&self, room_id: Uuid, user_id: Uuid, is_muted: bool) -> DomainResult<()>;

    /// 设置通知状态
    async fn set_notifications(&self, room_id: Uuid, user_id: Uuid, enabled: bool) -> DomainResult<()>;

    /// 更新最后已读消息
    async fn update_last_read(&self, room_id: Uuid, user_id: Uuid, message_id: Uuid) -> DomainResult<()>;

    /// 移除房间成员
    async fn remove_member(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<bool>;

    /// 获取房间所有成员
    async fn find_by_room(&self, room_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<RoomMember>>;

    /// 获取用户参与的房间
    async fn find_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<RoomMember>>;

    /// 根据角色查找成员
    async fn find_by_room_and_role(
        &self,
        room_id: Uuid,
        role: RoomMemberRole,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<RoomMember>>;

    /// 检查用户是否是房间成员
    async fn is_member(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<bool>;

    /// 检查用户是否有特定权限
    async fn check_permission(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> DomainResult<bool>;

    /// 获取成员权限列表
    async fn get_member_permissions(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<MemberPermissions>;

    /// 统计房间成员数量
    async fn count_by_room(&self, room_id: Uuid) -> DomainResult<u64>;

    /// 统计在线成员数量
    async fn count_online_by_room(&self, room_id: Uuid) -> DomainResult<u64>;

    /// 获取房间管理员列表
    async fn find_admins_by_room(&self, room_id: Uuid) -> DomainResult<Vec<RoomMember>>;

    /// 获取房间版主列表
    async fn find_moderators_by_room(&self, room_id: Uuid) -> DomainResult<Vec<RoomMember>>;

    /// 批量添加成员
    async fn add_members_batch(&self, members: &[RoomMember]) -> DomainResult<Vec<RoomMember>>;

    /// 批量移除成员
    async fn remove_members_batch(&self, room_id: Uuid, user_ids: &[Uuid]) -> DomainResult<u64>;

    /// 根据条件搜索成员
    async fn search(
        &self,
        params: &RoomMemberSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<RoomMember>>;

    /// 获取成员统计信息
    async fn get_statistics(&self, room_id: Uuid) -> DomainResult<RoomMemberStatistics>;

    /// 获取未读消息数量
    async fn get_unread_count(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<u64>;

    /// 获取活跃成员列表（最近有消息的成员）
    async fn find_active_members(
        &self,
        room_id: Uuid,
        since: DateTime<Utc>,
        limit: u32,
    ) -> DomainResult<Vec<RoomMember>>;

    /// 更新成员权限
    async fn update_permissions(&self, room_id: Uuid, user_id: Uuid, permissions: &JsonValue) -> DomainResult<()>;

    /// 获取静音成员列表
    async fn find_muted_members(&self, room_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<RoomMember>>;

    /// 清理不活跃的成员记录
    async fn cleanup_inactive_members(&self, inactive_days: u32) -> DomainResult<u64>;
}