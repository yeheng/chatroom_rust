//! 命令定义
//!
//! 包含用户管理、聊天室管理和组织管理的所有命令

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use domain::user::UserStatus;
use domain::message::MessageType;
// use domain::organization::OrganizationSettings;
use super::Command;

// ============================================================================
// 用户管理命令
// ============================================================================

/// 用户注册命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterUserCommand {
    pub username: String,
    pub email: String,
    pub password: String,
    pub avatar_url: Option<String>,
    pub display_name: Option<String>,
}

impl Command for RegisterUserCommand {
    type Result = domain::user::User;
}

/// 用户登录命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginUserCommand {
    pub email: String,
    pub password: String,
}

impl Command for LoginUserCommand {
    type Result = domain::user::User;
}

/// 更新用户信息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserCommand {
    pub user_id: Uuid,
    pub username: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl Command for UpdateUserCommand {
    type Result = domain::user::User;
}

/// 更新用户状态命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserStatusCommand {
    pub user_id: Uuid,
    pub status: UserStatus,
}

impl Command for UpdateUserStatusCommand {
    type Result = ();
}

/// 删除用户命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteUserCommand {
    pub user_id: Uuid,
}

impl Command for DeleteUserCommand {
    type Result = ();
}

// ============================================================================
// 聊天室管理命令
// ============================================================================

/// 创建聊天室命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChatRoomCommand {
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_private: bool,
    pub password: Option<String>,
    pub max_members: Option<u32>,
}

impl Command for CreateChatRoomCommand {
    type Result = domain::entities::chatroom::ChatRoom;
}

/// 加入聊天室命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinChatRoomCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub password: Option<String>,
}

impl Command for JoinChatRoomCommand {
    type Result = ();
}

/// 离开聊天室命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveChatRoomCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
}

impl Command for LeaveChatRoomCommand {
    type Result = ();
}

/// 发送消息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to_message_id: Option<Uuid>,
}

impl Command for SendMessageCommand {
    type Result = domain::message::Message;
}

/// 更新消息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMessageCommand {
    pub message_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
}

impl Command for UpdateMessageCommand {
    type Result = domain::message::Message;
}

/// 删除消息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteMessageCommand {
    pub message_id: Uuid,
    pub user_id: Uuid,
}

impl Command for DeleteMessageCommand {
    type Result = ();
}

/// 更新聊天室设置命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChatRoomCommand {
    pub room_id: Uuid,
    pub user_id: Uuid, // 执行操作的用户ID，用于权限验证
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_private: Option<bool>,
    pub max_members: Option<u32>,
}

impl Command for UpdateChatRoomCommand {
    type Result = domain::entities::chatroom::ChatRoom;
}

/// 删除聊天室命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteChatRoomCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
}

impl Command for DeleteChatRoomCommand {
    type Result = ();
}

// ============================================================================
// 组织管理命令（企业级功能）
// ============================================================================

/// 创建组织命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrganizationCommand {
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub settings: Option<serde_json::Value>,
}

impl Command for CreateOrganizationCommand {
    type Result = domain::organization::Organization;
}

/// 添加用户到组织命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddUserToOrganizationCommand {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
}

impl Command for AddUserToOrganizationCommand {
    type Result = ();
}

/// 移除用户从组织命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveUserFromOrganizationCommand {
    pub organization_id: Uuid,
    pub user_id: Uuid,
}

impl Command for RemoveUserFromOrganizationCommand {
    type Result = ();
}

/// 更新用户在组织中的角色命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRoleInOrganizationCommand {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
}

impl Command for UpdateUserRoleInOrganizationCommand {
    type Result = ();
}

/// 创建部门命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDepartmentCommand {
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_department_id: Option<Uuid>,
    pub manager_id: Option<Uuid>,
}

impl Command for CreateDepartmentCommand {
    type Result = domain::entities::department::Department;
}

/// 更新组织设置命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateOrganizationCommand {
    pub organization_id: Uuid,
    pub owner_id: Uuid, // 执行操作的用户ID，用于权限验证
    pub name: Option<String>,
    pub description: Option<String>,
    pub settings: Option<serde_json::Value>,
}

impl Command for UpdateOrganizationCommand {
    type Result = domain::organization::Organization;
}

/// 删除组织命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteOrganizationCommand {
    pub organization_id: Uuid,
    pub owner_id: Uuid, // 执行操作的用户ID，用于权限验证
}

impl Command for DeleteOrganizationCommand {
    type Result = ();
}