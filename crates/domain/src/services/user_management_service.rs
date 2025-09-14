//! 用户管理服务接口定义
//!
//! 提供用户管理相关的业务逻辑服务接口，包括用户角色、权限、部门、职位管理等

use crate::entities::{
    department::Department,
    position::Position,
    roles_permissions::{Permission, UserRole},
};
use crate::errors::DomainResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 用户管理服务接口
#[async_trait]
pub trait UserManagementService: Send + Sync {
    /// 创建用户角色
    async fn create_user_role(&self, request: CreateUserRoleRequest) -> DomainResult<UserRole>;

    /// 分配用户角色
    async fn assign_user_role(
        &self,
        user_id: Uuid,
        role_id: Uuid,
        assigned_by: Uuid,
    ) -> DomainResult<()>;

    /// 移除用户角色
    async fn remove_user_role(&self, user_id: Uuid, role_id: Uuid) -> DomainResult<()>;

    /// 创建部门
    async fn create_department(&self, request: CreateDepartmentRequest)
        -> DomainResult<Department>;

    /// 创建职位
    async fn create_position(&self, request: CreatePositionRequest) -> DomainResult<Position>;

    /// 分配用户职位
    async fn assign_user_position(
        &self,
        user_id: Uuid,
        position_id: Uuid,
        department_id: Uuid,
    ) -> DomainResult<()>;

    /// 检查用户权限
    async fn check_user_permission(
        &self,
        user_id: Uuid,
        permission: Permission,
    ) -> DomainResult<bool>;

    /// 获取用户角色列表
    async fn get_user_roles(&self, user_id: Uuid) -> DomainResult<Vec<UserRole>>;

    /// 获取用户权限列表
    async fn get_user_permissions(&self, user_id: Uuid) -> DomainResult<Vec<Permission>>;
}

/// 创建用户角色请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRoleRequest {
    /// 角色名称
    pub name: String,
    /// 角色描述
    pub description: Option<String>,
    /// 角色权限列表
    pub permissions: Vec<Permission>,
    /// 创建者ID
    pub created_by: Uuid,
}

impl CreateUserRoleRequest {
    /// 创建新的用户角色创建请求
    pub fn new(
        name: String,
        description: Option<String>,
        permissions: Vec<Permission>,
        created_by: Uuid,
    ) -> Self {
        Self {
            name,
            description,
            permissions,
            created_by,
        }
    }

    /// 创建简单角色请求（无权限）
    pub fn simple(name: String, created_by: Uuid) -> Self {
        Self::new(name, None, Vec::new(), created_by)
    }

    /// 验证请求参数
    pub fn validate(&self) -> DomainResult<()> {
        if self.name.trim().is_empty() {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "角色名称不能为空",
            ));
        }

        if self.name.len() < 2 {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "角色名称长度至少2个字符",
            ));
        }

        if self.name.len() > 50 {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "角色名称长度不能超过50个字符",
            ));
        }

        Ok(())
    }
}

/// 创建部门请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDepartmentRequest {
    /// 部门名称
    pub name: String,
    /// 部门描述
    pub description: Option<String>,
    /// 父部门ID（可选，顶级部门为None）
    pub parent_id: Option<Uuid>,
    /// 创建者ID
    pub created_by: Uuid,
}

impl CreateDepartmentRequest {
    /// 创建新的部门创建请求
    pub fn new(
        name: String,
        description: Option<String>,
        parent_id: Option<Uuid>,
        created_by: Uuid,
    ) -> Self {
        Self {
            name,
            description,
            parent_id,
            created_by,
        }
    }

    /// 创建顶级部门请求
    pub fn top_level(name: String, created_by: Uuid) -> Self {
        Self::new(name, None, None, created_by)
    }

    /// 创建子部门请求
    pub fn child(name: String, parent_id: Uuid, created_by: Uuid) -> Self {
        Self::new(name, None, Some(parent_id), created_by)
    }

    /// 验证请求参数
    pub fn validate(&self) -> DomainResult<()> {
        if self.name.trim().is_empty() {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "部门名称不能为空",
            ));
        }

        if self.name.len() < 2 {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "部门名称长度至少2个字符",
            ));
        }

        if self.name.len() > 50 {
            return Err(crate::errors::DomainError::validation_error(
                "name",
                "部门名称长度不能超过50个字符",
            ));
        }

        Ok(())
    }
}

/// 创建职位请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePositionRequest {
    /// 职位标题
    pub title: String,
    /// 职位描述
    pub description: Option<String>,
    /// 职位级别
    pub level: i32,
    /// 所属部门ID
    pub department_id: Uuid,
    /// 创建者ID
    pub created_by: Uuid,
}

impl CreatePositionRequest {
    /// 创建新的职位创建请求
    pub fn new(
        title: String,
        description: Option<String>,
        level: i32,
        department_id: Uuid,
        created_by: Uuid,
    ) -> Self {
        Self {
            title,
            description,
            level,
            department_id,
            created_by,
        }
    }

    /// 验证请求参数
    pub fn validate(&self) -> DomainResult<()> {
        if self.title.trim().is_empty() {
            return Err(crate::errors::DomainError::validation_error(
                "title",
                "职位标题不能为空",
            ));
        }

        if self.title.len() < 2 {
            return Err(crate::errors::DomainError::validation_error(
                "title",
                "职位标题长度至少2个字符",
            ));
        }

        if self.title.len() > 50 {
            return Err(crate::errors::DomainError::validation_error(
                "title",
                "职位标题长度不能超过50个字符",
            ));
        }

        if self.level < 1 || self.level > 10 {
            return Err(crate::errors::DomainError::validation_error(
                "level",
                "职位级别必须在1-10之间",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::roles_permissions::PermissionStatus;
    use chrono::Utc;

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
    fn test_create_user_role_request_new() {
        let created_by = Uuid::new_v4();
        let permissions = vec![create_test_permission()];

        let request = CreateUserRoleRequest::new(
            "测试角色".to_string(),
            Some("角色描述".to_string()),
            permissions.clone(),
            created_by,
        );

        assert_eq!(request.name, "测试角色");
        assert_eq!(request.description, Some("角色描述".to_string()));
        assert_eq!(request.permissions.len(), 1);
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_user_role_request_simple() {
        let created_by = Uuid::new_v4();

        let request = CreateUserRoleRequest::simple("简单角色".to_string(), created_by);

        assert_eq!(request.name, "简单角色");
        assert_eq!(request.description, None);
        assert!(request.permissions.is_empty());
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_user_role_request_validation() {
        let created_by = Uuid::new_v4();

        // 有效请求
        let valid_request = CreateUserRoleRequest::simple("有效角色".to_string(), created_by);
        assert!(valid_request.validate().is_ok());

        // 空名称
        let empty_name = CreateUserRoleRequest::simple("".to_string(), created_by);
        assert!(empty_name.validate().is_err());

        // 名称过短
        let short_name = CreateUserRoleRequest::simple("A".to_string(), created_by);
        assert!(short_name.validate().is_err());

        // 名称过长
        let long_name = CreateUserRoleRequest::simple("A".repeat(51), created_by);
        assert!(long_name.validate().is_err());

        // 边界值测试
        let min_valid = CreateUserRoleRequest::simple("AB".to_string(), created_by);
        assert!(min_valid.validate().is_ok());

        let max_valid = CreateUserRoleRequest::simple("A".repeat(50), created_by);
        assert!(max_valid.validate().is_ok());
    }

    #[test]
    fn test_create_department_request_top_level() {
        let created_by = Uuid::new_v4();

        let request = CreateDepartmentRequest::top_level("顶级部门".to_string(), created_by);

        assert_eq!(request.name, "顶级部门");
        assert_eq!(request.parent_id, None);
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_department_request_child() {
        let created_by = Uuid::new_v4();
        let parent_id = Uuid::new_v4();

        let request = CreateDepartmentRequest::child("子部门".to_string(), parent_id, created_by);

        assert_eq!(request.name, "子部门");
        assert_eq!(request.parent_id, Some(parent_id));
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_department_request_validation() {
        let created_by = Uuid::new_v4();

        // 有效请求
        let valid_request = CreateDepartmentRequest::top_level("技术部".to_string(), created_by);
        assert!(valid_request.validate().is_ok());

        // 空名称
        let empty_name = CreateDepartmentRequest::top_level("".to_string(), created_by);
        assert!(empty_name.validate().is_err());

        // 名称过短
        let short_name = CreateDepartmentRequest::top_level("A".to_string(), created_by);
        assert!(short_name.validate().is_err());

        // 名称过长
        let long_name = CreateDepartmentRequest::top_level("A".repeat(51), created_by);
        assert!(long_name.validate().is_err());
    }

    #[test]
    fn test_create_position_request() {
        let created_by = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let request = CreatePositionRequest::new(
            "高级工程师".to_string(),
            Some("负责核心开发".to_string()),
            8,
            department_id,
            created_by,
        );

        assert_eq!(request.title, "高级工程师");
        assert_eq!(request.description, Some("负责核心开发".to_string()));
        assert_eq!(request.level, 8);
        assert_eq!(request.department_id, department_id);
        assert_eq!(request.created_by, created_by);
    }

    #[test]
    fn test_create_position_request_validation() {
        let created_by = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        // 有效请求
        let valid_request =
            CreatePositionRequest::new("工程师".to_string(), None, 5, department_id, created_by);
        assert!(valid_request.validate().is_ok());

        // 空标题
        let empty_title =
            CreatePositionRequest::new("".to_string(), None, 5, department_id, created_by);
        assert!(empty_title.validate().is_err());

        // 标题过短
        let short_title =
            CreatePositionRequest::new("A".to_string(), None, 5, department_id, created_by);
        assert!(short_title.validate().is_err());

        // 标题过长
        let long_title =
            CreatePositionRequest::new("A".repeat(51), None, 5, department_id, created_by);
        assert!(long_title.validate().is_err());

        // 级别无效
        let invalid_level_low =
            CreatePositionRequest::new("工程师".to_string(), None, 0, department_id, created_by);
        assert!(invalid_level_low.validate().is_err());

        let invalid_level_high =
            CreatePositionRequest::new("工程师".to_string(), None, 11, department_id, created_by);
        assert!(invalid_level_high.validate().is_err());

        // 边界值测试
        let min_level =
            CreatePositionRequest::new("工程师".to_string(), None, 1, department_id, created_by);
        assert!(min_level.validate().is_ok());

        let max_level =
            CreatePositionRequest::new("工程师".to_string(), None, 10, department_id, created_by);
        assert!(max_level.validate().is_ok());
    }
}
