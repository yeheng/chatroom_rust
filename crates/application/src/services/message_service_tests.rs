//! 消息服务单元测试
//!
//! 测试消息服务的核心功能，包括消息发送、编辑、删除、查询等功能。

#[cfg(test)]
mod message_service_tests {
    use crate::errors::*;
    use crate::services::message_service::*;
    use domain::message::MessageType;
    use domain::user::User;
    use uuid::Uuid;

    /// 创建测试用的消息服务
    async fn create_test_message_service() -> MessageServiceImpl {
        MessageServiceImpl::new()
    }

    /// 创建测试用户
    async fn create_test_user(username: &str, email: &str) -> User {
        User::new(username, email).unwrap()
    }

    /// 创建测试房间
    fn create_test_room_id() -> Uuid {
        Uuid::new_v4()
    }

    /// 准备测试环境：添加用户和房间成员
    async fn setup_test_environment(service: &MessageServiceImpl, room_id: Uuid, user_id: Uuid) {
        // 添加测试用户
        let user = User::new_with_details(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hashed_password".to_string(),
            Some("Test User".to_string()),
            None,
        )
        .unwrap();

        service.add_test_user(user).await;
        service.add_room_member(room_id, user_id).await;
    }

    #[tokio::test]
    async fn test_send_message() {
        let service = create_test_message_service().await;
        let user_id = Uuid::new_v4();
        let room_id = create_test_room_id();

        // 设置测试环境
        setup_test_environment(&service, room_id, user_id).await;

        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "Hello, World!".to_string(),
            message_type: MessageType::Text,
            is_bot_message: false,
        };

        let result = service.send_message(command).await;

        assert!(result.is_ok());
        let message = result.unwrap();

        assert_eq!(message.content, "Hello, World!");
        assert_eq!(message.sender_id, user_id);
        assert_eq!(message.room_id, room_id);
        assert_eq!(message.message_type, MessageType::Text);
    }

    #[tokio::test]
    async fn test_send_empty_message_should_fail() {
        let service = create_test_message_service().await;
        let user_id = Uuid::new_v4();
        let room_id = create_test_room_id();

        // 设置测试环境
        setup_test_environment(&service, room_id, user_id).await;

        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "".to_string(),
            message_type: MessageType::Text,
            is_bot_message: false,
        };

        let result = service.send_message(command).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::Message(MessageError::EmptyContent) => {}
            _ => panic!("Expected EmptyContent error"),
        }
    }

    #[tokio::test]
    async fn test_send_message_with_sensitive_word_should_fail() {
        let service = create_test_message_service().await;
        let user_id = Uuid::new_v4();
        let room_id = create_test_room_id();

        // 设置测试环境
        setup_test_environment(&service, room_id, user_id).await;

        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "This contains password information".to_string(),
            message_type: MessageType::Text,
            is_bot_message: false,
        };

        let result = service.send_message(command).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::Message(MessageError::SensitiveContent) => {}
            _ => panic!("Expected SensitiveContent error"),
        }
    }

    #[tokio::test]
    async fn test_get_message() {
        let service = create_test_message_service().await;
        let user_id = Uuid::new_v4();
        let room_id = create_test_room_id();

        // 设置测试环境
        setup_test_environment(&service, room_id, user_id).await;

        // 先发送一条消息
        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "Test message".to_string(),
            message_type: MessageType::Text,
            is_bot_message: false,
        };

        let sent_message = service.send_message(command).await.unwrap();

        // 获取消息
        let result = service.get_message(sent_message.id).await;

        assert!(result.is_ok());
        let retrieved_message = result.unwrap();

        assert_eq!(retrieved_message.id, sent_message.id);
        assert_eq!(retrieved_message.content, "Test message");
    }

    #[tokio::test]
    async fn test_get_nonexistent_message_should_fail() {
        let service = create_test_message_service().await;
        let fake_id = Uuid::new_v4();

        let result = service.get_message(fake_id).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::Message(MessageError::MessageNotFound(_)) => {}
            _ => panic!("Expected MessageNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_room_messages() {
        let service = create_test_message_service().await;
        let user_id = Uuid::new_v4();
        let room_id = create_test_room_id();

        // 设置测试环境
        setup_test_environment(&service, room_id, user_id).await;

        // 发送几条消息
        for i in 1..=3 {
            let command = SendMessageCommand {
                room_id,
                user_id,
                content: format!("Message {}", i),
                message_type: MessageType::Text,
                is_bot_message: false,
            };

            service.send_message(command).await.unwrap();
        }

        // 获取房间消息
        let params = MessageQueryParams {
            room_id,
            page: Some(1),
            page_size: Some(10),
            message_type: None,
            sender_id: None,
            include_deleted: false,
        };

        let result = service.get_room_messages(params).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        assert_eq!(response.messages.len(), 3);
        assert_eq!(response.total, 3);
        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 10);
        assert_eq!(response.total_pages, 1);
    }

    #[tokio::test]
    async fn test_delete_message() {
        let service = create_test_message_service().await;
        let user_id = Uuid::new_v4();
        let room_id = create_test_room_id();

        // 设置测试环境
        setup_test_environment(&service, room_id, user_id).await;

        // 发送一条消息
        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "To be deleted".to_string(),
            message_type: MessageType::Text,
            is_bot_message: false,
        };

        let message = service.send_message(command).await.unwrap();

        // 删除消息
        let result = service.delete_message(message.id, user_id).await;
        assert!(result.is_ok());

        // 验证消息已被删除（无法获取）
        let get_result = service.get_message(message.id).await;
        assert!(get_result.is_err());
    }

    #[tokio::test]
    async fn test_delete_message_unauthorized_should_fail() {
        let service = create_test_message_service().await;
        let user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        let room_id = create_test_room_id();

        // 设置测试环境
        setup_test_environment(&service, room_id, user_id).await;

        // 发送一条消息
        let command = SendMessageCommand {
            room_id,
            user_id,
            content: "Protected message".to_string(),
            message_type: MessageType::Text,
            is_bot_message: false,
        };

        let message = service.send_message(command).await.unwrap();

        // 尝试用其他用户删除消息
        let result = service.delete_message(message.id, other_user_id).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::Message(MessageError::Unauthorized(_)) => {}
            _ => panic!("Expected Unauthorized error"),
        }
    }
}
