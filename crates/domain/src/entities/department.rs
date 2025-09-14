//! 部门实体定义
//!
//! 包含部门的核心信息和相关操作。

use crate::errors::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 部门实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Department {
    /// 部门ID
    pub id: Uuid,
    /// 部门名称
    pub name: String,
    /// 部门描述
    pub description: Option<String>,
    /// 父部门ID（支持层级结构）
    pub parent_id: Option<Uuid>,
    /// 部门经理ID
    pub manager_id: Option<Uuid>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl Department {
    /// 创建新部门
    pub fn new(
        name: String,
        description: Option<String>,
        parent_id: Option<Uuid>,
    ) -> DomainResult<Self> {
        Self::validate_name(&name)?;

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description,
            parent_id,
            manager_id: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// 创建具有指定ID的部门（用于从数据库加载）
    pub fn with_id(
        id: Uuid,
        name: String,
        description: Option<String>,
        parent_id: Option<Uuid>,
        manager_id: Option<Uuid>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        Self::validate_name(&name)?;

        Ok(Self {
            id,
            name,
            description,
            parent_id,
            manager_id,
            created_at,
            updated_at,
        })
    }

    /// 设置部门经理
    pub fn set_manager(&mut self, manager_id: Uuid) {
        self.manager_id = Some(manager_id);
        self.updated_at = Utc::now();
    }

    /// 移除部门经理
    pub fn remove_manager(&mut self) {
        self.manager_id = None;
        self.updated_at = Utc::now();
    }

    /// 更新部门名称
    pub fn update_name(&mut self, new_name: String) -> DomainResult<()> {
        Self::validate_name(&new_name)?;
        self.name = new_name;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 更新部门描述
    pub fn update_description(&mut self, new_description: Option<String>) {
        self.description = new_description;
        self.updated_at = Utc::now();
    }

    /// 更新父部门
    pub fn update_parent(&mut self, new_parent_id: Option<Uuid>) -> DomainResult<()> {
        // 不能将自己设为父部门
        if let Some(parent_id) = new_parent_id {
            if parent_id == self.id {
                return Err(DomainError::business_rule_violation(
                    "部门不能设置自己为父部门",
                ));
            }
        }

        self.parent_id = new_parent_id;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 检查是否为根部门
    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }

    /// 检查是否有经理
    pub fn has_manager(&self) -> bool {
        self.manager_id.is_some()
    }

    /// 检查用户是否为部门经理
    pub fn is_manager(&self, user_id: Uuid) -> bool {
        self.manager_id == Some(user_id)
    }

    /// 验证部门名称
    fn validate_name(name: &str) -> DomainResult<()> {
        if name.trim().is_empty() {
            return Err(DomainError::validation_error("name", "部门名称不能为空"));
        }

        if name.len() < 2 {
            return Err(DomainError::validation_error(
                "name",
                "部门名称长度至少2个字符",
            ));
        }

        if name.len() > 100 {
            return Err(DomainError::validation_error(
                "name",
                "部门名称长度不能超过100个字符",
            ));
        }

        // 检查是否包含前后空格
        if name.trim() != name {
            return Err(DomainError::validation_error(
                "name",
                "部门名称不能包含前后空格",
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_department_creation() {
        let department = Department::new(
            "技术部".to_string(),
            Some("负责技术开发和维护".to_string()),
            None,
        )
        .unwrap();

        assert_eq!(department.name, "技术部");
        assert_eq!(
            department.description,
            Some("负责技术开发和维护".to_string())
        );
        assert!(department.parent_id.is_none());
        assert!(department.manager_id.is_none());
        assert!(department.is_root());
        assert!(!department.has_manager());
    }

    #[test]
    fn test_department_with_parent() {
        let parent_id = Uuid::new_v4();
        let department = Department::new("前端开发组".to_string(), None, Some(parent_id)).unwrap();

        assert_eq!(department.name, "前端开发组");
        assert_eq!(department.parent_id, Some(parent_id));
        assert!(!department.is_root());
    }

    #[test]
    fn test_department_name_validation() {
        // 有效名称
        assert!(Department::new("技术部".to_string(), None, None).is_ok());
        assert!(Department::new("研发".to_string(), None, None).is_ok());

        // 无效名称
        assert!(Department::new("".to_string(), None, None).is_err());
        assert!(Department::new("  ".to_string(), None, None).is_err());
        assert!(Department::new("A".to_string(), None, None).is_err());
        assert!(Department::new("A".repeat(101), None, None).is_err());
        assert!(Department::new("  技术部  ".to_string(), None, None).is_err());
    }

    #[test]
    fn test_set_manager() {
        let manager_id = Uuid::new_v4();
        let mut department = Department::new("技术部".to_string(), None, None).unwrap();

        let original_updated_at = department.updated_at;

        // 等待一小段时间确保时间戳不同
        std::thread::sleep(std::time::Duration::from_millis(1));

        department.set_manager(manager_id);

        assert_eq!(department.manager_id, Some(manager_id));
        assert!(department.has_manager());
        assert!(department.is_manager(manager_id));
        assert!(!department.is_manager(Uuid::new_v4()));
        assert!(department.updated_at > original_updated_at);
    }

    #[test]
    fn test_remove_manager() {
        let manager_id = Uuid::new_v4();
        let mut department = Department::new("技术部".to_string(), None, None).unwrap();

        department.set_manager(manager_id);
        assert!(department.has_manager());

        department.remove_manager();
        assert!(!department.has_manager());
        assert!(department.manager_id.is_none());
    }

    #[test]
    fn test_update_name() {
        let mut department = Department::new("技术部".to_string(), None, None).unwrap();

        assert!(department.update_name("研发部".to_string()).is_ok());
        assert_eq!(department.name, "研发部");

        // 测试无效名称更新
        assert!(department.update_name("".to_string()).is_err());
        assert_eq!(department.name, "研发部"); // 名称应该保持不变
    }

    #[test]
    fn test_update_description() {
        let mut department = Department::new("技术部".to_string(), None, None).unwrap();

        let original_updated_at = department.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(1));

        department.update_description(Some("新的描述".to_string()));
        assert_eq!(department.description, Some("新的描述".to_string()));
        assert!(department.updated_at > original_updated_at);

        department.update_description(None);
        assert!(department.description.is_none());
    }

    #[test]
    fn test_update_parent() {
        let mut department = Department::new("技术部".to_string(), None, None).unwrap();

        let parent_id = Uuid::new_v4();

        // 设置父部门
        assert!(department.update_parent(Some(parent_id)).is_ok());
        assert_eq!(department.parent_id, Some(parent_id));
        assert!(!department.is_root());

        // 移除父部门
        assert!(department.update_parent(None).is_ok());
        assert!(department.parent_id.is_none());
        assert!(department.is_root());

        // 不能设置自己为父部门
        assert!(department.update_parent(Some(department.id)).is_err());
    }

    #[test]
    fn test_department_serialization() {
        let department =
            Department::new("技术部".to_string(), Some("技术开发部门".to_string()), None).unwrap();

        // 测试序列化
        let json = serde_json::to_string(&department).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: Department = serde_json::from_str(&json).unwrap();
        assert_eq!(department, deserialized);
    }
}
