//! 职位实体定义
//!
//! 包含职位的核心信息和相关操作。

use crate::errors::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 职位实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// 职位ID
    pub id: Uuid,
    /// 职位标题
    pub title: String,
    /// 职位描述
    pub description: Option<String>,
    /// 职位级别
    pub level: i32,
    /// 所属部门ID
    pub department_id: Uuid,
    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl Position {
    /// 创建新职位
    pub fn new(
        title: String,
        description: Option<String>,
        level: i32,
        department_id: Uuid,
    ) -> DomainResult<Self> {
        Self::validate_title(&title)?;
        Self::validate_level(level)?;

        Ok(Self {
            id: Uuid::new_v4(),
            title,
            description,
            level,
            department_id,
            created_at: Utc::now(),
        })
    }

    /// 创建具有指定ID的职位（用于从数据库加载）
    pub fn with_id(
        id: Uuid,
        title: String,
        description: Option<String>,
        level: i32,
        department_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        Self::validate_title(&title)?;
        Self::validate_level(level)?;

        Ok(Self {
            id,
            title,
            description,
            level,
            department_id,
            created_at,
        })
    }

    /// 更新职位标题
    pub fn update_title(&mut self, new_title: String) -> DomainResult<()> {
        Self::validate_title(&new_title)?;
        self.title = new_title;
        Ok(())
    }

    /// 更新职位描述
    pub fn update_description(&mut self, new_description: Option<String>) {
        self.description = new_description;
    }

    /// 更新职位级别
    pub fn update_level(&mut self, new_level: i32) -> DomainResult<()> {
        Self::validate_level(new_level)?;
        self.level = new_level;
        Ok(())
    }

    /// 更新所属部门
    pub fn update_department(&mut self, new_department_id: Uuid) {
        self.department_id = new_department_id;
    }

    /// 检查是否为高级职位（级别 >= 8）
    pub fn is_senior(&self) -> bool {
        self.level >= 8
    }

    /// 检查是否为中级职位（级别 5-7）
    pub fn is_intermediate(&self) -> bool {
        self.level >= 5 && self.level <= 7
    }

    /// 检查是否为初级职位（级别 1-4）
    pub fn is_junior(&self) -> bool {
        self.level >= 1 && self.level <= 4
    }

    /// 比较职位级别
    pub fn is_higher_than(&self, other: &Position) -> bool {
        self.level > other.level
    }

    /// 比较职位级别
    pub fn is_equal_level(&self, other: &Position) -> bool {
        self.level == other.level
    }

    /// 验证职位标题
    fn validate_title(title: &str) -> DomainResult<()> {
        if title.trim().is_empty() {
            return Err(DomainError::validation_error("title", "职位标题不能为空"));
        }

        if title.len() < 2 {
            return Err(DomainError::validation_error(
                "title",
                "职位标题长度至少2个字符",
            ));
        }

        if title.len() > 50 {
            return Err(DomainError::validation_error(
                "title",
                "职位标题长度不能超过50个字符",
            ));
        }

        // 检查是否包含前后空格
        if title.trim() != title {
            return Err(DomainError::validation_error(
                "title",
                "职位标题不能包含前后空格",
            ));
        }

        Ok(())
    }

    /// 验证职位级别
    fn validate_level(level: i32) -> DomainResult<()> {
        if !(1..=10).contains(&level) {
            return Err(DomainError::validation_error(
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

    #[test]
    fn test_position_creation() {
        let department_id = Uuid::new_v4();
        let position = Position::new(
            "高级软件工程师".to_string(),
            Some("负责核心系统开发".to_string()),
            8,
            department_id,
        )
        .unwrap();

        assert_eq!(position.title, "高级软件工程师");
        assert_eq!(position.description, Some("负责核心系统开发".to_string()));
        assert_eq!(position.level, 8);
        assert_eq!(position.department_id, department_id);
        assert!(position.is_senior());
        assert!(!position.is_intermediate());
        assert!(!position.is_junior());
    }

    #[test]
    fn test_position_title_validation() {
        let department_id = Uuid::new_v4();

        // 有效标题
        assert!(Position::new("工程师".to_string(), None, 5, department_id).is_ok());
        assert!(Position::new("高级工程师".to_string(), None, 8, department_id).is_ok());

        // 无效标题
        assert!(Position::new("".to_string(), None, 5, department_id).is_err());
        assert!(Position::new("  ".to_string(), None, 5, department_id).is_err());
        assert!(Position::new("A".to_string(), None, 5, department_id).is_err());
        assert!(Position::new("A".repeat(51), None, 5, department_id).is_err());
        assert!(Position::new("  工程师  ".to_string(), None, 5, department_id).is_err());
    }

    #[test]
    fn test_position_level_validation() {
        let department_id = Uuid::new_v4();

        // 有效级别
        assert!(Position::new("工程师".to_string(), None, 1, department_id).is_ok());
        assert!(Position::new("工程师".to_string(), None, 5, department_id).is_ok());
        assert!(Position::new("工程师".to_string(), None, 10, department_id).is_ok());

        // 无效级别
        assert!(Position::new("工程师".to_string(), None, 0, department_id).is_err());
        assert!(Position::new("工程师".to_string(), None, 11, department_id).is_err());
        assert!(Position::new("工程师".to_string(), None, -1, department_id).is_err());
    }

    #[test]
    fn test_position_level_categories() {
        let department_id = Uuid::new_v4();

        // 初级职位 (1-4)
        let junior = Position::new("初级工程师".to_string(), None, 3, department_id).unwrap();
        assert!(junior.is_junior());
        assert!(!junior.is_intermediate());
        assert!(!junior.is_senior());

        // 中级职位 (5-7)
        let intermediate = Position::new("工程师".to_string(), None, 6, department_id).unwrap();
        assert!(!intermediate.is_junior());
        assert!(intermediate.is_intermediate());
        assert!(!intermediate.is_senior());

        // 高级职位 (8-10)
        let senior = Position::new("高级工程师".to_string(), None, 9, department_id).unwrap();
        assert!(!senior.is_junior());
        assert!(!senior.is_intermediate());
        assert!(senior.is_senior());
    }

    #[test]
    fn test_position_comparison() {
        let department_id = Uuid::new_v4();
        let junior = Position::new("初级工程师".to_string(), None, 3, department_id).unwrap();
        let senior = Position::new("高级工程师".to_string(), None, 8, department_id).unwrap();
        let same_level = Position::new("工程师".to_string(), None, 8, department_id).unwrap();

        assert!(senior.is_higher_than(&junior));
        assert!(!junior.is_higher_than(&senior));
        assert!(!senior.is_higher_than(&same_level));
        assert!(senior.is_equal_level(&same_level));
        assert!(!senior.is_equal_level(&junior));
    }

    #[test]
    fn test_position_updates() {
        let department_id = Uuid::new_v4();
        let mut position = Position::new("工程师".to_string(), None, 5, department_id).unwrap();

        // 更新标题
        assert!(position.update_title("高级工程师".to_string()).is_ok());
        assert_eq!(position.title, "高级工程师");

        // 更新描述
        position.update_description(Some("新的描述".to_string()));
        assert_eq!(position.description, Some("新的描述".to_string()));

        // 更新级别
        assert!(position.update_level(8).is_ok());
        assert_eq!(position.level, 8);
        assert!(position.is_senior());

        // 更新部门
        let new_department_id = Uuid::new_v4();
        position.update_department(new_department_id);
        assert_eq!(position.department_id, new_department_id);

        // 测试无效更新
        assert!(position.update_title("".to_string()).is_err());
        assert!(position.update_level(11).is_err());
    }

    #[test]
    fn test_position_serialization() {
        let department_id = Uuid::new_v4();
        let position = Position::new(
            "高级工程师".to_string(),
            Some("技术专家".to_string()),
            8,
            department_id,
        )
        .unwrap();

        // 测试序列化
        let json = serde_json::to_string(&position).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(position, deserialized);
    }
}
