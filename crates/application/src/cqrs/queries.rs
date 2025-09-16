//! 查询定义
//!
//! 包含用户、聊天室和组织相关的查询

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use domain::user::UserStatus;
use domain::message::MessageType;
use super::{Query, dtos::*};

// ============================================================================
// 用户查询
// ============================================================================

/// 根据ID查询用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserByIdQuery {
    pub user_id: Uuid,
}

impl Query for GetUserByIdQuery {
    type Result = Option<UserDto>;
}

/// 根据邮箱查询用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserByEmailQuery {
    pub email: String,
}

impl Query for GetUserByEmailQuery {
    type Result = Option<UserDto>;
}

/// 根据用户名查询用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserByUsernameQuery {
    pub username: String,
}

impl Query for GetUserByUsernameQuery {
    type Result = Option<UserDto>;
}

/// 搜索用户查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchUsersQuery {
    pub keyword: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub status_filter: Option<UserStatus>,
}

impl Query for SearchUsersQuery {
    type Result = Vec<UserDto>;
}

/// 获取用户统计信息查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserStatsQuery;

impl Query for GetUserStatsQuery {
    type Result = UserStatisticsDto;
}

/// 获取用户详细资料查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserProfileQuery {
    pub user_id: Uuid,
}

impl Query for GetUserProfileQuery {
    type Result = Option<UserProfileDto>;
}

// ============================================================================
// 聊天室查询
// ============================================================================

/// 获取用户加入的聊天室列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserRoomsQuery {
    pub user_id: Uuid,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Query for GetUserRoomsQuery {
    type Result = Vec<ChatRoomDto>;
}

/// 根据ID获取聊天室
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChatRoomByIdQuery {
    pub room_id: Uuid,
}

impl Query for GetChatRoomByIdQuery {
    type Result = Option<ChatRoomDto>;
}

/// 获取聊天室详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetChatRoomDetailQuery {
    pub room_id: Uuid,
}

impl Query for GetChatRoomDetailQuery {
    type Result = Option<ChatRoomDetailDto>;
}

/// 获取聊天室成员列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRoomMembersQuery {
    pub room_id: Uuid,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Query for GetRoomMembersQuery {
    type Result = Vec<RoomMemberDto>;
}

/// 获取聊天室消息历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRoomMessagesQuery {
    pub room_id: Uuid,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
}

impl Query for GetRoomMessagesQuery {
    type Result = Vec<MessageDto>;
}

/// 搜索消息查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMessagesQuery {
    pub room_id: Option<Uuid>,
    pub keyword: String,
    pub message_type: Option<MessageType>,
    pub user_id: Option<Uuid>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Query for SearchMessagesQuery {
    type Result = Vec<MessageDto>;
}

/// 获取消息详情查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMessageByIdQuery {
    pub message_id: Uuid,
}

impl Query for GetMessageByIdQuery {
    type Result = Option<MessageDto>;
}

/// 获取消息回复线程查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMessageThreadQuery {
    pub root_message_id: Uuid,
}

impl Query for GetMessageThreadQuery {
    type Result = Option<MessageThreadDto>;
}

/// 搜索公共聊天室查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchPublicRoomsQuery {
    pub keyword: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Query for SearchPublicRoomsQuery {
    type Result = Vec<ChatRoomDto>;
}

// ============================================================================
// 组织查询（企业级功能）
// ============================================================================

/// 获取用户的组织列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserOrganizationsQuery {
    pub user_id: Uuid,
    pub include_details: bool,
}

impl Query for GetUserOrganizationsQuery {
    type Result = Vec<OrganizationDto>;
}

/// 根据ID获取组织
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOrganizationByIdQuery {
    pub organization_id: Uuid,
}

impl Query for GetOrganizationByIdQuery {
    type Result = Option<OrganizationDto>;
}

/// 获取组织成员列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOrganizationMembersQuery {
    pub organization_id: Uuid,
    pub department_id: Option<Uuid>,
    pub role_id: Option<Uuid>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Query for GetOrganizationMembersQuery {
    type Result = Vec<OrganizationMemberDto>;
}

/// 获取组织部门列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOrganizationDepartmentsQuery {
    pub organization_id: Uuid,
    pub parent_id: Option<Uuid>,
}

impl Query for GetOrganizationDepartmentsQuery {
    type Result = Vec<DepartmentDto>;
}

/// 获取组织角色列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOrganizationRolesQuery {
    pub organization_id: Uuid,
}

impl Query for GetOrganizationRolesQuery {
    type Result = Vec<RoleDto>;
}

/// 获取用户在组织中的权限
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserPermissionsInOrganizationQuery {
    pub user_id: Uuid,
    pub organization_id: Uuid,
}

impl Query for GetUserPermissionsInOrganizationQuery {
    type Result = Vec<PermissionDto>;
}

/// 搜索组织查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOrganizationsQuery {
    pub keyword: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Query for SearchOrganizationsQuery {
    type Result = Vec<OrganizationDto>;
}