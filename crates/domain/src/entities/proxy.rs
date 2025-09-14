//! 代理关系管理
//!
//! 支持企业级的代理关系和权限委托功能

use crate::entities::roles_permissions::Permission;
use crate::errors::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 代理关系实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProxy {
    /// 代理关系ID
    pub id: Uuid,
    /// 委托人ID（被代理人）
    pub principal_id: Uuid,
    /// 代理人ID
    pub proxy_id: Uuid,
    /// 代理类型
    pub proxy_type: ProxyType,
    /// 代理权限
    pub permissions: Vec<Permission>,
    /// 开始日期
    pub start_date: DateTime<Utc>,
    /// 结束日期（可选）
    pub end_date: Option<DateTime<Utc>>,
    /// 是否激活
    pub is_active: bool,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

/// 代理类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyType {
    /// 临时代理
    Temporary,
    /// 永久代理
    Permanent,
    /// 紧急代理
    Emergency,
    /// 休假代理
    Vacation,
}

impl ProxyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProxyType::Temporary => "temporary",
            ProxyType::Permanent => "permanent",
            ProxyType::Emergency => "emergency",
            ProxyType::Vacation => "vacation",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "temporary" => Some(ProxyType::Temporary),
            "permanent" => Some(ProxyType::Permanent),
            "emergency" => Some(ProxyType::Emergency),
            "vacation" => Some(ProxyType::Vacation),
            _ => None,
        }
    }
}

/// 代理操作枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyAction {
    /// 加入房间
    JoinRoom {
        room_id: Uuid,
        password: Option<String>,
    },
    /// 发送消息
    SendMessage { room_id: Uuid, content: String },
    /// 离开房间
    LeaveRoom { room_id: Uuid },
    /// 创建房间
    CreateRoom {
        name: String,
        description: Option<String>,
    },
}

impl UserProxy {
    /// 创建新的代理关系
    pub fn new(
        principal_id: Uuid,
        proxy_id: Uuid,
        proxy_type: ProxyType,
        permissions: Vec<Permission>,
    ) -> DomainResult<Self> {
        if principal_id == proxy_id {
            return Err(DomainError::business_rule_violation(
                "委托人和代理人不能为同一用户",
            ));
        }

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            principal_id,
            proxy_id,
            proxy_type,
            permissions,
            start_date: now,
            end_date: None,
            is_active: true,
            created_at: now,
        })
    }

    /// 创建具有指定日期的代理关系（用于从数据库加载）
    pub fn with_dates(
        id: Uuid,
        principal_id: Uuid,
        proxy_id: Uuid,
        proxy_type: ProxyType,
        permissions: Vec<Permission>,
        start_date: DateTime<Utc>,
        end_date: Option<DateTime<Utc>>,
        is_active: bool,
        created_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        if principal_id == proxy_id {
            return Err(DomainError::business_rule_violation(
                "委托人和代理人不能为同一用户",
            ));
        }

        // 验证日期逻辑
        if let Some(end_date) = end_date {
            if end_date <= start_date {
                return Err(DomainError::validation_error(
                    "end_date",
                    "结束日期必须晚于开始日期",
                ));
            }
        }

        Ok(Self {
            id,
            principal_id,
            proxy_id,
            proxy_type,
            permissions,
            start_date,
            end_date,
            is_active,
            created_at,
        })
    }

    /// 激活代理关系
    pub fn activate(&mut self) {
        self.is_active = true;
    }

    /// 停用代理关系
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// 设置结束日期
    pub fn set_end_date(&mut self, end_date: DateTime<Utc>) -> DomainResult<()> {
        if end_date <= self.start_date {
            return Err(DomainError::validation_error(
                "end_date",
                "结束日期必须晚于开始日期",
            ));
        }

        self.end_date = Some(end_date);

        // 如果结束日期已经过了，自动停用
        if end_date <= Utc::now() {
            self.is_active = false;
        }

        Ok(())
    }

    /// 检查是否有指定权限
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// 检查是否为活跃代理
    pub fn is_active_proxy(&self) -> bool {
        self.is_active
            && self.start_date <= Utc::now()
            && self.end_date.is_none_or(|end| end > Utc::now())
    }

    /// 验证代理权限
    pub fn validate_proxy_action(&self, action: &ProxyAction) -> DomainResult<()> {
        if !self.is_active_proxy() {
            return Err(DomainError::business_rule_violation("代理关系不活跃"));
        }

        // 根据操作类型检查权限（简化版本，实际应用中需要更详细的权限映射）
        let required_permission_check = match action {
            ProxyAction::JoinRoom { .. } => {
                // 检查是否有加入房间的权限
                self.permissions
                    .iter()
                    .any(|p| p.resource == "room" && (p.action == "join" || p.action == "all"))
            }
            ProxyAction::SendMessage { .. } => {
                // 检查是否有发送消息的权限
                self.permissions
                    .iter()
                    .any(|p| p.resource == "message" && (p.action == "send" || p.action == "all"))
            }
            ProxyAction::LeaveRoom { .. } => {
                // 检查是否有离开房间的权限
                self.permissions
                    .iter()
                    .any(|p| p.resource == "room" && (p.action == "leave" || p.action == "all"))
            }
            ProxyAction::CreateRoom { .. } => {
                // 检查是否有创建房间的权限
                self.permissions
                    .iter()
                    .any(|p| p.resource == "room" && (p.action == "create" || p.action == "all"))
            }
        };

        if !required_permission_check {
            return Err(DomainError::business_rule_violation("代理权限不足"));
        }

        Ok(())
    }

    /// 检查代理关系是否过期
    pub fn is_expired(&self) -> bool {
        self.end_date.is_some_and(|end| end <= Utc::now())
    }

    /// 检查代理关系是否即将过期（24小时内）
    pub fn is_expiring_soon(&self) -> bool {
        if let Some(end_date) = self.end_date {
            let now = Utc::now();
            let twenty_four_hours_later = now + chrono::Duration::hours(24);
            end_date <= twenty_four_hours_later && end_date > now
        } else {
            false
        }
    }

    /// 延长代理关系
    pub fn extend(&mut self, new_end_date: Option<DateTime<Utc>>) -> DomainResult<()> {
        if let Some(new_end) = new_end_date {
            if new_end <= self.start_date {
                return Err(DomainError::validation_error(
                    "end_date",
                    "结束日期必须晚于开始日期",
                ));
            }

            // 如果当前有结束日期，新的结束日期应该晚于当前结束日期
            if let Some(current_end) = self.end_date {
                if new_end <= current_end {
                    return Err(DomainError::business_rule_violation(
                        "新的结束日期必须晚于当前结束日期",
                    ));
                }
            }
        }

        self.end_date = new_end_date;

        // 如果代理因过期被停用，重新激活
        if self.is_expired() && !self.is_active {
            self.is_active = true;
        }

        Ok(())
    }

    /// 添加权限
    pub fn add_permission(&mut self, permission: Permission) {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
        }
    }

    /// 移除权限
    pub fn remove_permission(&mut self, permission: &Permission) {
        self.permissions.retain(|p| p != permission);
    }

    /// 检查是否为临时代理
    pub fn is_temporary(&self) -> bool {
        matches!(self.proxy_type, ProxyType::Temporary)
    }

    /// 检查是否为永久代理
    pub fn is_permanent(&self) -> bool {
        matches!(self.proxy_type, ProxyType::Permanent)
    }

    /// 检查是否为紧急代理
    pub fn is_emergency(&self) -> bool {
        matches!(self.proxy_type, ProxyType::Emergency)
    }

    /// 检查是否为休假代理
    pub fn is_vacation(&self) -> bool {
        matches!(self.proxy_type, ProxyType::Vacation)
    }
}

// 为了兼容性，保留旧的ProxyRelationship类型别名
pub type ProxyRelationship = UserProxy;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::roles_permissions::{Permission, PermissionStatus};
    use chrono::Duration;

    fn create_test_permission(resource: &str, action: &str) -> Permission {
        Permission {
            id: Uuid::new_v4(),
            name: format!("{}_{}", resource, action),
            description: None,
            resource: resource.to_string(),
            action: action.to_string(),
            status: PermissionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_user_proxy_creation() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let proxy = UserProxy::new(
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions.clone(),
        )
        .unwrap();

        assert_eq!(proxy.principal_id, principal_id);
        assert_eq!(proxy.proxy_id, proxy_id);
        assert_eq!(proxy.proxy_type, ProxyType::Temporary);
        assert_eq!(proxy.permissions, permissions);
        assert!(proxy.is_active);
        assert!(proxy.is_active_proxy());
        assert!(proxy.is_temporary());
        assert!(!proxy.is_permanent());
    }

    #[test]
    fn test_same_user_proxy_validation() {
        let user_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let result = UserProxy::new(
            user_id,
            user_id, // 同一个用户
            ProxyType::Temporary,
            permissions,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_proxy_activation_deactivation() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let mut proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Permanent, permissions).unwrap();

        assert!(proxy.is_active);
        assert!(proxy.is_active_proxy());

        proxy.deactivate();
        assert!(!proxy.is_active);
        assert!(!proxy.is_active_proxy());

        proxy.activate();
        assert!(proxy.is_active);
        assert!(proxy.is_active_proxy());
    }

    #[test]
    fn test_set_end_date() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let mut proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Temporary, permissions).unwrap();

        let future_date = Utc::now() + Duration::days(30);
        assert!(proxy.set_end_date(future_date).is_ok());
        assert_eq!(proxy.end_date, Some(future_date));
        assert!(proxy.is_active_proxy());

        // 创建一个开始日期是过去的代理，以便可以设置过期的结束日期
        let past_start = Utc::now() - Duration::hours(2);
        let mut past_proxy = UserProxy::with_dates(
            Uuid::new_v4(),
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            vec![create_test_permission("room", "join")],
            past_start,
            None,
            true,
            past_start,
        )
        .unwrap();

        // 设置一个已经过期的日期应该自动停用代理
        let expired_date = Utc::now() - Duration::minutes(30); // 30分钟前过期
        assert!(past_proxy.set_end_date(expired_date).is_ok());
        assert!(!past_proxy.is_active);
    }

    #[test]
    fn test_invalid_end_date() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let mut proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Temporary, permissions).unwrap();

        // 结束日期早于开始日期
        let invalid_date = proxy.start_date - Duration::hours(1);
        assert!(proxy.set_end_date(invalid_date).is_err());
    }

    #[test]
    fn test_has_permission() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let room_join_permission = create_test_permission("room", "join");
        let message_send_permission = create_test_permission("message", "send");
        let permissions = vec![room_join_permission.clone()];

        let proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Temporary, permissions).unwrap();

        assert!(proxy.has_permission(&room_join_permission));
        assert!(!proxy.has_permission(&message_send_permission));
    }

    #[test]
    fn test_validate_proxy_action() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();

        let permissions = vec![
            create_test_permission("room", "join"),
            create_test_permission("message", "send"),
        ];

        let proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Temporary, permissions).unwrap();

        // 允许的操作
        let join_action = ProxyAction::JoinRoom {
            room_id,
            password: None,
        };
        assert!(proxy.validate_proxy_action(&join_action).is_ok());

        let send_message_action = ProxyAction::SendMessage {
            room_id,
            content: "Hello".to_string(),
        };
        assert!(proxy.validate_proxy_action(&send_message_action).is_ok());

        // 不允许的操作
        let create_room_action = ProxyAction::CreateRoom {
            name: "New Room".to_string(),
            description: None,
        };
        assert!(proxy.validate_proxy_action(&create_room_action).is_err());
    }

    #[test]
    fn test_inactive_proxy_validation() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let room_id = Uuid::new_v4();

        let permissions = vec![create_test_permission("room", "join")];

        let mut proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Temporary, permissions).unwrap();

        proxy.deactivate();

        let join_action = ProxyAction::JoinRoom {
            room_id,
            password: None,
        };

        // 非活跃代理不能执行操作
        assert!(proxy.validate_proxy_action(&join_action).is_err());
    }

    #[test]
    fn test_expired_proxy() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let start_date = Utc::now() - Duration::days(10);
        let end_date = Utc::now() - Duration::days(1); // 昨天结束

        let proxy = UserProxy::with_dates(
            Uuid::new_v4(),
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions,
            start_date,
            Some(end_date),
            true,
            start_date,
        )
        .unwrap();

        assert!(proxy.is_expired());
        assert!(!proxy.is_active_proxy());
    }

    #[test]
    fn test_expiring_soon() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let start_date = Utc::now() - Duration::days(1);
        let end_date = Utc::now() + Duration::hours(12); // 12小时后结束

        let proxy = UserProxy::with_dates(
            Uuid::new_v4(),
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions,
            start_date,
            Some(end_date),
            true,
            start_date,
        )
        .unwrap();

        assert!(proxy.is_expiring_soon());
        assert!(!proxy.is_expired());
    }

    #[test]
    fn test_extend_proxy() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let mut proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Temporary, permissions).unwrap();

        let original_end = Utc::now() + Duration::days(7);
        assert!(proxy.set_end_date(original_end).is_ok());

        // 延长代理
        let new_end = Utc::now() + Duration::days(14);
        assert!(proxy.extend(Some(new_end)).is_ok());
        assert_eq!(proxy.end_date, Some(new_end));

        // 不能设置更早的结束日期
        let earlier_end = Utc::now() + Duration::days(3);
        assert!(proxy.extend(Some(earlier_end)).is_err());
    }

    #[test]
    fn test_permission_management() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let room_join_permission = create_test_permission("room", "join");
        let message_send_permission = create_test_permission("message", "send");

        let mut proxy = UserProxy::new(
            principal_id,
            proxy_id,
            ProxyType::Permanent,
            vec![room_join_permission.clone()],
        )
        .unwrap();

        assert!(proxy.has_permission(&room_join_permission));
        assert!(!proxy.has_permission(&message_send_permission));

        // 添加权限
        proxy.add_permission(message_send_permission.clone());
        assert!(proxy.has_permission(&message_send_permission));

        // 移除权限
        proxy.remove_permission(&room_join_permission);
        assert!(!proxy.has_permission(&room_join_permission));
        assert!(proxy.has_permission(&message_send_permission));
    }

    #[test]
    fn test_proxy_type_checks() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let temporary_proxy = UserProxy::new(
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions.clone(),
        )
        .unwrap();

        let permanent_proxy = UserProxy::new(
            principal_id,
            proxy_id,
            ProxyType::Permanent,
            permissions.clone(),
        )
        .unwrap();

        let emergency_proxy = UserProxy::new(
            principal_id,
            proxy_id,
            ProxyType::Emergency,
            permissions.clone(),
        )
        .unwrap();

        let vacation_proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Vacation, permissions).unwrap();

        assert!(temporary_proxy.is_temporary());
        assert!(!temporary_proxy.is_permanent());

        assert!(permanent_proxy.is_permanent());
        assert!(!permanent_proxy.is_temporary());

        assert!(emergency_proxy.is_emergency());
        assert!(!emergency_proxy.is_vacation());

        assert!(vacation_proxy.is_vacation());
        assert!(!vacation_proxy.is_emergency());
    }

    #[test]
    fn test_proxy_serialization() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission("room", "join")];

        let proxy =
            UserProxy::new(principal_id, proxy_id, ProxyType::Temporary, permissions).unwrap();

        // 测试序列化
        let json = serde_json::to_string(&proxy).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: UserProxy = serde_json::from_str(&json).unwrap();
        assert_eq!(proxy, deserialized);
    }
}
