//! 组织层级管理相关实体
//!
//! 支持企业级的组织结构、部门和职位管理

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 组织实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// 组织ID
    pub id: Uuid,
    /// 组织名称
    pub name: String,
    /// 组织描述
    pub description: Option<String>,
    /// 父组织ID（支持层级结构）
    pub parent_id: Option<Uuid>,
    /// 组织类型
    pub organization_type: OrganizationType,
    /// 组织状态
    pub status: OrganizationStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 组织类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationType {
    /// 公司
    Company,
    /// 部门
    Department,
    /// 团队
    Team,
    /// 项目组
    ProjectGroup,
}

impl OrganizationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrganizationType::Company => "company",
            OrganizationType::Department => "department",
            OrganizationType::Team => "team",
            OrganizationType::ProjectGroup => "project_group",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "company" => Some(OrganizationType::Company),
            "department" => Some(OrganizationType::Department),
            "team" => Some(OrganizationType::Team),
            "project_group" => Some(OrganizationType::ProjectGroup),
            _ => None,
        }
    }
}

/// 组织状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationStatus {
    /// 活跃
    Active,
    /// 暂停
    Suspended,
    /// 已删除
    Deleted,
}

impl OrganizationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrganizationStatus::Active => "active",
            OrganizationStatus::Suspended => "suspended",
            OrganizationStatus::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(OrganizationStatus::Active),
            "suspended" => Some(OrganizationStatus::Suspended),
            "deleted" => Some(OrganizationStatus::Deleted),
            _ => None,
        }
    }
}

/// 职位实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// 职位ID
    pub id: Uuid,
    /// 职位名称
    pub name: String,
    /// 职位描述
    pub description: Option<String>,
    /// 所属组织ID
    pub organization_id: Uuid,
    /// 职位级别
    pub level: u32,
    /// 职位状态
    pub status: PositionStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

/// 职位状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionStatus {
    /// 活跃
    Active,
    /// 暂停
    Suspended,
    /// 已删除
    Deleted,
}

impl PositionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PositionStatus::Active => "active",
            PositionStatus::Suspended => "suspended",
            PositionStatus::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(PositionStatus::Active),
            "suspended" => Some(PositionStatus::Suspended),
            "deleted" => Some(PositionStatus::Deleted),
            _ => None,
        }
    }
}

/// 用户组织关联
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOrganization {
    /// 用户ID
    pub user_id: Uuid,
    /// 组织ID
    pub organization_id: Uuid,
    /// 职位ID（可选）
    pub position_id: Option<Uuid>,
    /// 加入时间
    pub joined_at: DateTime<Utc>,
    /// 离开时间（可选）
    pub left_at: Option<DateTime<Utc>>,
    /// 关联状态
    pub status: UserOrganizationStatus,
}

/// 用户组织关联状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserOrganizationStatus {
    /// 活跃
    Active,
    /// 已离开
    Left,
    /// 暂停
    Suspended,
}

impl UserOrganizationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserOrganizationStatus::Active => "active",
            UserOrganizationStatus::Left => "left",
            UserOrganizationStatus::Suspended => "suspended",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(UserOrganizationStatus::Active),
            "left" => Some(UserOrganizationStatus::Left),
            "suspended" => Some(UserOrganizationStatus::Suspended),
            _ => None,
        }
    }
}

/// 组织层级路径
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationPath {
    /// 组织ID
    pub organization_id: Uuid,
    /// 路径（从根到当前组织的ID序列）
    pub path: Vec<Uuid>,
    /// 层级深度
    pub depth: u32,
}

impl Organization {
    /// 创建新的组织
    pub fn new(
        name: String,
        description: Option<String>,
        parent_id: Option<Uuid>,
        organization_type: OrganizationType,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            parent_id,
            organization_type,
            status: OrganizationStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// 检查是否为根组织
    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }

    /// 更新组织信息
    pub fn update(&mut self, name: Option<String>, description: Option<String>) {
        if let Some(name) = name {
            self.name = name;
        }
        self.description = description;
        self.updated_at = Utc::now();
    }

    /// 激活组织
    pub fn activate(&mut self) {
        self.status = OrganizationStatus::Active;
        self.updated_at = Utc::now();
    }

    /// 暂停组织
    pub fn suspend(&mut self) {
        self.status = OrganizationStatus::Suspended;
        self.updated_at = Utc::now();
    }

    /// 删除组织（软删除）
    pub fn delete(&mut self) {
        self.status = OrganizationStatus::Deleted;
        self.updated_at = Utc::now();
    }
}

impl Position {
    /// 创建新的职位
    pub fn new(
        name: String,
        description: Option<String>,
        organization_id: Uuid,
        level: u32,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            organization_id,
            level,
            status: PositionStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// 更新职位信息
    pub fn update(
        &mut self,
        name: Option<String>,
        description: Option<String>,
        level: Option<u32>,
    ) {
        if let Some(name) = name {
            self.name = name;
        }
        self.description = description;
        if let Some(level) = level {
            self.level = level;
        }
        self.updated_at = Utc::now();
    }
}

impl UserOrganization {
    /// 创建新的用户组织关联
    pub fn new(user_id: Uuid, organization_id: Uuid, position_id: Option<Uuid>) -> Self {
        Self {
            user_id,
            organization_id,
            position_id,
            joined_at: Utc::now(),
            left_at: None,
            status: UserOrganizationStatus::Active,
        }
    }

    /// 用户离开组织
    pub fn leave(&mut self) {
        self.status = UserOrganizationStatus::Left;
        self.left_at = Some(Utc::now());
    }

    /// 暂停用户在组织中的状态
    pub fn suspend(&mut self) {
        self.status = UserOrganizationStatus::Suspended;
    }

    /// 恢复用户在组织中的状态
    pub fn reactivate(&mut self) {
        self.status = UserOrganizationStatus::Active;
        self.left_at = None;
    }
}

impl OrganizationPath {
    /// 创建新的组织路径
    pub fn new(organization_id: Uuid, path: Vec<Uuid>) -> Self {
        Self {
            organization_id,
            depth: path.len() as u32,
            path,
        }
    }

    /// 添加子组织到路径
    pub fn append_child(&self, child_id: Uuid) -> Self {
        let mut new_path = self.path.clone();
        new_path.push(child_id);
        Self::new(child_id, new_path)
    }

    /// 检查是否为指定组织的子组织
    pub fn is_descendant_of(&self, ancestor_id: Uuid) -> bool {
        self.path.contains(&ancestor_id)
    }

    /// 获取直接父组织ID
    pub fn parent_id(&self) -> Option<Uuid> {
        if self.path.len() > 1 {
            self.path.get(self.path.len() - 2).copied()
        } else {
            None
        }
    }

    /// 获取根组织ID
    pub fn root_id(&self) -> Option<Uuid> {
        self.path.first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_organization_creation() {
        let org = Organization::new(
            "Test Organization".to_string(),
            Some("Test Description".to_string()),
            None,
            OrganizationType::Company,
        );

        assert_eq!(org.name, "Test Organization");
        assert_eq!(org.description, Some("Test Description".to_string()));
        assert_eq!(org.organization_type, OrganizationType::Company);
        assert_eq!(org.status, OrganizationStatus::Active);
        assert!(org.is_root());
    }

    #[test]
    fn test_position_creation() {
        let org_id = Uuid::new_v4();
        let position = Position::new(
            "Software Engineer".to_string(),
            Some("Backend Developer".to_string()),
            org_id,
            3,
        );

        assert_eq!(position.name, "Software Engineer");
        assert_eq!(position.organization_id, org_id);
        assert_eq!(position.level, 3);
        assert_eq!(position.status, PositionStatus::Active);
    }

    #[test]
    fn test_organization_path() {
        let root_id = Uuid::new_v4();
        let dept_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();

        let root_path = OrganizationPath::new(root_id, vec![root_id]);
        let dept_path = root_path.append_child(dept_id);
        let team_path = dept_path.append_child(team_id);

        assert_eq!(root_path.depth, 1);
        assert_eq!(dept_path.depth, 2);
        assert_eq!(team_path.depth, 3);

        assert!(team_path.is_descendant_of(root_id));
        assert!(team_path.is_descendant_of(dept_id));
        assert!(!root_path.is_descendant_of(dept_id));

        assert_eq!(team_path.root_id(), Some(root_id));
        assert_eq!(team_path.parent_id(), Some(dept_id));
    }
}
