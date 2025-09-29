//! 聊天室系统核心领域模型。
//!
//! 这里只保留纯业务概念：实体、值对象、领域错误。
//! 所有外部技术细节（数据库、加密、异步运行时等）都被隔离在其他层。

mod chat_room;
mod errors;
mod message;
mod room_member;
mod user;
mod value_objects;

pub use chat_room::{ChatRoom, ChatRoomVisibility};
pub use errors::{DomainError, RepositoryError};
pub use message::{Message, MessageRevision, MessageType};
pub use room_member::{RoomMember, RoomRole};
pub use user::{User, UserStatus};
pub use value_objects::{
    MessageContent, MessageId, PasswordHash, RoomId, Timestamp, UserEmail, UserId, Username,
};

#[cfg(test)]
mod rbac_tests {
    use super::*;
    use time::OffsetDateTime;
    use uuid::Uuid;

    /// 测试用户的超级用户权限管理
    #[test]
    fn user_superuser_management_works() {
        let now = OffsetDateTime::now_utc();
        let mut user = User {
            id: UserId::from(Uuid::new_v4()),
            username: Username::parse("testuser".to_string()).unwrap(),
            email: UserEmail::parse("test@example.com".to_string()).unwrap(),
            password: PasswordHash::new("hashed_password".to_string()).unwrap(),
            status: UserStatus::Active,
            is_superuser: false,
            created_at: now,
            updated_at: now,
        };

        // 初始状态下不是管理员
        assert!(!user.is_system_admin());

        // 提升为超级用户
        user.grant_superuser(now);
        assert!(user.is_system_admin());
        assert!(user.is_superuser);

        // 撤销超级用户权限
        user.revoke_superuser(now);
        assert!(!user.is_system_admin());
        assert!(!user.is_superuser);
    }

    /// 测试房间角色权限方法
    #[test]
    fn room_role_permissions_work() {
        // 测试 Owner 权限
        let owner = RoomRole::Owner;
        assert!(owner.has_admin_access(), "Owner should have admin access");
        assert!(owner.is_owner(), "Owner should be identified as owner");
        assert!(
            owner.can_manage_members(),
            "Owner should be able to manage members"
        );
        assert!(
            owner.can_delete_messages(),
            "Owner should be able to delete messages"
        );

        // 测试 Admin 权限
        let admin = RoomRole::Admin;
        assert!(admin.has_admin_access(), "Admin should have admin access");
        assert!(!admin.is_owner(), "Admin should not be identified as owner");
        assert!(
            admin.can_manage_members(),
            "Admin should be able to manage members"
        );
        assert!(
            admin.can_delete_messages(),
            "Admin should be able to delete messages"
        );

        // 测试 Member 权限
        let member = RoomRole::Member;
        assert!(
            !member.has_admin_access(),
            "Member should not have admin access"
        );
        assert!(
            !member.is_owner(),
            "Member should not be identified as owner"
        );
        assert!(
            !member.can_manage_members(),
            "Member should not be able to manage members"
        );
        assert!(
            !member.can_delete_messages(),
            "Member should not be able to delete messages"
        );
    }

    /// 测试角色层次结构
    #[test]
    fn role_hierarchy_works() {
        // Owner > Admin > Member 的权限层次
        let owner = RoomRole::Owner;
        let admin = RoomRole::Admin;
        let member = RoomRole::Member;

        // Owner 和 Admin 都有管理权限
        assert!(owner.has_admin_access());
        assert!(admin.has_admin_access());
        assert!(!member.has_admin_access());

        // 只有 Owner 是所有者
        assert!(owner.is_owner());
        assert!(!admin.is_owner());
        assert!(!member.is_owner());

        // Owner 和 Admin 都能管理成员
        assert!(owner.can_manage_members());
        assert!(admin.can_manage_members());
        assert!(!member.can_manage_members());

        // Owner 和 Admin 都能删除消息
        assert!(owner.can_delete_messages());
        assert!(admin.can_delete_messages());
        assert!(!member.can_delete_messages());
    }
}
