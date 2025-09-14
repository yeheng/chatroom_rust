//! 房间成员实体定义
//!
//! 包含房间成员的核心信息和相关操作。

use crate::errors::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 成员角色枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberRole {
    /// 房主
    Owner,
    /// 管理员
    Admin,
    /// 普通成员
    Member,
    /// 机器人
    Bot,
}

impl Default for MemberRole {
    fn default() -> Self {
        Self::Member
    }
}

/// 房间成员实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomMember {
    /// 房间ID
    pub room_id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 成员角色
    pub role: MemberRole,
    /// 加入时间
    pub joined_at: DateTime<Utc>,
}

impl RoomMember {
    /// 创建房间成员
    pub fn new(room_id: Uuid, user_id: Uuid, role: MemberRole) -> Self {
        Self {
            room_id,
            user_id,
            role,
            joined_at: Utc::now(),
        }
    }

    /// 创建具有指定时间的房间成员（用于从数据库加载）
    pub fn with_time(
        room_id: Uuid,
        user_id: Uuid,
        role: MemberRole,
        joined_at: DateTime<Utc>,
    ) -> Self {
        Self {
            room_id,
            user_id,
            role,
            joined_at,
        }
    }

    /// 检查是否为管理员（房主或管理员）
    pub fn is_admin(&self) -> bool {
        matches!(self.role, MemberRole::Owner | MemberRole::Admin)
    }

    /// 检查是否为房主
    pub fn is_owner(&self) -> bool {
        matches!(self.role, MemberRole::Owner)
    }

    /// 检查是否为机器人
    pub fn is_bot(&self) -> bool {
        matches!(self.role, MemberRole::Bot)
    }

    /// 检查是否为普通成员
    pub fn is_member(&self) -> bool {
        matches!(self.role, MemberRole::Member)
    }

    /// 提升为管理员
    pub fn promote_to_admin(&mut self) -> DomainResult<()> {
        if self.is_owner() {
            return Err(DomainError::business_rule_violation("房主不能被提升或降级"));
        }

        if self.is_bot() {
            return Err(DomainError::business_rule_violation(
                "机器人不能被提升为管理员",
            ));
        }

        self.role = MemberRole::Admin;
        Ok(())
    }

    /// 降级为普通成员
    pub fn demote_to_member(&mut self) -> DomainResult<()> {
        if self.is_owner() {
            return Err(DomainError::business_rule_violation("房主不能被提升或降级"));
        }

        if self.is_bot() {
            return Err(DomainError::business_rule_violation("机器人角色不能被修改"));
        }

        self.role = MemberRole::Member;
        Ok(())
    }

    /// 转让房主身份
    pub fn transfer_ownership(&mut self) -> DomainResult<()> {
        if !self.is_owner() {
            return Err(DomainError::business_rule_violation(
                "只有房主才能转让房主身份",
            ));
        }

        self.role = MemberRole::Admin;
        Ok(())
    }

    /// 接受房主身份
    pub fn accept_ownership(&mut self) -> DomainResult<()> {
        if self.is_bot() {
            return Err(DomainError::business_rule_violation("机器人不能成为房主"));
        }

        self.role = MemberRole::Owner;
        Ok(())
    }

    /// 检查是否可以管理其他成员
    pub fn can_manage_member(&self, other: &RoomMember) -> bool {
        match (&self.role, &other.role) {
            // 房主可以管理所有人
            (MemberRole::Owner, _) => true,
            // 管理员可以管理普通成员和机器人，但不能管理房主和其他管理员
            (MemberRole::Admin, MemberRole::Member | MemberRole::Bot) => true,
            // 其他情况都不能管理
            _ => false,
        }
    }

    /// 检查是否可以踢出其他成员
    pub fn can_kick_member(&self, other: &RoomMember) -> bool {
        match (&self.role, &other.role) {
            // 房主可以踢出除自己以外的所有人
            (MemberRole::Owner, role) if *role != MemberRole::Owner => true,
            // 管理员可以踢出普通成员，但不能踢出房主、其他管理员
            (MemberRole::Admin, MemberRole::Member) => true,
            // 其他情况都不能踢出
            _ => false,
        }
    }

    /// 获取角色权重（用于排序和比较）
    pub fn role_weight(&self) -> u8 {
        match self.role {
            MemberRole::Owner => 4,
            MemberRole::Admin => 3,
            MemberRole::Member => 2,
            MemberRole::Bot => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_member_creation() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let member = RoomMember::new(room_id, user_id, MemberRole::Member);

        assert_eq!(member.room_id, room_id);
        assert_eq!(member.user_id, user_id);
        assert_eq!(member.role, MemberRole::Member);
        assert!(!member.is_admin());
        assert!(!member.is_owner());
        assert!(!member.is_bot());
        assert!(member.is_member());
    }

    #[test]
    fn test_member_roles() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        // 测试房主
        let owner = RoomMember::new(room_id, user_id, MemberRole::Owner);
        assert!(owner.is_owner());
        assert!(owner.is_admin());
        assert!(!owner.is_member());
        assert!(!owner.is_bot());

        // 测试管理员
        let admin = RoomMember::new(room_id, user_id, MemberRole::Admin);
        assert!(!admin.is_owner());
        assert!(admin.is_admin());
        assert!(!admin.is_member());
        assert!(!admin.is_bot());

        // 测试普通成员
        let member = RoomMember::new(room_id, user_id, MemberRole::Member);
        assert!(!member.is_owner());
        assert!(!member.is_admin());
        assert!(member.is_member());
        assert!(!member.is_bot());

        // 测试机器人
        let bot = RoomMember::new(room_id, user_id, MemberRole::Bot);
        assert!(!bot.is_owner());
        assert!(!bot.is_admin());
        assert!(!bot.is_member());
        assert!(bot.is_bot());
    }

    #[test]
    fn test_member_promotion() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let mut member = RoomMember::new(room_id, user_id, MemberRole::Member);

        // 提升为管理员
        assert!(member.promote_to_admin().is_ok());
        assert!(member.is_admin());
        assert_eq!(member.role, MemberRole::Admin);

        // 降级为普通成员
        assert!(member.demote_to_member().is_ok());
        assert!(member.is_member());
        assert_eq!(member.role, MemberRole::Member);
    }

    #[test]
    fn test_owner_promotion_restriction() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let mut owner = RoomMember::new(room_id, user_id, MemberRole::Owner);

        // 房主不能被提升或降级
        assert!(owner.promote_to_admin().is_err());
        assert!(owner.demote_to_member().is_err());
    }

    #[test]
    fn test_bot_promotion_restriction() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let mut bot = RoomMember::new(room_id, user_id, MemberRole::Bot);

        // 机器人不能被提升为管理员
        assert!(bot.promote_to_admin().is_err());
        assert!(bot.demote_to_member().is_err());
    }

    #[test]
    fn test_ownership_transfer() {
        let room_id = Uuid::new_v4();
        let user_id1 = Uuid::new_v4();
        let user_id2 = Uuid::new_v4();

        let mut current_owner = RoomMember::new(room_id, user_id1, MemberRole::Owner);
        let mut new_owner = RoomMember::new(room_id, user_id2, MemberRole::Admin);

        // 转让房主身份
        assert!(current_owner.transfer_ownership().is_ok());
        assert_eq!(current_owner.role, MemberRole::Admin);

        // 接受房主身份
        assert!(new_owner.accept_ownership().is_ok());
        assert_eq!(new_owner.role, MemberRole::Owner);
    }

    #[test]
    fn test_bot_cannot_be_owner() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let mut bot = RoomMember::new(room_id, user_id, MemberRole::Bot);

        // 机器人不能成为房主
        assert!(bot.accept_ownership().is_err());
    }

    #[test]
    fn test_member_management_permissions() {
        let room_id = Uuid::new_v4();
        let owner = RoomMember::new(room_id, Uuid::new_v4(), MemberRole::Owner);
        let admin = RoomMember::new(room_id, Uuid::new_v4(), MemberRole::Admin);
        let member = RoomMember::new(room_id, Uuid::new_v4(), MemberRole::Member);
        let bot = RoomMember::new(room_id, Uuid::new_v4(), MemberRole::Bot);

        // 房主可以管理所有人
        assert!(owner.can_manage_member(&admin));
        assert!(owner.can_manage_member(&member));
        assert!(owner.can_manage_member(&bot));

        // 管理员可以管理普通成员和机器人
        assert!(!admin.can_manage_member(&owner)); // 不能管理房主
        assert!(!admin.can_manage_member(&admin)); // 不能管理其他管理员
        assert!(admin.can_manage_member(&member));
        assert!(admin.can_manage_member(&bot));

        // 普通成员不能管理任何人
        assert!(!member.can_manage_member(&owner));
        assert!(!member.can_manage_member(&admin));
        assert!(!member.can_manage_member(&member));
        assert!(!member.can_manage_member(&bot));
    }

    #[test]
    fn test_kick_permissions() {
        let room_id = Uuid::new_v4();
        let owner = RoomMember::new(room_id, Uuid::new_v4(), MemberRole::Owner);
        let admin = RoomMember::new(room_id, Uuid::new_v4(), MemberRole::Admin);
        let member = RoomMember::new(room_id, Uuid::new_v4(), MemberRole::Member);

        // 房主可以踢出管理员和普通成员
        assert!(owner.can_kick_member(&admin));
        assert!(owner.can_kick_member(&member));
        assert!(!owner.can_kick_member(&owner)); // 不能踢出自己

        // 管理员只能踢出普通成员
        assert!(!admin.can_kick_member(&owner)); // 不能踢出房主
        assert!(!admin.can_kick_member(&admin)); // 不能踢出其他管理员
        assert!(admin.can_kick_member(&member));

        // 普通成员不能踢出任何人
        assert!(!member.can_kick_member(&owner));
        assert!(!member.can_kick_member(&admin));
        assert!(!member.can_kick_member(&member));
    }

    #[test]
    fn test_role_weight() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();

        let owner = RoomMember::new(room_id, user_id, MemberRole::Owner);
        let admin = RoomMember::new(room_id, user_id, MemberRole::Admin);
        let member = RoomMember::new(room_id, user_id, MemberRole::Member);
        let bot = RoomMember::new(room_id, user_id, MemberRole::Bot);

        assert_eq!(owner.role_weight(), 4);
        assert_eq!(admin.role_weight(), 3);
        assert_eq!(member.role_weight(), 2);
        assert_eq!(bot.role_weight(), 1);

        // 权重应该正确排序
        assert!(owner.role_weight() > admin.role_weight());
        assert!(admin.role_weight() > member.role_weight());
        assert!(member.role_weight() > bot.role_weight());
    }

    #[test]
    fn test_room_member_serialization() {
        let room_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let member = RoomMember::new(room_id, user_id, MemberRole::Admin);

        // 测试序列化
        let json = serde_json::to_string(&member).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: RoomMember = serde_json::from_str(&json).unwrap();
        assert_eq!(member, deserialized);
    }
}
