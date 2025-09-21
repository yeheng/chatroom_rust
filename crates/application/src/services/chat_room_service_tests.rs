//! 聊天室服务单元测试
//!
//! 测试聊天室服务的核心功能，包括房间创建、加入、离开、成员管理等。

#[cfg(test)]
mod chat_room_service_tests {
    use crate::errors::*;
    use crate::services::chat_room_service::*;
    use crate::services::MemberRole;
    use domain::chatroom::ChatRoomStatus;
    use infrastructure;
    use uuid::Uuid;

    /// 创建测试用的聊天室服务
    async fn create_test_chat_room_service() -> ChatRoomServiceImpl {
        let kafka_config = infrastructure::KafkaConfig {
            brokers: vec!["localhost:9092".to_string()],
            consumer_group_id: "test".to_string(),
            ..Default::default()
        };
        ChatRoomServiceImpl::with_config(Some(&kafka_config), None)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_create_public_room() {
        let service = create_test_chat_room_service().await;
        let user_id = Uuid::new_v4();

        let request = CreateRoomRequest {
            name: "Test Room".to_string(),
            description: Some("A test room".to_string()),
            owner_id: user_id,
            is_private: false,
            password: None,
        };

        let result = service.create_room(request).await;

        assert!(result.is_ok());
        let room = result.unwrap();

        assert_eq!(room.name, "Test Room");
        assert_eq!(room.description, Some("A test room".to_string()));
        assert!(!room.is_private);
        assert_eq!(room.status, ChatRoomStatus::Active);
    }

    #[tokio::test]
    async fn test_create_private_room() {
        let service = create_test_chat_room_service().await;
        let user_id = Uuid::new_v4();

        let request = CreateRoomRequest {
            name: "Private Room".to_string(),
            description: Some("A private room".to_string()),
            owner_id: user_id,
            is_private: true,
            password: Some("secret123".to_string()),
        };

        let result = service.create_room(request).await;

        assert!(result.is_ok());
        let room = result.unwrap();

        assert_eq!(room.name, "Private Room");
        assert!(room.is_private);
        assert!(room.password_hash.is_some());
    }

    #[tokio::test]
    async fn test_join_public_room() {
        let service = create_test_chat_room_service().await;
        let user_id = Uuid::new_v4();

        // 首先创建房间
        let create_request = CreateRoomRequest {
            name: "Public Room".to_string(),
            description: None,
            owner_id: user_id,
            is_private: false,
            password: None,
        };

        let room = service.create_room(create_request).await.unwrap();

        // 加入房间
        let result = service.join_room(room.id, user_id, None).await;

        assert!(result.is_ok());

        // 验证用户已加入房间
        let members = service.get_room_members(room.id).await.unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].user_id, user_id);
        assert_eq!(members[0].role, MemberRole::Member);
    }

    #[tokio::test]
    async fn test_leave_room() {
        let service = create_test_chat_room_service().await;
        let user_id = Uuid::new_v4();

        // 创建房间并加入
        let create_request = CreateRoomRequest {
            name: "Test Room".to_string(),
            description: None,
            owner_id: user_id,
            is_private: false,
            password: None,
        };

        let room = service.create_room(create_request).await.unwrap();
        service.join_room(room.id, user_id, None).await.unwrap();

        // 离开房间
        let result = service.leave_room(room.id, user_id).await;

        assert!(result.is_ok());

        // 验证用户已离开房间
        let members = service.get_room_members(room.id).await.unwrap();
        assert_eq!(members.len(), 0);
    }

    #[tokio::test]
    async fn test_get_room_info() {
        let service = create_test_chat_room_service().await;
        let user_id = Uuid::new_v4();

        // 创建房间
        let create_request = CreateRoomRequest {
            name: "Test Room".to_string(),
            description: Some("A test room".to_string()),
            owner_id: user_id,
            is_private: false,
            password: None,
        };

        let room = service.create_room(create_request).await.unwrap();

        // 获取房间信息
        let result = service.get_room(room.id).await;

        assert!(result.is_ok());
        let retrieved_room = result.unwrap();

        assert_eq!(retrieved_room.id, room.id);
        assert_eq!(retrieved_room.name, "Test Room");
        assert_eq!(retrieved_room.description, Some("A test room".to_string()));
    }

    #[tokio::test]
    async fn test_get_nonexistent_room() {
        let service = create_test_chat_room_service().await;

        let result = service.get_room(Uuid::new_v4()).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::ChatRoom(ChatRoomError::RoomNotFound(_)) => {}
            _ => panic!("Expected room not found error"),
        }
    }

    #[tokio::test]
    async fn test_member_role_management() {
        let service = create_test_chat_room_service().await;
        let owner_id = Uuid::new_v4();
        let member_id = Uuid::new_v4();

        // 创建房间
        let create_request = CreateRoomRequest {
            name: "Test Room".to_string(),
            description: None,
            owner_id,
            is_private: false,
            password: None,
        };

        let room = service.create_room(create_request).await.unwrap();

        // 加入房间
        service.join_room(room.id, member_id, None).await.unwrap();

        // 设置成员角色
        let result = service
            .set_member_role(room.id, member_id, MemberRole::Admin, owner_id)
            .await;

        assert!(result.is_ok());

        // 验证角色更新
        let members = service.get_room_members(room.id).await.unwrap();
        let member = members.iter().find(|m| m.user_id == member_id);
        assert!(member.is_some());
        assert_eq!(member.unwrap().role, MemberRole::Admin);
    }
}
