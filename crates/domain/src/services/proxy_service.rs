//! 代理服务接口定义
//!
//! 提供代理关系管理的业务逻辑服务接口，包括代理创建、权限管理、操作执行等

use crate::entities::{
    proxy::{ProxyAction, ProxyType, UserProxy},
    roles_permissions::Permission,
};
use crate::errors::DomainResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 代理服务接口
#[async_trait]
pub trait ProxyService: Send + Sync {
    /// 创建代理关系
    async fn create_proxy(&self, request: CreateProxyRequest) -> DomainResult<UserProxy>;

    /// 激活代理
    async fn activate_proxy(&self, proxy_id: Uuid) -> DomainResult<()>;

    /// 停用代理
    async fn deactivate_proxy(&self, proxy_id: Uuid) -> DomainResult<()>;

    /// 检查代理权限
    async fn check_proxy_permission(
        &self,
        proxy_id: Uuid,
        principal_id: Uuid,
        permission: Permission,
    ) -> DomainResult<bool>;

    /// 获取活跃的代理关系
    async fn get_active_proxies(&self, principal_id: Uuid) -> DomainResult<Vec<UserProxy>>;

    /// 执行代理操作
    async fn execute_as_proxy(
        &self,
        proxy_id: Uuid,
        principal_id: Uuid,
        action: ProxyAction,
    ) -> DomainResult<()>;

    /// 获取用户的代理关系
    async fn get_user_proxies(&self, user_id: Uuid) -> DomainResult<Vec<UserProxy>>;
}

/// 创建代理请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProxyRequest {
    /// 委托人ID（被代理人）
    pub principal_id: Uuid,
    /// 代理人ID
    pub proxy_id: Uuid,
    /// 代理类型
    pub proxy_type: ProxyType,
    /// 代理权限列表
    pub permissions: Vec<Permission>,
    /// 开始日期（可选，默认为当前时间）
    pub start_date: Option<DateTime<Utc>>,
    /// 结束日期（可选，无结束日期表示长期代理）
    pub end_date: Option<DateTime<Utc>>,
}

impl CreateProxyRequest {
    /// 创建新的代理创建请求
    pub fn new(
        principal_id: Uuid,
        proxy_id: Uuid,
        proxy_type: ProxyType,
        permissions: Vec<Permission>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            principal_id,
            proxy_id,
            proxy_type,
            permissions,
            start_date,
            end_date,
        }
    }

    /// 创建临时代理请求
    pub fn temporary(
        principal_id: Uuid,
        proxy_id: Uuid,
        permissions: Vec<Permission>,
        end_date: DateTime<Utc>,
    ) -> Self {
        Self::new(
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions,
            None,
            Some(end_date),
        )
    }

    /// 创建永久代理请求
    pub fn permanent(principal_id: Uuid, proxy_id: Uuid, permissions: Vec<Permission>) -> Self {
        Self::new(
            principal_id,
            proxy_id,
            ProxyType::Permanent,
            permissions,
            None,
            None,
        )
    }

    /// 创建紧急代理请求
    pub fn emergency(
        principal_id: Uuid,
        proxy_id: Uuid,
        permissions: Vec<Permission>,
        end_date: DateTime<Utc>,
    ) -> Self {
        Self::new(
            principal_id,
            proxy_id,
            ProxyType::Emergency,
            permissions,
            None,
            Some(end_date),
        )
    }

    /// 创建休假代理请求
    pub fn vacation(
        principal_id: Uuid,
        proxy_id: Uuid,
        permissions: Vec<Permission>,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> Self {
        Self::new(
            principal_id,
            proxy_id,
            ProxyType::Vacation,
            permissions,
            Some(start_date),
            Some(end_date),
        )
    }

    /// 验证请求参数
    pub fn validate(&self) -> DomainResult<()> {
        // 委托人和代理人不能是同一个用户
        if self.principal_id == self.proxy_id {
            return Err(crate::errors::DomainError::business_rule_violation(
                "委托人和代理人不能为同一用户",
            ));
        }

        // 验证日期逻辑
        if let (Some(start), Some(end)) = (self.start_date, self.end_date) {
            if end <= start {
                return Err(crate::errors::DomainError::validation_error(
                    "end_date",
                    "结束日期必须晚于开始日期",
                ));
            }
        }

        // 临时和紧急代理必须有结束日期
        match self.proxy_type {
            ProxyType::Temporary | ProxyType::Emergency => {
                if self.end_date.is_none() {
                    return Err(crate::errors::DomainError::validation_error(
                        "end_date",
                        "临时和紧急代理必须设置结束日期",
                    ));
                }
            }
            ProxyType::Vacation => {
                if self.start_date.is_none() || self.end_date.is_none() {
                    return Err(crate::errors::DomainError::validation_error(
                        "dates",
                        "休假代理必须设置开始和结束日期",
                    ));
                }
            }
            ProxyType::Permanent => {
                // 永久代理可以不设置结束日期
            }
        }

        // 权限列表不能为空（除非是特定的系统级操作）
        if self.permissions.is_empty() {
            return Err(crate::errors::DomainError::validation_error(
                "permissions",
                "代理权限列表不能为空",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::roles_permissions::PermissionStatus;
    use chrono::Duration;

    fn create_test_permission() -> Permission {
        Permission {
            id: Uuid::new_v4(),
            name: "test_permission".to_string(),
            description: Some("测试权限".to_string()),
            resource: "test".to_string(),
            action: "read".to_string(),
            status: PermissionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_create_proxy_request_new() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];
        let start_date = Some(Utc::now());
        let end_date = Some(Utc::now() + Duration::days(30));

        let request = CreateProxyRequest::new(
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions.clone(),
            start_date,
            end_date,
        );

        assert_eq!(request.principal_id, principal_id);
        assert_eq!(request.proxy_id, proxy_id);
        assert_eq!(request.proxy_type, ProxyType::Temporary);
        assert_eq!(request.permissions.len(), 1);
        assert_eq!(request.start_date, start_date);
        assert_eq!(request.end_date, end_date);
    }

    #[test]
    fn test_create_proxy_request_temporary() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];
        let end_date = Utc::now() + Duration::days(7);

        let request = CreateProxyRequest::temporary(principal_id, proxy_id, permissions, end_date);

        assert_eq!(request.proxy_type, ProxyType::Temporary);
        assert_eq!(request.end_date, Some(end_date));
        assert_eq!(request.start_date, None);
    }

    #[test]
    fn test_create_proxy_request_permanent() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];

        let request = CreateProxyRequest::permanent(principal_id, proxy_id, permissions);

        assert_eq!(request.proxy_type, ProxyType::Permanent);
        assert_eq!(request.end_date, None);
        assert_eq!(request.start_date, None);
    }

    #[test]
    fn test_create_proxy_request_emergency() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];
        let end_date = Utc::now() + Duration::hours(24);

        let request = CreateProxyRequest::emergency(principal_id, proxy_id, permissions, end_date);

        assert_eq!(request.proxy_type, ProxyType::Emergency);
        assert_eq!(request.end_date, Some(end_date));
    }

    #[test]
    fn test_create_proxy_request_vacation() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];
        let start_date = Utc::now() + Duration::days(1);
        let end_date = Utc::now() + Duration::days(14);

        let request =
            CreateProxyRequest::vacation(principal_id, proxy_id, permissions, start_date, end_date);

        assert_eq!(request.proxy_type, ProxyType::Vacation);
        assert_eq!(request.start_date, Some(start_date));
        assert_eq!(request.end_date, Some(end_date));
    }

    #[test]
    fn test_create_proxy_request_validation_same_user() {
        let user_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];

        let request = CreateProxyRequest::new(
            user_id,
            user_id, // 同一个用户
            ProxyType::Temporary,
            permissions,
            None,
            Some(Utc::now() + Duration::days(1)),
        );

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_create_proxy_request_validation_date_logic() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];
        let start_date = Utc::now();
        let end_date = start_date - Duration::hours(1); // 结束日期早于开始日期

        let request = CreateProxyRequest::new(
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions,
            Some(start_date),
            Some(end_date),
        );

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_create_proxy_request_validation_temporary_needs_end_date() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];

        let request = CreateProxyRequest::new(
            principal_id,
            proxy_id,
            ProxyType::Temporary,
            permissions,
            None,
            None, // 临时代理缺少结束日期
        );

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_create_proxy_request_validation_empty_permissions() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();

        let request = CreateProxyRequest::new(
            principal_id,
            proxy_id,
            ProxyType::Permanent,
            Vec::new(), // 空权限列表
            None,
            None,
        );

        assert!(request.validate().is_err());
    }

    #[test]
    fn test_create_proxy_request_validation_vacation_needs_both_dates() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];

        // 缺少开始日期
        let request1 = CreateProxyRequest::new(
            principal_id,
            proxy_id,
            ProxyType::Vacation,
            permissions.clone(),
            None,
            Some(Utc::now() + Duration::days(7)),
        );
        assert!(request1.validate().is_err());

        // 缺少结束日期
        let request2 = CreateProxyRequest::new(
            principal_id,
            proxy_id,
            ProxyType::Vacation,
            permissions,
            Some(Utc::now()),
            None,
        );
        assert!(request2.validate().is_err());
    }

    #[test]
    fn test_create_proxy_request_validation_success() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permissions = vec![create_test_permission()];

        // 有效的永久代理
        let permanent_request =
            CreateProxyRequest::permanent(principal_id, proxy_id, permissions.clone());
        assert!(permanent_request.validate().is_ok());

        // 有效的临时代理
        let temporary_request = CreateProxyRequest::temporary(
            principal_id,
            proxy_id,
            permissions.clone(),
            Utc::now() + Duration::days(7),
        );
        assert!(temporary_request.validate().is_ok());

        // 有效的休假代理
        let vacation_request = CreateProxyRequest::vacation(
            principal_id,
            proxy_id,
            permissions,
            Utc::now() + Duration::days(1),
            Utc::now() + Duration::days(14),
        );
        assert!(vacation_request.validate().is_ok());
    }
}
