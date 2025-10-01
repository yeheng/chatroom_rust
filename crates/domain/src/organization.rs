use crate::errors::DomainError;
use crate::value_objects::{OrgId, OrgPath, Timestamp};

/// 组织节点（简化版：移除冗余字段）
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Organization {
    pub id: OrgId,
    pub name: String,
    pub path: OrgPath, // 'root.sales.east' - 唯一真相来源
    pub metadata: Option<serde_json::Value>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Organization {
    /// 创建组织节点（统一构造函数，消除根节点/子节点特殊情况）
    pub fn new(
        id: OrgId,
        name: impl Into<String>,
        parent_path: Option<&OrgPath>, // None表示根节点
        now: Timestamp,
    ) -> Result<Self, DomainError> {
        let name = Self::validate_name(name.into())?;
        let path = match parent_path {
            Some(parent) => parent.append(name.to_lowercase())?,
            None => OrgPath::root(name.to_lowercase()),
        };

        Ok(Self {
            id,
            name,
            path,
            metadata: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// 重命名组织(会影响path)
    pub fn rename(&mut self, name: impl Into<String>, now: Timestamp) -> Result<(), DomainError> {
        let name = Self::validate_name(name.into())?;
        self.name = name;
        // Note: path更新需要在Repository层处理(涉及级联更新子节点)
        self.updated_at = now;
        Ok(())
    }

    /// 验证组织名称
    fn validate_name(name: String) -> Result<String, DomainError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(DomainError::invalid_argument("org_name", "不能为空"));
        }
        if trimmed.len() > 100 {
            return Err(DomainError::invalid_argument("org_name", "长度不能超过100"));
        }
        // ltree要求path只能包含字母数字下划线
        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c.is_whitespace())
        {
            return Err(DomainError::invalid_argument(
                "org_name",
                "只能包含字母数字下划线和空格",
            ));
        }
        Ok(trimmed.to_owned())
    }

    /// 检查是否是根节点（从path派生）
    pub fn is_root(&self) -> bool {
        self.path.level() == 0
    }

    /// 获取层级（从path派生）
    pub fn level(&self) -> i32 {
        self.path.level()
    }

    /// 获取父路径（从path派生）
    pub fn parent_path(&self) -> Option<OrgPath> {
        self.path.parent()
    }

    /// 检查是否是指定组织的祖先
    pub fn is_ancestor_of(&self, other: &Organization) -> bool {
        other.path.is_descendant_of(&self.path)
    }

    /// 检查是否是指定组织的后代
    pub fn is_descendant_of(&self, other: &Organization) -> bool {
        self.path.is_descendant_of(&other.path)
    }

    /// 更新元数据
    pub fn update_metadata(&mut self, metadata: Option<serde_json::Value>, now: Timestamp) {
        self.metadata = metadata;
        self.updated_at = now;
    }
}
