//! 组织管理服务接口定义
//!
//! 提供组织相关的业务逻辑服务接口，包括组织创建、层级管理、禁止状态等

use crate::entities::organization::Organization;
use crate::errors::DomainResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 组织管理服务接口
#[async_trait]
pub trait OrganizationService: Send + Sync {
    /// 创建组织
    async fn create_organization(
        &self,
        request: CreateOrganizationRequest,
    ) -> DomainResult<Organization>;

    /// 禁止组织
    async fn ban_organization(&self, org_id: Uuid, admin_id: Uuid) -> DomainResult<()>;

    /// 解除禁止
    async fn unban_organization(&self, org_id: Uuid, admin_id: Uuid) -> DomainResult<()>;

    /// 获取组织层级
    async fn get_organization_hierarchy(&self, org_id: Uuid) -> DomainResult<Vec<Organization>>;

    /// 检查用户组织禁止状态
    async fn check_user_organization_ban(&self, user_id: Uuid) -> DomainResult<bool>;

    /// 获取被禁止的组织列表
    async fn get_banned_organizations(&self, org_id: Uuid) -> DomainResult<Vec<Uuid>>;

    /// 用户加入组织
    async fn join_organization(&self, user_id: Uuid, org_id: Uuid) -> DomainResult<()>;

    /// 用户离开组织
    async fn leave_organization(&self, user_id: Uuid, org_id: Uuid) -> DomainResult<()>;
}

/// 创建组织请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrganizationRequest {
    /// 组织名称
    pub name: String,
    /// 父组织ID（可选，顶级组织为None）
    pub parent_id: Option<Uuid>,
    /// 创建者ID
    pub created_by: Uuid,
}

impl CreateOrganizationRequest {
    /// 创建新的组织创建请求
    pub fn new(name: String, parent_id: Option<Uuid>, created_by: Uuid) -> Self {
        Self {
            name,
            parent_id,
            created_by,
        }
    }

    /// 创建顶级组织请求
    pub fn top_level(name: String, created_by: Uuid) -> Self {
        Self::new(name, None, created_by)
    }

    /// 创建子组织请求
    pub fn child(name: String, parent_id: Uuid, created_by: Uuid) -> Self {
        Self::new(name, Some(parent_id), created_by)
    }

    /// 验证请求参数
    pub fn validate(&self) -> DomainResult<()> {
        if self.name.trim().is_empty() {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "组织名称不能为空",
            ));
        }

        if self.name.len() < 2 {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "组织名称长度至少2个字符",
            ));
        }

        if self.name.len() > 100 {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "组织名称长度不能超过100个字符",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_organization_request_new() {
        let created_by = Uuid::new_v4();
        let parent_id = Some(Uuid::new_v4());

        let request = CreateOrganizationRequest::new("测试组织".to_string(), parent_id, created_by);

        assert_eq!(request.name, "测试组织");
        assert_eq!(request.parent_id, parent_id);
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_organization_request_top_level() {
        let created_by = Uuid::new_v4();

        let request = CreateOrganizationRequest::top_level("顶级组织".to_string(), created_by);

        assert_eq!(request.name, "顶级组织");
        assert_eq!(request.parent_id, None);
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_organization_request_child() {
        let created_by = Uuid::new_v4();
        let parent_id = Uuid::new_v4();

        let request = CreateOrganizationRequest::child("子组织".to_string(), parent_id, created_by);

        assert_eq!(request.name, "子组织");
        assert_eq!(request.parent_id, Some(parent_id));
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_organization_request_validation() {
        let created_by = Uuid::new_v4();

        // 有效请求
        let valid_request =
            CreateOrganizationRequest::new("有效组织".to_string(), None, created_by);
        assert!(valid_request.validate().is_ok());

        // 空名称
        let empty_name = CreateOrganizationRequest::new("".to_string(), None, created_by);
        assert!(empty_name.validate().is_err());

        // 空白名称
        let whitespace_name = CreateOrganizationRequest::new("  ".to_string(), None, created_by);
        assert!(whitespace_name.validate().is_err());

        // 名称过短
        let short_name = CreateOrganizationRequest::new("A".to_string(), None, created_by);
        assert!(short_name.validate().is_err());

        // 名称过长
        let long_name = CreateOrganizationRequest::new("A".repeat(101), None, created_by);
        assert!(long_name.validate().is_err());

        // 边界值测试 - 2个字符
        let min_valid = CreateOrganizationRequest::new("AB".to_string(), None, created_by);
        assert!(min_valid.validate().is_ok());

        // 边界值测试 - 100个字符
        let max_valid = CreateOrganizationRequest::new("A".repeat(100), None, created_by);
        assert!(max_valid.validate().is_ok());
    }
}
