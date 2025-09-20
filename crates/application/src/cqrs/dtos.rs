//! 数据传输对象（DTO）定义
//!
//! 包含应用层与外界交互的所有数据传输对象

use chrono::{DateTime, Utc};
use domain::entities::room_member::MemberRole;
use domain::message::MessageType;
use domain::user::UserStatus;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
// use domain::organization::OrganizationSettings;

// ============================================================================
// 用户相关 DTO
// ============================================================================

/// 用户数据传输对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_activity_at: Option<DateTime<Utc>>,
}

/// 用户详细资料 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileDto {
    pub user: UserDto,
    pub organizations: Vec<OrganizationDto>,
    pub rooms: Vec<ChatRoomDto>,
    pub statistics: UserStatisticsDto,
}

/// 用户统计信息 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStatisticsDto {
    pub total_rooms_created: u32,
    pub total_messages_sent: u32,
    pub total_organizations_joined: u32,
    pub last_active_at: DateTime<Utc>,
    pub total_users: u64,
    pub active_users: u64,
    pub online_users: u64,
}

/// 认证响应 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponseDto {
    pub user: UserDto,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

/// 用户会话信息 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSessionDto {
    pub user_id: Uuid,
    pub access_token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

// ============================================================================
// 聊天室相关 DTO
// ============================================================================

/// 聊天室数据传输对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoomDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_private: bool,
    pub member_count: u32,
    pub max_members: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 聊天室详细信息 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoomDetailDto {
    pub room: ChatRoomDto,
    pub members: Vec<RoomMemberDto>,
    pub recent_messages: Vec<MessageDto>,
    pub settings: RoomSettingsDto,
}

/// 房间成员 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMemberDto {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
}

/// 房间设置 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSettingsDto {
    pub allow_guests: bool,
    pub message_retention_days: Option<u32>,
    pub max_message_length: u32,
    pub file_upload_enabled: bool,
    pub announcement: Option<String>,
}

// ============================================================================
// 消息相关 DTO
// ============================================================================

/// 消息数据传输对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDto {
    pub id: Uuid,
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to_message_id: Option<Uuid>,
    pub reply_to_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub edited: bool,
}

/// 消息线程 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageThreadDto {
    pub root_message: MessageDto,
    pub replies: Vec<MessageDto>,
    pub total_replies: u32,
}

// ============================================================================
// 组织相关 DTO（企业级功能）
// ============================================================================

/// 组织数据传输对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub settings: serde_json::Value,
    pub member_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 组织成员 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMemberDto {
    pub user_id: Uuid,
    pub username: String,
    pub display_name: Option<String>,
    pub email: String,
    pub avatar_url: Option<String>,
    pub role: RoleDto,
    pub department: Option<DepartmentDto>,
    pub position: Option<PositionDto>,
    pub joined_at: DateTime<Utc>,
    pub status: UserStatus,
}

/// 部门 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentDto {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_department_id: Option<Uuid>,
    pub manager_id: Option<Uuid>,
    pub level: u8,
    pub member_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 职位 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionDto {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub level: u8,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 角色 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDto {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub name: String,
    pub description: Option<String>,
    pub role_type: String, // 系统角色、组织角色、自定义角色
    pub permissions: Vec<PermissionDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 权限 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDto {
    pub id: Uuid,
    pub name: String,
    pub resource: String,
    pub action: String,
    pub description: Option<String>,
}

/// 用户角色关系 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRoleDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub role_id: Uuid,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Uuid,
}

/// 代理关系 DTO（企业级功能）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRelationshipDto {
    pub id: Uuid,
    pub principal_id: Uuid,
    pub proxy_id: Uuid,
    pub organization_id: Uuid,
    pub proxy_type: String,
    pub start_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
    pub permissions: Vec<PermissionDto>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 机器人 DTO（企业级功能）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub bot_type: String,
    pub configuration: serde_json::Value,
    pub organization_id: Option<Uuid>,
    pub owner_id: Uuid,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 在线时长统计 DTO（企业级功能）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineTimeDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: chrono::NaiveDate,
    pub total_seconds: u32,
    pub session_count: u32,
    pub first_login: DateTime<Utc>,
    pub last_logout: Option<DateTime<Utc>>,
    pub device_type: Option<String>,
    pub ip_address: Option<String>,
}

// ============================================================================
// 请求/响应 DTO
// ============================================================================

/// 创建用户请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequestDto {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// 用户登录请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequestDto {
    pub email: String,
    pub password: String,
}

/// 创建聊天室请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChatRoomRequestDto {
    pub name: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub password: Option<String>,
    pub max_members: Option<u32>,
}

/// 发送消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequestDto {
    pub room_id: Uuid,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to_message_id: Option<Uuid>,
}

/// 搜索消息请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMessagesRequestDto {
    pub room_id: Option<Uuid>,
    pub keyword: String,
    pub message_type: Option<MessageType>,
    pub user_id: Option<Uuid>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// 创建组织请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrganizationRequestDto {
    pub name: String,
    pub description: Option<String>,
    pub settings: Option<serde_json::Value>,
}

/// 添加用户到组织请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddUserToOrganizationRequestDto {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
}

/// 组织成员过滤条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMemberFiltersDto {
    pub department_id: Option<Uuid>,
    pub role_id: Option<Uuid>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
