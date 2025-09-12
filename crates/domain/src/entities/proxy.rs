//! 代理关系管理
//!
//! 支持企业级的代理关系和权限委托功能

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 代理关系实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyRelationship {
    /// 代理关系ID
    pub id: Uuid,
    /// 委托人ID（被代理人）
    pub principal_user_id: Uuid,
    /// 代理人ID
    pub proxy_user_id: Uuid,
    /// 代理类型
    pub proxy_type: ProxyType,
    /// 代理权限范围
    pub permissions: Vec<String>,
    /// 生效时间
    pub effective_from: DateTime<Utc>,
    /// 失效时间（可选）
    pub effective_until: Option<DateTime<Utc>>,
    /// 代理状态
    pub status: ProxyStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 创建者ID
    pub created_by: Uuid,
    /// 备注信息
    pub notes: Option<String>,
}

/// 代理类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyType {
    /// 临时代理（短期）
    Temporary,
    /// 长期代理
    LongTerm,
    /// 紧急代理
    Emergency,
    /// 部分代理（指定权限）
    Partial,
    /// 全权代理
    Full,
}

impl ProxyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProxyType::Temporary => "temporary",
            ProxyType::LongTerm => "long_term",
            ProxyType::Emergency => "emergency",
            ProxyType::Partial => "partial",
            ProxyType::Full => "full",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "temporary" => Some(ProxyType::Temporary),
            "long_term" => Some(ProxyType::LongTerm),
            "emergency" => Some(ProxyType::Emergency),
            "partial" => Some(ProxyType::Partial),
            "full" => Some(ProxyType::Full),
            _ => None,
        }
    }
}

/// 代理状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyStatus {
    /// 等待激活
    Pending,
    /// 活跃
    Active,
    /// 暂停
    Suspended,
    /// 已过期
    Expired,
    /// 已撤销
    Revoked,
    /// 已删除
    Deleted,
}

impl ProxyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProxyStatus::Pending => "pending",
            ProxyStatus::Active => "active",
            ProxyStatus::Suspended => "suspended",
            ProxyStatus::Expired => "expired",
            ProxyStatus::Revoked => "revoked",
            ProxyStatus::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ProxyStatus::Pending),
            "active" => Some(ProxyStatus::Active),
            "suspended" => Some(ProxyStatus::Suspended),
            "expired" => Some(ProxyStatus::Expired),
            "revoked" => Some(ProxyStatus::Revoked),
            "deleted" => Some(ProxyStatus::Deleted),
            _ => None,
        }
    }
}

/// 代理操作记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyActivity {
    /// 记录ID
    pub id: Uuid,
    /// 代理关系ID
    pub proxy_relationship_id: Uuid,
    /// 操作类型
    pub action_type: ProxyActionType,
    /// 操作描述
    pub action_description: String,
    /// 操作时间
    pub performed_at: DateTime<Utc>,
    /// 操作者ID（代理人）
    pub performed_by: Uuid,
    /// 相关资源ID（可选）
    pub resource_id: Option<Uuid>,
    /// 操作结果
    pub result: ProxyActionResult,
    /// 元数据
    pub metadata: Option<String>,
}

/// 代理操作类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyActionType {
    /// 发送消息
    SendMessage,
    /// 创建聊天室
    CreateRoom,
    /// 修改聊天室
    ModifyRoom,
    /// 管理用户
    ManageUser,
    /// 查看报告
    ViewReport,
    /// 系统配置
    SystemConfig,
    /// 自定义操作
    Custom(String),
}

impl ProxyActionType {
    pub fn as_str(&self) -> String {
        match self {
            ProxyActionType::SendMessage => "send_message".to_string(),
            ProxyActionType::CreateRoom => "create_room".to_string(),
            ProxyActionType::ModifyRoom => "modify_room".to_string(),
            ProxyActionType::ManageUser => "manage_user".to_string(),
            ProxyActionType::ViewReport => "view_report".to_string(),
            ProxyActionType::SystemConfig => "system_config".to_string(),
            ProxyActionType::Custom(action) => action.clone(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "send_message" => ProxyActionType::SendMessage,
            "create_room" => ProxyActionType::CreateRoom,
            "modify_room" => ProxyActionType::ModifyRoom,
            "manage_user" => ProxyActionType::ManageUser,
            "view_report" => ProxyActionType::ViewReport,
            "system_config" => ProxyActionType::SystemConfig,
            _ => ProxyActionType::Custom(s.to_string()),
        }
    }
}

/// 代理操作结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyActionResult {
    /// 成功
    Success,
    /// 失败
    Failed,
    /// 权限不足
    InsufficientPermissions,
    /// 代理关系无效
    InvalidProxy,
}

impl ProxyActionResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProxyActionResult::Success => "success",
            ProxyActionResult::Failed => "failed",
            ProxyActionResult::InsufficientPermissions => "insufficient_permissions",
            ProxyActionResult::InvalidProxy => "invalid_proxy",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "success" => Some(ProxyActionResult::Success),
            "failed" => Some(ProxyActionResult::Failed),
            "insufficient_permissions" => Some(ProxyActionResult::InsufficientPermissions),
            "invalid_proxy" => Some(ProxyActionResult::InvalidProxy),
            _ => None,
        }
    }
}

/// 代理权限定义
pub mod proxy_permissions {
    /// 消息相关权限
    pub const MESSAGE_SEND: &str = "proxy:message:send";
    pub const MESSAGE_READ: &str = "proxy:message:read";
    pub const MESSAGE_DELETE: &str = "proxy:message:delete";

    /// 聊天室相关权限
    pub const ROOM_CREATE: &str = "proxy:room:create";
    pub const ROOM_MANAGE: &str = "proxy:room:manage";
    pub const ROOM_DELETE: &str = "proxy:room:delete";

    /// 用户管理权限
    pub const USER_MANAGE: &str = "proxy:user:manage";
    pub const USER_VIEW: &str = "proxy:user:view";

    /// 系统管理权限
    pub const SYSTEM_CONFIG: &str = "proxy:system:config";
    pub const SYSTEM_MONITOR: &str = "proxy:system:monitor";
}

impl ProxyRelationship {
    /// 创建新的代理关系
    pub fn new(
        principal_user_id: Uuid,
        proxy_user_id: Uuid,
        proxy_type: ProxyType,
        permissions: Vec<String>,
        effective_from: DateTime<Utc>,
        effective_until: Option<DateTime<Utc>>,
        created_by: Uuid,
        notes: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            principal_user_id,
            proxy_user_id,
            proxy_type,
            permissions,
            effective_from,
            effective_until,
            status: ProxyStatus::Pending,
            created_at: now,
            updated_at: now,
            created_by,
            notes,
        }
    }

    /// 检查代理关系是否有效
    pub fn is_valid(&self) -> bool {
        let now = Utc::now();

        // 检查状态
        if !matches!(self.status, ProxyStatus::Active) {
            return false;
        }

        // 检查生效时间
        if now < self.effective_from {
            return false;
        }

        // 检查失效时间
        if let Some(until) = self.effective_until {
            if now > until {
                return false;
            }
        }

        true
    }

    /// 检查代理关系是否已过期
    pub fn is_expired(&self) -> bool {
        if let Some(until) = self.effective_until {
            Utc::now() > until
        } else {
            false
        }
    }

    /// 激活代理关系
    pub fn activate(&mut self) {
        self.status = ProxyStatus::Active;
        self.updated_at = Utc::now();
    }

    /// 暂停代理关系
    pub fn suspend(&mut self) {
        self.status = ProxyStatus::Suspended;
        self.updated_at = Utc::now();
    }

    /// 撤销代理关系
    pub fn revoke(&mut self) {
        self.status = ProxyStatus::Revoked;
        self.updated_at = Utc::now();
    }

    /// 检查是否拥有特定权限
    pub fn has_permission(&self, permission: &str) -> bool {
        if !self.is_valid() {
            return false;
        }

        // 全权代理拥有所有权限
        if matches!(self.proxy_type, ProxyType::Full) {
            return true;
        }

        // 检查具体权限
        self.permissions.contains(&permission.to_string())
    }

    /// 添加权限
    pub fn add_permission(&mut self, permission: String) {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
            self.updated_at = Utc::now();
        }
    }

    /// 移除权限
    pub fn remove_permission(&mut self, permission: &str) {
        self.permissions.retain(|p| p != permission);
        self.updated_at = Utc::now();
    }

    /// 扩展代理期限
    pub fn extend_validity(&mut self, new_until: DateTime<Utc>) {
        self.effective_until = Some(new_until);
        self.updated_at = Utc::now();
    }

    /// 更新备注
    pub fn update_notes(&mut self, notes: Option<String>) {
        self.notes = notes;
        self.updated_at = Utc::now();
    }
}

impl ProxyActivity {
    /// 创建新的代理活动记录
    pub fn new(
        proxy_relationship_id: Uuid,
        action_type: ProxyActionType,
        action_description: String,
        performed_by: Uuid,
        resource_id: Option<Uuid>,
        result: ProxyActionResult,
        metadata: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            proxy_relationship_id,
            action_type,
            action_description,
            performed_at: Utc::now(),
            performed_by,
            resource_id,
            result,
            metadata,
        }
    }

    /// 记录成功的代理操作
    pub fn success(
        proxy_relationship_id: Uuid,
        action_type: ProxyActionType,
        action_description: String,
        performed_by: Uuid,
        resource_id: Option<Uuid>,
    ) -> Self {
        Self::new(
            proxy_relationship_id,
            action_type,
            action_description,
            performed_by,
            resource_id,
            ProxyActionResult::Success,
            None,
        )
    }

    /// 记录失败的代理操作
    pub fn failure(
        proxy_relationship_id: Uuid,
        action_type: ProxyActionType,
        action_description: String,
        performed_by: Uuid,
        resource_id: Option<Uuid>,
        error_message: Option<String>,
    ) -> Self {
        Self::new(
            proxy_relationship_id,
            action_type,
            action_description,
            performed_by,
            resource_id,
            ProxyActionResult::Failed,
            error_message,
        )
    }
}

/// 代理服务接口
pub trait ProxyService {
    /// 创建代理关系
    fn create_proxy_relationship(
        &self,
        principal_user_id: Uuid,
        proxy_user_id: Uuid,
        proxy_type: ProxyType,
        permissions: Vec<String>,
        effective_from: DateTime<Utc>,
        effective_until: Option<DateTime<Utc>>,
        created_by: Uuid,
        notes: Option<String>,
    ) -> Result<ProxyRelationship, Box<dyn std::error::Error>>;

    /// 检查用户是否可以代理某个操作
    fn can_proxy_action(&self, proxy_user_id: Uuid, principal_user_id: Uuid, action: &str) -> bool;

    /// 获取用户的代理关系列表
    fn get_proxy_relationships(&self, user_id: Uuid) -> Vec<ProxyRelationship>;

    /// 记录代理活动
    fn record_proxy_activity(&self, activity: ProxyActivity);

    /// 获取代理活动记录
    fn get_proxy_activities(&self, proxy_relationship_id: Uuid) -> Vec<ProxyActivity>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_relationship_creation() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let created_by = Uuid::new_v4();
        let permissions = vec![proxy_permissions::MESSAGE_SEND.to_string()];

        let proxy_rel = ProxyRelationship::new(
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions,
            Utc::now(),
            Some(Utc::now() + chrono::Duration::days(7)),
            created_by,
            Some("Temporary delegation for vacation".to_string()),
        );

        assert_eq!(proxy_rel.principal_user_id, principal_id);
        assert_eq!(proxy_rel.proxy_user_id, proxy_id);
        assert_eq!(proxy_rel.proxy_type, ProxyType::Temporary);
        assert_eq!(proxy_rel.status, ProxyStatus::Pending);
    }

    #[test]
    fn test_proxy_validity() {
        let mut proxy_rel = ProxyRelationship::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ProxyType::Temporary,
            vec![],
            Utc::now() - chrono::Duration::minutes(10), // 10分钟前生效
            Some(Utc::now() + chrono::Duration::hours(1)), // 1小时后过期
            Uuid::new_v4(),
            None,
        );

        // 初始状态是Pending，应该无效
        assert!(!proxy_rel.is_valid());

        // 激活后应该有效
        proxy_rel.activate();
        assert!(proxy_rel.is_valid());

        // 暂停后应该无效
        proxy_rel.suspend();
        assert!(!proxy_rel.is_valid());
    }

    #[test]
    fn test_proxy_permissions() {
        let mut proxy_rel = ProxyRelationship::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ProxyType::Partial,
            vec![proxy_permissions::MESSAGE_SEND.to_string()],
            Utc::now(),
            None,
            Uuid::new_v4(),
            None,
        );

        proxy_rel.activate();

        assert!(proxy_rel.has_permission(proxy_permissions::MESSAGE_SEND));
        assert!(!proxy_rel.has_permission(proxy_permissions::ROOM_CREATE));

        // 添加权限
        proxy_rel.add_permission(proxy_permissions::ROOM_CREATE.to_string());
        assert!(proxy_rel.has_permission(proxy_permissions::ROOM_CREATE));
    }

    #[test]
    fn test_full_proxy_permissions() {
        let mut proxy_rel = ProxyRelationship::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            ProxyType::Full,
            vec![], // 即使没有具体权限列表
            Utc::now(),
            None,
            Uuid::new_v4(),
            None,
        );

        proxy_rel.activate();

        // 全权代理应该拥有所有权限
        assert!(proxy_rel.has_permission(proxy_permissions::MESSAGE_SEND));
        assert!(proxy_rel.has_permission(proxy_permissions::ROOM_CREATE));
        assert!(proxy_rel.has_permission(proxy_permissions::SYSTEM_CONFIG));
        assert!(proxy_rel.has_permission("any_custom_permission"));
    }

    #[test]
    fn test_proxy_activity_creation() {
        let proxy_rel_id = Uuid::new_v4();
        let performed_by = Uuid::new_v4();
        let resource_id = Some(Uuid::new_v4());

        let activity = ProxyActivity::success(
            proxy_rel_id,
            ProxyActionType::SendMessage,
            "Sent message on behalf of principal".to_string(),
            performed_by,
            resource_id,
        );

        assert_eq!(activity.proxy_relationship_id, proxy_rel_id);
        assert_eq!(activity.action_type, ProxyActionType::SendMessage);
        assert_eq!(activity.result, ProxyActionResult::Success);
    }
}
