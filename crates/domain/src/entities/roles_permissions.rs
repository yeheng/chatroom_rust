//! 用户角色和权限系统
//!
//! 提供细粒度的权限控制和角色管理功能

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// 权限实体
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Permission {
    /// 权限ID
    pub id: Uuid,
    /// 权限名称
    pub name: String,
    /// 权限描述
    pub description: Option<String>,
    /// 权限资源
    pub resource: String,
    /// 权限操作
    pub action: String,
    /// 权限状态
    pub status: PermissionStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 权限状态
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionStatus {
    /// 活跃
    Active,
    /// 已禁用
    Disabled,
    /// 已删除
    Deleted,
}

impl PermissionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionStatus::Active => "active",
            PermissionStatus::Disabled => "disabled",
            PermissionStatus::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(PermissionStatus::Active),
            "disabled" => Some(PermissionStatus::Disabled),
            "deleted" => Some(PermissionStatus::Deleted),
            _ => None,
        }
    }
}

/// 角色实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// 角色ID
    pub id: Uuid,
    /// 角色名称
    pub name: String,
    /// 角色描述
    pub description: Option<String>,
    /// 角色类型
    pub role_type: RoleType,
    /// 所属组织ID（可选，全局角色为None）
    pub organization_id: Option<Uuid>,
    /// 角色状态
    pub status: RoleStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 角色类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleType {
    /// 系统角色（预定义）
    System,
    /// 组织角色
    Organization,
    /// 自定义角色
    Custom,
}

impl RoleType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoleType::System => "system",
            RoleType::Organization => "organization",
            RoleType::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "system" => Some(RoleType::System),
            "organization" => Some(RoleType::Organization),
            "custom" => Some(RoleType::Custom),
            _ => None,
        }
    }
}

/// 角色状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleStatus {
    /// 活跃
    Active,
    /// 已禁用
    Disabled,
    /// 已删除
    Deleted,
}

impl RoleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoleStatus::Active => "active",
            RoleStatus::Disabled => "disabled",
            RoleStatus::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(RoleStatus::Active),
            "disabled" => Some(RoleStatus::Disabled),
            "deleted" => Some(RoleStatus::Deleted),
            _ => None,
        }
    }
}

/// 角色权限关联
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RolePermission {
    /// 角色ID
    pub role_id: Uuid,
    /// 权限ID
    pub permission_id: Uuid,
    /// 授权时间
    pub granted_at: DateTime<Utc>,
    /// 授权者ID
    pub granted_by: Uuid,
}

/// 用户角色关联
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRole {
    /// 用户ID
    pub user_id: Uuid,
    /// 角色ID
    pub role_id: Uuid,
    /// 分配时间
    pub assigned_at: DateTime<Utc>,
    /// 分配者ID
    pub assigned_by: Uuid,
    /// 到期时间（可选）
    pub expires_at: Option<DateTime<Utc>>,
    /// 状态
    pub status: UserRoleStatus,
}

/// 用户角色状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRoleStatus {
    /// 活跃
    Active,
    /// 已过期
    Expired,
    /// 已撤销
    Revoked,
    /// 暂停
    Suspended,
}

impl UserRoleStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRoleStatus::Active => "active",
            UserRoleStatus::Expired => "expired",
            UserRoleStatus::Revoked => "revoked",
            UserRoleStatus::Suspended => "suspended",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(UserRoleStatus::Active),
            "expired" => Some(UserRoleStatus::Expired),
            "revoked" => Some(UserRoleStatus::Revoked),
            "suspended" => Some(UserRoleStatus::Suspended),
            _ => None,
        }
    }
}

/// 系统预定义权限
pub mod system_permissions {
    /// 用户管理权限
    pub const USER_CREATE: &str = "user:create";
    pub const USER_READ: &str = "user:read";
    pub const USER_UPDATE: &str = "user:update";
    pub const USER_DELETE: &str = "user:delete";

    /// 聊天室管理权限
    pub const ROOM_CREATE: &str = "room:create";
    pub const ROOM_READ: &str = "room:read";
    pub const ROOM_UPDATE: &str = "room:update";
    pub const ROOM_DELETE: &str = "room:delete";
    pub const ROOM_JOIN: &str = "room:join";
    pub const ROOM_LEAVE: &str = "room:leave";

    /// 消息管理权限
    pub const MESSAGE_SEND: &str = "message:send";
    pub const MESSAGE_READ: &str = "message:read";
    pub const MESSAGE_UPDATE: &str = "message:update";
    pub const MESSAGE_DELETE: &str = "message:delete";

    /// 组织管理权限（企业功能）
    pub const ORGANIZATION_CREATE: &str = "organization:create";
    pub const ORGANIZATION_READ: &str = "organization:read";
    pub const ORGANIZATION_UPDATE: &str = "organization:update";
    pub const ORGANIZATION_DELETE: &str = "organization:delete";

    /// 角色管理权限
    pub const ROLE_CREATE: &str = "role:create";
    pub const ROLE_READ: &str = "role:read";
    pub const ROLE_UPDATE: &str = "role:update";
    pub const ROLE_DELETE: &str = "role:delete";
    pub const ROLE_ASSIGN: &str = "role:assign";

    /// 系统管理权限
    pub const SYSTEM_ADMIN: &str = "system:admin";
    pub const SYSTEM_CONFIG: &str = "system:config";
    pub const SYSTEM_MONITOR: &str = "system:monitor";
}

/// 系统预定义角色
pub mod system_roles {
    /// 系统管理员
    pub const SYSTEM_ADMIN: &str = "system_admin";
    /// 组织管理员
    pub const ORGANIZATION_ADMIN: &str = "organization_admin";
    /// 普通用户
    pub const USER: &str = "user";
    /// 访客
    pub const GUEST: &str = "guest";
}

impl Permission {
    /// 创建新的权限
    pub fn new(
        name: String,
        description: Option<String>,
        resource: String,
        action: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            resource,
            action,
            status: PermissionStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// 创建系统权限
    pub fn system_permission(resource: &str, action: &str) -> Self {
        let name = format!("{}:{}", resource, action);
        let description = Some(format!("System permission for {} {}", action, resource));
        Self::new(name, description, resource.to_string(), action.to_string())
    }

    /// 获取权限标识符
    pub fn identifier(&self) -> String {
        format!("{}:{}", self.resource, self.action)
    }

    /// 禁用权限
    pub fn disable(&mut self) {
        self.status = PermissionStatus::Disabled;
        self.updated_at = Utc::now();
    }

    /// 启用权限
    pub fn enable(&mut self) {
        self.status = PermissionStatus::Active;
        self.updated_at = Utc::now();
    }
}

impl Role {
    /// 创建新的角色
    pub fn new(
        name: String,
        description: Option<String>,
        role_type: RoleType,
        organization_id: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            role_type,
            organization_id,
            status: RoleStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// 创建系统角色
    pub fn system_role(name: String, description: Option<String>) -> Self {
        Self::new(name, description, RoleType::System, None)
    }

    /// 创建组织角色
    pub fn organization_role(
        name: String,
        description: Option<String>,
        organization_id: Uuid,
    ) -> Self {
        Self::new(
            name,
            description,
            RoleType::Organization,
            Some(organization_id),
        )
    }

    /// 检查是否为系统角色
    pub fn is_system_role(&self) -> bool {
        matches!(self.role_type, RoleType::System)
    }

    /// 检查是否为组织角色
    pub fn is_organization_role(&self) -> bool {
        matches!(self.role_type, RoleType::Organization)
    }

    /// 更新角色信息
    pub fn update(&mut self, name: Option<String>, description: Option<String>) {
        if let Some(name) = name {
            self.name = name;
        }
        self.description = description;
        self.updated_at = Utc::now();
    }

    /// 禁用角色
    pub fn disable(&mut self) {
        self.status = RoleStatus::Disabled;
        self.updated_at = Utc::now();
    }

    /// 启用角色
    pub fn enable(&mut self) {
        self.status = RoleStatus::Active;
        self.updated_at = Utc::now();
    }
}

impl UserRole {
    /// 创建新的用户角色关联
    pub fn new(
        user_id: Uuid,
        role_id: Uuid,
        assigned_by: Uuid,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            user_id,
            role_id,
            assigned_at: Utc::now(),
            assigned_by,
            expires_at,
            status: UserRoleStatus::Active,
        }
    }

    /// 检查角色是否已过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// 检查角色是否有效
    pub fn is_valid(&self) -> bool {
        matches!(self.status, UserRoleStatus::Active) && !self.is_expired()
    }

    /// 撤销角色
    pub fn revoke(&mut self) {
        self.status = UserRoleStatus::Revoked;
    }

    /// 暂停角色
    pub fn suspend(&mut self) {
        self.status = UserRoleStatus::Suspended;
    }

    /// 恢复角色
    pub fn restore(&mut self) {
        self.status = UserRoleStatus::Active;
    }
}

/// 权限检查结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionCheckResult {
    /// 允许
    Allowed,
    /// 拒绝
    Denied,
    /// 权限不存在
    NotFound,
    /// 角色已过期
    Expired,
}

/// 用户权限上下文
#[derive(Debug, Clone)]
pub struct UserPermissionContext {
    /// 用户ID
    pub user_id: Uuid,
    /// 用户角色列表
    pub roles: Vec<Role>,
    /// 用户权限列表（通过角色获得的权限）
    pub permissions: HashSet<String>,
    /// 组织ID（可选）
    pub organization_id: Option<Uuid>,
}

impl UserPermissionContext {
    /// 创建新的用户权限上下文
    pub fn new(
        user_id: Uuid,
        roles: Vec<Role>,
        permissions: HashSet<String>,
        organization_id: Option<Uuid>,
    ) -> Self {
        Self {
            user_id,
            roles,
            permissions,
            organization_id,
        }
    }

    /// 检查用户是否拥有指定权限
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(permission)
    }

    /// 检查用户是否拥有指定角色
    pub fn has_role(&self, role_name: &str) -> bool {
        self.roles.iter().any(|role| role.name == role_name)
    }

    /// 检查用户是否为系统管理员
    pub fn is_system_admin(&self) -> bool {
        self.has_role(system_roles::SYSTEM_ADMIN)
    }

    /// 检查用户是否为组织管理员
    pub fn is_organization_admin(&self) -> bool {
        self.has_role(system_roles::ORGANIZATION_ADMIN)
    }

    /// 获取用户在指定组织的角色
    pub fn get_organization_roles(&self, organization_id: Uuid) -> Vec<&Role> {
        self.roles
            .iter()
            .filter(|role| role.organization_id == Some(organization_id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_creation() {
        let permission = Permission::new(
            "test_permission".to_string(),
            Some("Test permission".to_string()),
            "test_resource".to_string(),
            "read".to_string(),
        );

        assert_eq!(permission.name, "test_permission");
        assert_eq!(permission.identifier(), "test_resource:read");
        assert_eq!(permission.status, PermissionStatus::Active);
    }

    #[test]
    fn test_role_creation() {
        let role = Role::system_role("test_role".to_string(), Some("Test role".to_string()));

        assert_eq!(role.name, "test_role");
        assert_eq!(role.role_type, RoleType::System);
        assert!(role.is_system_role());
        assert!(!role.is_organization_role());
    }

    #[test]
    fn test_user_role_expiration() {
        let user_id = Uuid::new_v4();
        let role_id = Uuid::new_v4();
        let assigned_by = Uuid::new_v4();
        let expires_at = Some(Utc::now() - chrono::Duration::hours(1)); // 1小时前过期

        let user_role = UserRole::new(user_id, role_id, assigned_by, expires_at);

        assert!(user_role.is_expired());
        assert!(!user_role.is_valid());
    }

    #[test]
    fn test_user_permission_context() {
        let user_id = Uuid::new_v4();
        let role = Role::system_role(
            system_roles::SYSTEM_ADMIN.to_string(),
            Some("System Administrator".to_string()),
        );

        let mut permissions = HashSet::new();
        permissions.insert(system_permissions::SYSTEM_ADMIN.to_string());
        permissions.insert(system_permissions::USER_CREATE.to_string());

        let context = UserPermissionContext::new(user_id, vec![role], permissions, None);

        assert!(context.has_permission(system_permissions::SYSTEM_ADMIN));
        assert!(context.has_permission(system_permissions::USER_CREATE));
        assert!(!context.has_permission(system_permissions::ROOM_DELETE));
        assert!(context.is_system_admin());
        assert!(context.has_role(system_roles::SYSTEM_ADMIN));
    }
}
