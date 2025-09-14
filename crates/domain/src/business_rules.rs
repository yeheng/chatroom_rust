//! 业务规则验证
//!
//! 包含聊天室系统的各种业务规则验证逻辑，确保业务逻辑的正确性和数据一致性

use crate::entities::{
    chatroom::ChatRoom,
    message::{Message, MessageType},
    proxy::{ProxyAction, UserProxy},
    roles_permissions::Permission,
    user::UserStatus,
};
use crate::errors::{DomainError, DomainResult};

/// 房间加入规则验证
pub struct RoomJoinRules;

impl RoomJoinRules {
    /// 验证用户是否可以加入房间
    pub fn can_user_join(
        room: &ChatRoom,
        user_status: UserStatus,
        user_org_banned: bool,
    ) -> DomainResult<()> {
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

        // 检查房间状态
        if !room.is_active() {
            return Err(DomainError::business_rule_violation(
                "房间已被禁用或删除，无法加入",
            ));
        }

        // 检查房间成员数量限制
        if let Some(max_members) = room.max_members {
            if room.member_count >= max_members {
                return Err(DomainError::business_rule_violation(
                    "房间成员数已达上限，无法加入",
                ));
            }
        }

        // 私密房间需要密码（在调用时验证）
        if room.is_private {
            // 密码验证在外部进行
        }

        Ok(())
    }

    /// 验证密码（仅用于私密房间）
    pub fn validate_room_password(room: &ChatRoom, password: Option<&str>) -> DomainResult<()> {
        if !room.is_private {
            return Ok(()); // 公开房间不需要密码
        }

        match password {
            Some(pwd) => room.verify_password(pwd),
            None => Err(DomainError::validation_error(
                "password",
                "私密房间需要提供密码",
            )),
        }?;

        Ok(())
    }
}

/// 消息发送规则验证
pub struct MessageSendRules;

impl MessageSendRules {
    /// 验证消息内容
    pub fn validate_content(message: &Message) -> DomainResult<()> {
        // 对于不同消息类型，检查相应的内容
        match &message.message_type {
            MessageType::Text => {
                if message.content.trim().is_empty() {
                    return Err(DomainError::business_rule_violation("消息内容不能为空"));
                }
            }
            MessageType::Image { url, .. } => {
                if url.trim().is_empty() {
                    return Err(DomainError::business_rule_violation("图片URL不能为空"));
                }
                if message.content.trim().is_empty() {
                    // 图片消息的说明文本可以为空，不做验证
                }
            }
            MessageType::File { filename, url, .. } => {
                if filename.trim().is_empty() || url.trim().is_empty() {
                    return Err(DomainError::business_rule_violation("文件名和URL不能为空"));
                }
            }
            MessageType::Emoji { emoji_id } => {
                if emoji_id.trim().is_empty() {
                    return Err(DomainError::business_rule_violation("表情ID不能为空"));
                }
            }
        }

        // 验证消息内容长度
        if message.content.len() > 10000 {
            return Err(DomainError::business_rule_violation(
                "消息内容过长，最大限制10000字符",
            ));
        }

        // 检查敏感词（简化版）
        if Self::contains_sensitive_content(&message.content) {
            return Err(DomainError::business_rule_violation("消息包含敏感内容"));
        }

        Ok(())
    }

    /// 验证用户是否可以发送消息
    pub fn can_user_send_message(
        user_status: UserStatus,
        user_org_banned: bool,
        room_active: bool,
    ) -> DomainResult<()> {
        // 检查用户状态
        if !matches!(user_status, UserStatus::Active) {
            return Err(DomainError::business_rule_violation(
                "用户状态不活跃，无法发送消息",
            ));
        }

        // 检查用户是否被组织禁止
        if user_org_banned {
            return Err(DomainError::business_rule_violation(
                "用户所属组织被禁止，无法发送消息",
            ));
        }

        // 检查房间状态
        if !room_active {
            return Err(DomainError::business_rule_violation(
                "房间已被禁用或删除，无法发送消息",
            ));
        }

        Ok(())
    }

    /// 验证消息回复引用是否有效
    pub fn validate_reply_reference(
        reply_to_message: Option<&Message>,
        current_room_id: uuid::Uuid,
    ) -> DomainResult<()> {
        if let Some(original_msg) = reply_to_message {
            if original_msg.room_id != current_room_id {
                return Err(DomainError::business_rule_violation(
                    "不能回复其他房间的消息",
                ));
            }

            // 检查原消息是否已被删除
            if matches!(
                original_msg.status,
                crate::entities::message::MessageStatus::Deleted
            ) {
                return Err(DomainError::business_rule_violation("不能回复已删除的消息"));
            }
        }

        Ok(())
    }

    /// 检查敏感内容（简化版实现）
    fn contains_sensitive_content(content: &str) -> bool {
        let sensitive_words = [
            "敏感词",
            "违禁词",
            "垃圾广告",
            "spam",
            "政治敏感",
            "色情",
            "赌博",
            "暴力",
        ];

        let content_lower = content.to_lowercase();
        sensitive_words
            .iter()
            .any(|word| content_lower.contains(&word.to_lowercase()))
    }
}

/// 代理权限规则验证
pub struct ProxyPermissionRules;

impl ProxyPermissionRules {
    /// 验证代理权限
    pub fn validate_proxy_action(proxy: &UserProxy, action: &ProxyAction) -> DomainResult<()> {
        if !proxy.is_active_proxy() {
            return Err(DomainError::business_rule_violation("代理关系不活跃"));
        }

        // 根据操作类型检查权限
        let required_permission_check =
            match action {
                ProxyAction::JoinRoom { .. } => {
                    // 检查是否有加入房间的权限
                    proxy
                        .permissions
                        .iter()
                        .any(|p| (p.resource == "room" && p.action == "join") || p.action == "all")
                }
                ProxyAction::SendMessage { .. } => {
                    // 检查是否有发送消息的权限
                    proxy.permissions.iter().any(|p| {
                        (p.resource == "message" && p.action == "send") || p.action == "all"
                    })
                }
                ProxyAction::LeaveRoom { .. } => {
                    // 检查是否有离开房间的权限
                    proxy
                        .permissions
                        .iter()
                        .any(|p| (p.resource == "room" && p.action == "leave") || p.action == "all")
                }
                ProxyAction::CreateRoom { .. } => {
                    // 检查是否有创建房间的权限
                    proxy.permissions.iter().any(|p| {
                        (p.resource == "room" && p.action == "create") || p.action == "all"
                    })
                }
            };

        if !required_permission_check {
            return Err(DomainError::business_rule_violation("代理权限不足"));
        }

        Ok(())
    }

    /// 验证代理关系的创建规则
    pub fn validate_proxy_creation(
        principal_id: uuid::Uuid,
        proxy_id: uuid::Uuid,
        permissions: &[Permission],
    ) -> DomainResult<()> {
        // 委托人和代理人不能是同一个用户
        if principal_id == proxy_id {
            return Err(DomainError::business_rule_violation(
                "委托人和代理人不能为同一用户",
            ));
        }

        // 权限列表不能为空
        if permissions.is_empty() {
            return Err(DomainError::business_rule_violation("代理权限列表不能为空"));
        }

        // 检查权限的有效性
        for permission in permissions {
            if permission.resource.trim().is_empty() || permission.action.trim().is_empty() {
                return Err(DomainError::business_rule_violation("权限定义不能包含空值"));
            }
        }

        Ok(())
    }
}

/// 用户状态规则验证
pub struct UserStatusRules;

impl UserStatusRules {
    /// 验证用户状态转换是否有效
    pub fn validate_status_transition(from: UserStatus, to: UserStatus) -> DomainResult<()> {
        use UserStatus::*;

        let valid_transitions = match from {
            Active => [Inactive, Suspended, Deleted].as_slice(),
            Inactive => [Active, Suspended, Deleted].as_slice(),
            Suspended => [Active, Inactive, Deleted].as_slice(),
            Deleted => [].as_slice(), // 已删除用户不能转换到其他状态
        };

        if !valid_transitions.contains(&to) {
            return Err(DomainError::business_rule_violation(format!(
                "无效的用户状态转换：从 {:?} 到 {:?}",
                from, to
            )));
        }

        Ok(())
    }

    /// 验证用户是否可以执行操作
    pub fn can_user_perform_action(user_status: UserStatus, action: &str) -> DomainResult<()> {
        match user_status {
            UserStatus::Active => Ok(()),
            UserStatus::Inactive => Err(DomainError::business_rule_violation(format!(
                "未激活用户无法执行操作: {}",
                action
            ))),
            UserStatus::Suspended => Err(DomainError::business_rule_violation(format!(
                "已暂停用户无法执行操作: {}",
                action
            ))),
            UserStatus::Deleted => Err(DomainError::business_rule_violation(format!(
                "已删除用户无法执行操作: {}",
                action
            ))),
        }
    }
}

/// 组织权限规则验证
pub struct OrganizationRules;

impl OrganizationRules {
    /// 验证组织层级深度
    pub fn validate_organization_depth(current_depth: u32) -> DomainResult<()> {
        const MAX_DEPTH: u32 = 5;

        if current_depth >= MAX_DEPTH {
            return Err(DomainError::business_rule_violation(format!(
                "组织层级不能超过{}层",
                MAX_DEPTH
            )));
        }

        Ok(())
    }

    /// 验证用户是否可以访问组织资源
    pub fn can_access_organization_resource(
        user_org_id: Option<uuid::Uuid>,
        resource_org_id: uuid::Uuid,
        user_org_banned: bool,
    ) -> DomainResult<()> {
        // 检查用户组织是否被禁止
        if user_org_banned {
            return Err(DomainError::business_rule_violation(
                "用户所属组织被禁止，无法访问组织资源",
            ));
        }

        // 检查用户是否属于同一组织
        match user_org_id {
            Some(user_org) if user_org == resource_org_id => Ok(()),
            Some(_) => Err(DomainError::business_rule_violation(
                "只能访问同一组织内的资源",
            )),
            None => Err(DomainError::business_rule_violation(
                "未加入组织的用户无法访问组织资源",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{chatroom::ChatRoom, roles_permissions::PermissionStatus};
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_room_join_rules_valid_user() {
        let owner_id = Uuid::new_v4();
        let room = ChatRoom::new_public("Test Room".to_string(), owner_id, None, Some(10)).unwrap();

        let result = RoomJoinRules::can_user_join(&room, UserStatus::Active, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_room_join_rules_inactive_user() {
        let owner_id = Uuid::new_v4();
        let room = ChatRoom::new_public("Test Room".to_string(), owner_id, None, Some(10)).unwrap();

        let result = RoomJoinRules::can_user_join(&room, UserStatus::Inactive, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("用户状态不活跃"));
    }

    #[test]
    fn test_room_join_rules_banned_organization() {
        let owner_id = Uuid::new_v4();
        let room = ChatRoom::new_public("Test Room".to_string(), owner_id, None, Some(10)).unwrap();

        let result = RoomJoinRules::can_user_join(&room, UserStatus::Active, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("组织被禁止"));
    }

    #[test]
    fn test_message_send_rules_valid_content() {
        let message =
            Message::new_text(Uuid::new_v4(), Uuid::new_v4(), "Hello, world!".to_string()).unwrap();

        let result = MessageSendRules::validate_content(&message);
        assert!(result.is_ok());
    }

    #[test]
    fn test_message_send_rules_empty_content() {
        // 创建一个有效的消息，然后手动修改内容为空
        let mut message =
            Message::new_text(Uuid::new_v4(), Uuid::new_v4(), "valid".to_string()).unwrap();
        message.content = "   ".to_string(); // 修改为空白内容

        let result = MessageSendRules::validate_content(&message);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("消息内容不能为空"));
    }

    #[test]
    fn test_message_send_rules_too_long() {
        // 创建一个有效的消息，然后手动修改内容为超长
        let mut message =
            Message::new_text(Uuid::new_v4(), Uuid::new_v4(), "valid".to_string()).unwrap();
        message.content = "a".repeat(10001); // 修改为超长内容

        let result = MessageSendRules::validate_content(&message);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("消息内容过长"));
    }

    #[test]
    fn test_message_send_rules_sensitive_content() {
        // 创建一个有效的消息，然后手动修改内容为敏感词
        let mut message =
            Message::new_text(Uuid::new_v4(), Uuid::new_v4(), "valid".to_string()).unwrap();
        message.content = "这是敏感词测试".to_string(); // 修改为敏感内容

        let result = MessageSendRules::validate_content(&message);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("敏感内容"));
    }

    #[test]
    fn test_user_status_transition_valid() {
        let result =
            UserStatusRules::validate_status_transition(UserStatus::Active, UserStatus::Suspended);
        assert!(result.is_ok());
    }

    #[test]
    fn test_user_status_transition_invalid() {
        let result =
            UserStatusRules::validate_status_transition(UserStatus::Deleted, UserStatus::Active);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("无效的用户状态转换"));
    }

    #[test]
    fn test_user_can_perform_action() {
        let result = UserStatusRules::can_user_perform_action(UserStatus::Active, "send_message");
        assert!(result.is_ok());

        let result =
            UserStatusRules::can_user_perform_action(UserStatus::Suspended, "send_message");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("已暂停用户无法执行"));
    }

    #[test]
    fn test_organization_depth_validation() {
        let result = OrganizationRules::validate_organization_depth(3);
        assert!(result.is_ok());

        let result = OrganizationRules::validate_organization_depth(6);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("组织层级不能超过"));
    }

    #[test]
    fn test_proxy_creation_validation() {
        let principal_id = Uuid::new_v4();
        let proxy_id = Uuid::new_v4();
        let permission = Permission {
            id: Uuid::new_v4(),
            name: "test_permission".to_string(),
            description: Some("测试权限".to_string()),
            resource: "room".to_string(),
            action: "join".to_string(),
            status: PermissionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result =
            ProxyPermissionRules::validate_proxy_creation(principal_id, proxy_id, &[permission]);
        assert!(result.is_ok());

        // 测试同一用户
        let result = ProxyPermissionRules::validate_proxy_creation(principal_id, principal_id, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("不能为同一用户"));

        // 测试空权限列表
        let result = ProxyPermissionRules::validate_proxy_creation(principal_id, proxy_id, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("权限列表不能为空"));
    }

    #[test]
    fn test_organization_access_rules() {
        let org_id = Uuid::new_v4();
        let user_org_id = Some(org_id);

        // 同一组织，未被禁止
        let result =
            OrganizationRules::can_access_organization_resource(user_org_id, org_id, false);
        assert!(result.is_ok());

        // 同一组织，但被禁止
        let result = OrganizationRules::can_access_organization_resource(user_org_id, org_id, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("组织被禁止"));

        // 不同组织
        let result = OrganizationRules::can_access_organization_resource(
            Some(Uuid::new_v4()),
            org_id,
            false,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("同一组织"));

        // 未加入组织
        let result = OrganizationRules::can_access_organization_resource(None, org_id, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("未加入组织"));
    }

    #[test]
    fn test_reply_message_validation() {
        let room_id = Uuid::new_v4();
        let original_message =
            Message::new_text(room_id, Uuid::new_v4(), "Original message".to_string()).unwrap();

        // 有效回复
        let result = MessageSendRules::validate_reply_reference(Some(&original_message), room_id);
        assert!(result.is_ok());

        // 跨房间回复
        let result =
            MessageSendRules::validate_reply_reference(Some(&original_message), Uuid::new_v4());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("不能回复其他房间"));
    }
}
