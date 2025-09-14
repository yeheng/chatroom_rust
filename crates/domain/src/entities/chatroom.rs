//! 聊天室实体定义
//!
//! 包含聊天室的核心信息和相关操作。

use crate::errors::{DomainError, DomainResult};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 聊天室状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRoomStatus {
    /// 活跃状态
    Active,
    /// 已归档状态
    Archived,
    /// 已删除状态
    Deleted,
}

impl Default for ChatRoomStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// 聊天室实体
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRoom {
    /// 聊天室唯一ID
    pub id: Uuid,
    /// 聊天室名称
    pub name: String,
    /// 聊天室描述（可选）
    pub description: Option<String>,
    /// 是否为私密房间
    pub is_private: bool,
    /// 密码哈希（仅私密房间使用）
    pub password_hash: Option<String>,
    /// 房间所有者ID
    pub owner_id: Uuid,
    /// 最大成员数量（None表示无限制）
    pub max_members: Option<u32>,
    /// 当前成员数量
    pub member_count: u32,
    /// 聊天室状态
    pub status: ChatRoomStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
    /// 最后活跃时间
    pub last_activity_at: DateTime<Utc>,
}

impl ChatRoom {
    /// 创建新的公开聊天室
    pub fn new_public(
        name: impl Into<String>,
        owner_id: Uuid,
        description: Option<String>,
        max_members: Option<u32>,
    ) -> DomainResult<Self> {
        let name = name.into();
        Self::validate_name(&name)?;

        if let Some(max) = max_members {
            Self::validate_max_members(max)?;
        }

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description,
            is_private: false,
            password_hash: None,
            owner_id,
            max_members,
            member_count: 1, // 创建者自动成为成员
            status: ChatRoomStatus::Active,
            created_at: now,
            updated_at: now,
            last_activity_at: now,
        })
    }

    /// 创建新的私密聊天室
    pub fn new_private(
        name: impl Into<String>,
        description: Option<String>,
        owner_id: Uuid,
        password: &str,
    ) -> DomainResult<Self> {
        let name = name.into();
        Self::validate_name(&name)?;

        let password_hash = Some(hash(password, DEFAULT_COST).map_err(|e| {
            DomainError::validation_error("password", format!("密码哈希失败: {}", e))
        })?);

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description,
            is_private: true,
            password_hash,
            owner_id,
            max_members: None,
            member_count: 1, // 创建者自动成为成员
            status: ChatRoomStatus::Active,
            created_at: now,
            updated_at: now,
            last_activity_at: now,
        })
    }

    /// 创建具有指定ID的聊天室（用于从数据库加载）
    pub fn with_id(
        id: Uuid,
        name: impl Into<String>,
        description: Option<String>,
        is_private: bool,
        password_hash: Option<String>,
        owner_id: Uuid,
        max_members: Option<u32>,
        member_count: u32,
        status: ChatRoomStatus,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        last_activity_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        let name = name.into();
        Self::validate_name(&name)?;

        if is_private && password_hash.is_none() {
            return Err(DomainError::validation_error(
                "password_hash",
                "私密房间必须设置密码",
            ));
        }

        if let Some(ref hash) = password_hash {
            Self::validate_password_hash(hash)?;
        }

        if let Some(max) = max_members {
            Self::validate_max_members(max)?;
        }

        Ok(Self {
            id,
            name,
            description,
            is_private,
            password_hash,
            owner_id,
            max_members,
            member_count,
            status,
            created_at,
            updated_at,
            last_activity_at,
        })
    }

    /// 更新聊天室名称
    pub fn update_name(&mut self, new_name: impl Into<String>) -> DomainResult<()> {
        let new_name = new_name.into();
        Self::validate_name(&new_name)?;

        self.name = new_name;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 更新聊天室描述
    pub fn update_description(&mut self, new_description: Option<String>) {
        self.description = new_description;
        self.updated_at = Utc::now();
    }

    /// 更新最大成员数量
    pub fn update_max_members(&mut self, new_max: Option<u32>) -> DomainResult<()> {
        if let Some(max) = new_max {
            Self::validate_max_members(max)?;

            // 检查当前成员数量是否超过新的限制
            if self.member_count > max {
                return Err(DomainError::business_rule_violation(format!(
                    "当前成员数量({})超过新的最大限制({})",
                    self.member_count, max
                )));
            }
        }

        self.max_members = new_max;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 更新密码（仅私密房间）
    pub fn change_password(&mut self, new_password: Option<&str>) -> DomainResult<()> {
        match new_password {
            Some(password) => {
                let password_hash = hash(password, DEFAULT_COST).map_err(|e| {
                    DomainError::validation_error("password", format!("密码哈希失败: {}", e))
                })?;
                self.password_hash = Some(password_hash);
                self.is_private = true;
            }
            None => {
                self.password_hash = None;
                self.is_private = false;
            }
        }
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 验证房间密码
    pub fn verify_password(&self, password: &str) -> DomainResult<bool> {
        match &self.password_hash {
            Some(hash_value) => verify(password, hash_value).map_err(|e| {
                DomainError::validation_error("password", format!("密码验证失败: {}", e))
            }),
            None => Ok(true), // 公开房间无需密码
        }
    }

    /// 更新房间信息
    pub fn update_info(&mut self, name: Option<String>, description: Option<String>) {
        if let Some(name) = name {
            self.name = name;
        }
        self.description = description;
        self.updated_at = Utc::now();
    }

    /// 增加成员数量
    pub fn add_member(&mut self) -> DomainResult<()> {
        if let Some(max) = self.max_members {
            if self.member_count >= max {
                return Err(DomainError::business_rule_violation(format!(
                    "聊天室已达到最大成员数量限制({})",
                    max
                )));
            }
        }

        self.member_count += 1;
        self.updated_at = Utc::now();
        self.last_activity_at = Utc::now();
        Ok(())
    }

    /// 减少成员数量
    pub fn remove_member(&mut self) -> DomainResult<()> {
        if self.member_count == 0 {
            return Err(DomainError::business_rule_violation("成员数量已经为0"));
        }

        self.member_count -= 1;
        self.updated_at = Utc::now();
        Ok(())
    }

    /// 更新最后活跃时间
    pub fn update_activity(&mut self) {
        self.last_activity_at = Utc::now();
    }

    /// 归档聊天室
    pub fn archive(&mut self) {
        self.status = ChatRoomStatus::Archived;
        self.updated_at = Utc::now();
    }

    /// 取消归档
    pub fn unarchive(&mut self) {
        if self.status == ChatRoomStatus::Archived {
            self.status = ChatRoomStatus::Active;
            self.updated_at = Utc::now();
        }
    }

    /// 软删除聊天室
    pub fn soft_delete(&mut self) {
        self.status = ChatRoomStatus::Deleted;
        self.updated_at = Utc::now();
    }

    /// 检查聊天室是否活跃
    pub fn is_active(&self) -> bool {
        self.status == ChatRoomStatus::Active
    }

    /// 检查聊天室是否已归档
    pub fn is_archived(&self) -> bool {
        self.status == ChatRoomStatus::Archived
    }

    /// 检查聊天室是否已删除
    pub fn is_deleted(&self) -> bool {
        self.status == ChatRoomStatus::Deleted
    }

    /// 检查是否可以加入更多成员
    pub fn can_add_member(&self) -> bool {
        if !self.is_active() {
            return false;
        }

        match self.max_members {
            Some(max) => self.member_count < max,
            None => true,
        }
    }

    /// 检查是否为空房间
    pub fn is_empty(&self) -> bool {
        self.member_count == 0
    }

    /// 检查是否为满员房间
    pub fn is_full(&self) -> bool {
        match self.max_members {
            Some(max) => self.member_count >= max,
            None => false,
        }
    }

    /// 验证聊天室名称
    fn validate_name(name: &str) -> DomainResult<()> {
        if name.is_empty() {
            return Err(DomainError::validation_error("name", "聊天室名称不能为空"));
        }

        if name.len() < 2 {
            return Err(DomainError::validation_error(
                "name",
                "聊天室名称长度至少2个字符",
            ));
        }

        if name.len() > 100 {
            return Err(DomainError::validation_error(
                "name",
                "聊天室名称长度不能超过100个字符",
            ));
        }

        // 检查是否包含不当字符
        if name.trim() != name {
            return Err(DomainError::validation_error(
                "name",
                "聊天室名称不能包含前后空格",
            ));
        }

        Ok(())
    }

    /// 验证密码哈希
    fn validate_password_hash(password_hash: &str) -> DomainResult<()> {
        if password_hash.is_empty() {
            return Err(DomainError::validation_error(
                "password_hash",
                "密码哈希不能为空",
            ));
        }

        // 简单验证bcrypt哈希格式（以$2开头）
        if !password_hash.starts_with("$2") {
            return Err(DomainError::validation_error(
                "password_hash",
                "密码哈希格式不正确",
            ));
        }

        Ok(())
    }

    /// 验证最大成员数量
    fn validate_max_members(max_members: u32) -> DomainResult<()> {
        if max_members == 0 {
            return Err(DomainError::validation_error(
                "max_members",
                "最大成员数量必须大于0",
            ));
        }

        if max_members > 10000 {
            return Err(DomainError::validation_error(
                "max_members",
                "最大成员数量不能超过10000",
            ));
        }

        Ok(())
    }

    /// 验证用户是否可以加入房间（业务规则）
    pub fn can_user_join(
        &self,
        user_status: crate::entities::user::UserStatus,
        user_org_banned: bool,
    ) -> DomainResult<()> {
        use crate::entities::user::UserStatus;

        // 检查用户状态
        if !matches!(user_status, UserStatus::Active) {
            return Err(DomainError::business_rule_violation(
                "用户状态不活跃，无法加入房间",
            ));
        }

        // 检查用户是否被组织禁止
        if user_org_banned {
            return Err(DomainError::business_rule_violation(
                "用户所属组织被禁止，无法加入房间",
            ));
        }

        // 检查房间是否活跃
        if !self.is_active() {
            return Err(DomainError::business_rule_violation("房间不可用"));
        }

        // 检查房间是否已满
        if !self.can_add_member() {
            return Err(DomainError::business_rule_violation("房间已满"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_public_chatroom_creation() {
        let owner_id = Uuid::new_v4();
        let room = ChatRoom::new_public(
            "Test Room",
            owner_id,
            Some("A test room".to_string()),
            Some(100),
        )
        .unwrap();

        assert_eq!(room.name, "Test Room");
        assert_eq!(room.owner_id, owner_id);
        assert!(!room.is_private);
        assert!(room.password_hash.is_none());
        assert_eq!(room.max_members, Some(100));
        assert_eq!(room.member_count, 1);
        assert!(room.is_active());
    }

    #[test]
    fn test_private_chatroom_creation() {
        let owner_id = Uuid::new_v4();
        let password = "secret123";
        let room = ChatRoom::new_private(
            "Private Room",
            Some("A private room".to_string()),
            owner_id,
            password,
        )
        .unwrap();

        assert_eq!(room.name, "Private Room");
        assert_eq!(room.owner_id, owner_id);
        assert!(room.is_private);
        assert!(room.password_hash.is_some());
        assert_eq!(room.member_count, 1);
    }

    #[test]
    fn test_chatroom_name_validation() {
        let owner_id = Uuid::new_v4();

        // 有效名称
        assert!(ChatRoom::new_public("Valid Room", owner_id, None, None).is_ok());
        assert!(ChatRoom::new_public("房间123", owner_id, None, None).is_ok());

        // 无效名称
        assert!(ChatRoom::new_public("", owner_id, None, None).is_err());
        assert!(ChatRoom::new_public("A", owner_id, None, None).is_err());
        assert!(ChatRoom::new_public(&"A".repeat(101), owner_id, None, None).is_err());
        assert!(ChatRoom::new_public(" Trimmed ", owner_id, None, None).is_err());
    }

    #[test]
    fn test_member_management() {
        let owner_id = Uuid::new_v4();
        let mut room = ChatRoom::new_public("Test Room", owner_id, None, Some(3)).unwrap();

        // 测试添加成员
        assert!(room.add_member().is_ok());
        assert_eq!(room.member_count, 2);
        assert!(!room.is_full());

        assert!(room.add_member().is_ok());
        assert_eq!(room.member_count, 3);
        assert!(room.is_full());

        // 测试超过限制
        assert!(room.add_member().is_err());

        // 测试移除成员
        assert!(room.remove_member().is_ok());
        assert_eq!(room.member_count, 2);
        assert!(!room.is_full());
        assert!(room.can_add_member());
    }

    #[test]
    fn test_chatroom_status_operations() {
        let owner_id = Uuid::new_v4();
        let mut room = ChatRoom::new_public("Test Room", owner_id, None, None).unwrap();

        // 测试归档
        room.archive();
        assert!(room.is_archived());
        assert!(!room.can_add_member());

        // 测试取消归档
        room.unarchive();
        assert!(room.is_active());
        assert!(room.can_add_member());

        // 测试软删除
        room.soft_delete();
        assert!(room.is_deleted());
        assert!(!room.can_add_member());
    }

    #[test]
    fn test_password_verification() {
        let owner_id = Uuid::new_v4();
        let password = "secret123";

        let room =
            ChatRoom::new_private("Room", Some("Private room".to_string()), owner_id, password)
                .unwrap();

        // 测试正确密码
        assert!(room.verify_password(password).unwrap());

        // 测试错误密码
        assert!(!room.verify_password("wrongpassword").unwrap());

        // 测试公开房间（无需密码）
        let public_room = ChatRoom::new_public("Public Room", owner_id, None, None).unwrap();
        assert!(public_room.verify_password("anypassword").unwrap());
    }

    #[test]
    fn test_max_members_validation() {
        let owner_id = Uuid::new_v4();

        // 有效的最大成员数
        assert!(ChatRoom::new_public("Room", owner_id, None, Some(100)).is_ok());
        assert!(ChatRoom::new_public("Room", owner_id, None, Some(10000)).is_ok());
        assert!(ChatRoom::new_public("Room", owner_id, None, None).is_ok());

        // 无效的最大成员数
        assert!(ChatRoom::new_public("Room", owner_id, None, Some(0)).is_err());
        assert!(ChatRoom::new_public("Room", owner_id, None, Some(10001)).is_err());
    }

    #[test]
    fn test_chatroom_updates() {
        let owner_id = Uuid::new_v4();
        let mut room = ChatRoom::new_public("Original Room", owner_id, None, Some(100)).unwrap();
        let original_updated_at = room.updated_at;

        // 等待一小段时间确保时间戳不同
        std::thread::sleep(std::time::Duration::from_millis(1));

        // 测试更新名称
        room.update_name("Updated Room").unwrap();
        assert_eq!(room.name, "Updated Room");
        assert!(room.updated_at > original_updated_at);

        // 测试更新描述
        room.update_description(Some("New description".to_string()));
        assert_eq!(room.description, Some("New description".to_string()));

        // 测试更新最大成员数
        room.update_max_members(Some(200)).unwrap();
        assert_eq!(room.max_members, Some(200));

        // 测试无效的最大成员数更新（小于当前成员数）
        room.member_count = 150;
        assert!(room.update_max_members(Some(100)).is_err());
    }

    #[test]
    fn test_chatroom_serialization() {
        let owner_id = Uuid::new_v4();
        let room = ChatRoom::new_public("Test Room", owner_id, None, Some(100)).unwrap();

        // 测试序列化
        let json = serde_json::to_string(&room).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: ChatRoom = serde_json::from_str(&json).unwrap();
        assert_eq!(room, deserialized);
    }
}
