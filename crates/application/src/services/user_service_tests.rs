//! 用户服务单元测试
//!
//! 测试用户服务的核心功能，包括用户创建、状态管理、搜索、扩展信息管理等功能。

#[cfg(test)]
mod user_service_tests {
    use crate::errors::*;
    use crate::services::user_service::*;
    use domain::user::UserStatus;
    use serde_json::json;
    use std::time::Instant;
    use tokio::time::Duration;
    use uuid::Uuid;

    /// 创建测试用的用户服务
    async fn create_test_user_service() -> UserServiceImpl {
        UserServiceImpl::new()
    }

    /// 创建测试用的用户服务（带配置）
    async fn create_test_user_service_with_config() -> UserServiceImpl {
        UserServiceImpl::with_config(None, None).await.unwrap()
    }

    /// 创建测试用户请求
    fn create_test_user_request() -> CreateUserRequest {
        CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        }
    }

    #[tokio::test]
    async fn test_create_user() {
        let service = create_test_user_service().await;

        let request = CreateUserRequest {
            username: "newuser".to_string(),
            email: "newuser@example.com".to_string(),
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        };

        let result = service.create_user(request).await;

        assert!(result.is_ok());
        let user = result.unwrap();

        assert_eq!(user.username, "newuser");
        assert_eq!(user.email, "newuser@example.com");
        assert_eq!(user.status, UserStatus::Active);
        assert!(user.id != Uuid::nil());
    }

    #[tokio::test]
    async fn test_create_user_with_avatar_and_display_name() {
        let service = create_test_user_service().await;

        let request = CreateUserRequest {
            username: "avataruser".to_string(),
            email: "avatar@example.com".to_string(),
            password: "StrongPass123!".to_string(),
            avatar_url: Some("https://example.com/avatar.jpg".to_string()),
            display_name: Some("Avatar User".to_string()),
        };

        let result = service.create_user(request).await;

        assert!(result.is_ok());
        let user = result.unwrap();

        assert_eq!(
            user.avatar_url,
            Some("https://example.com/avatar.jpg".to_string())
        );
        assert_eq!(user.display_name, Some("Avatar User".to_string()));
    }

    #[tokio::test]
    async fn test_create_user_username_conflict() {
        let service = create_test_user_service().await;

        let request = CreateUserRequest {
            username: "duplicateuser".to_string(),
            email: "user1@example.com".to_string(),
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        };

        // 第一次创建应该成功
        assert!(service.create_user(request.clone()).await.is_ok());

        // 第二次使用相同的用户名应该失败
        let request2 = CreateUserRequest {
            username: "duplicateuser".to_string(),
            email: "user2@example.com".to_string(), // 不同邮箱
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        };

        let result = service.create_user(request2).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::UsernameConflict(_)) => {}
            _ => panic!("Expected UsernameConflict error"),
        }
    }

    #[tokio::test]
    async fn test_create_user_email_conflict() {
        let service = create_test_user_service().await;

        let request1 = CreateUserRequest {
            username: "user1".to_string(),
            email: "same@example.com".to_string(),
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        };

        let request2 = CreateUserRequest {
            username: "user2".to_string(),
            email: "same@example.com".to_string(), // 相同邮箱
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        };

        // 第一次创建应该成功
        assert!(service.create_user(request1).await.is_ok());

        // 第二次使用相同的邮箱应该失败
        let result = service.create_user(request2).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::EmailConflict(_)) => {}
            _ => panic!("Expected EmailConflict error"),
        }
    }

    #[tokio::test]
    async fn test_create_user_invalid_username() {
        let service = create_test_user_service().await;

        let request = CreateUserRequest {
            username: "ab".to_string(), // 用户名太短
            email: "test@example.com".to_string(),
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        };

        let result = service.create_user(request).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::Validation(_)) => {}
            _ => panic!("Expected Validation error"),
        }
    }

    #[tokio::test]
    async fn test_create_user_invalid_email() {
        let service = create_test_user_service().await;

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "invalid-email".to_string(), // 无效邮箱格式
            password: "StrongPass123!".to_string(),
            avatar_url: None,
            display_name: None,
        };

        let result = service.create_user(request).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::Validation(_)) => {}
            _ => panic!("Expected Validation error"),
        }
    }

    #[tokio::test]
    async fn test_create_user_weak_password() {
        let service = create_test_user_service().await;

        let request = CreateUserRequest {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password: "weak".to_string(), // 密码太弱
            avatar_url: None,
            display_name: None,
        };

        let result = service.create_user(request).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::Validation(_)) => {}
            _ => panic!("Expected Validation error"),
        }
    }

    #[tokio::test]
    async fn test_get_user_by_id() {
        let service = create_test_user_service().await;

        // 先创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 获取用户信息
        let result = service.get_user_by_id(user.id).await;

        assert!(result.is_ok());
        let retrieved_user = result.unwrap();

        assert_eq!(retrieved_user.id, user.id);
        assert_eq!(retrieved_user.username, user.username);
        assert_eq!(retrieved_user.email, user.email);
    }

    #[tokio::test]
    async fn test_get_user_by_id_not_found() {
        let service = create_test_user_service().await;

        let result = service.get_user_by_id(Uuid::new_v4()).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::UserNotFound(_)) => {}
            _ => panic!("Expected UserNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_user_by_username() {
        let service = create_test_user_service().await;

        // 先创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 通过用户名获取用户
        let result = service.get_user_by_username(&user.username).await;

        assert!(result.is_ok());
        let retrieved_user = result.unwrap();

        assert_eq!(retrieved_user.id, user.id);
        assert_eq!(retrieved_user.username, user.username);
    }

    #[tokio::test]
    async fn test_get_user_by_email() {
        let service = create_test_user_service().await;

        // 先创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 通过邮箱获取用户
        let result = service.get_user_by_email(&user.email).await;

        assert!(result.is_ok());
        let retrieved_user = result.unwrap();

        assert_eq!(retrieved_user.id, user.id);
        assert_eq!(retrieved_user.email, user.email);
    }

    #[tokio::test]
    async fn test_update_user() {
        let service = create_test_user_service().await;

        // 先创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 更新用户信息
        let update_request = UpdateUserRequest {
            username: Some("newusername".to_string()),
            email: Some("newemail@example.com".to_string()),
            display_name: Some("New Display Name".to_string()),
            avatar_url: Some("https://example.com/new_avatar.jpg".to_string()),
        };

        let result = service.update_user(user.id, update_request).await;

        assert!(result.is_ok());
        let updated_user = result.unwrap();

        assert_eq!(updated_user.username, "newusername");
        assert_eq!(updated_user.email, "newemail@example.com");
        assert_eq!(
            updated_user.display_name,
            Some("New Display Name".to_string())
        );
        assert_eq!(
            updated_user.avatar_url,
            Some("https://example.com/new_avatar.jpg".to_string())
        );
        assert!(updated_user.updated_at > user.updated_at);
    }

    #[tokio::test]
    async fn test_update_user_status() {
        let service = create_test_user_service().await;

        // 创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 更新用户状态为忙碌
        let result = service
            .update_user_status(user.id, UserStatus::Active)
            .await;

        assert!(result.is_ok());

        // 验证状态已更新
        let updated_user = service.get_user_by_id(user.id).await.unwrap();
        assert_eq!(updated_user.status, UserStatus::Active);
    }

    #[tokio::test]
    async fn test_get_users_status() {
        let service = create_test_user_service().await;

        // 创建多个用户
        let mut user_ids = Vec::new();
        for i in 0..3 {
            let request = CreateUserRequest {
                username: format!("user{}", i),
                email: format!("user{}@example.com", i),
                password: "StrongPass123!".to_string(),
                avatar_url: None,
                display_name: None,
            };
            let user = service.create_user(request).await.unwrap();
            user_ids.push(user.id);
        }

        // 批量查询用户状态
        let result = service.get_users_status(&user_ids).await;

        assert!(result.is_ok());
        let status_map = result.unwrap();

        assert_eq!(status_map.len(), 3);
        for (user_id, status) in status_map {
            assert!(user_ids.contains(&user_id));
            assert_eq!(status, UserStatus::Active);
        }
    }

    #[tokio::test]
    async fn test_get_users_status_too_many() {
        let service = create_test_user_service().await;

        // 生成超过限制的用户ID数量（>100）
        let user_ids: Vec<Uuid> = (0..101).map(|_| Uuid::new_v4()).collect();

        let result = service.get_users_status(&user_ids).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::Validation(_)) => {}
            _ => panic!("Expected Validation error"),
        }
    }

    #[tokio::test]
    async fn test_search_users() {
        let service = create_test_user_service().await;

        // 创建多个用户
        let users = vec!["alice", "bob", "charlie"];
        for username in &users {
            let request = CreateUserRequest {
                username: username.to_string(),
                email: format!("{}@example.com", username),
                password: "StrongPass123!".to_string(),
                avatar_url: None,
                display_name: None,
            };
            service.create_user(request).await.unwrap();
        }

        // 搜索包含 "a" 的用户名
        let search_request = UserSearchRequest {
            query: "a".to_string(),
            page: 1,
            page_size: 10,
            status_filter: None,
        };

        let result = service.search_users(search_request).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // 应该找到 alice 和 charlie
        assert_eq!(response.users.len(), 2);
        assert_eq!(response.total, 2);
        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 10);
        assert_eq!(response.total_pages, 1);

        let usernames: Vec<String> = response.users.into_iter().map(|u| u.username).collect();
        assert!(usernames.contains(&"alice".to_string()));
        assert!(usernames.contains(&"charlie".to_string()));
    }

    #[tokio::test]
    async fn test_search_users_with_status_filter() {
        let service = create_test_user_service().await;

        // 创建多个用户
        let mut user_ids = Vec::new();
        for i in 0..3 {
            let request = CreateUserRequest {
                username: format!("user{}", i),
                email: format!("user{}@example.com", i),
                password: "StrongPass123!".to_string(),
                avatar_url: None,
                display_name: None,
            };
            let user = service.create_user(request).await.unwrap();
            user_ids.push(user.id);
        }

        // 设置不同的账户状态：仅一个 Active，其余非 Active
        service
            .update_user_status(user_ids[0], UserStatus::Active)
            .await
            .unwrap();
        service
            .update_user_status(user_ids[1], UserStatus::Inactive)
            .await
            .unwrap();
        service
            .update_user_status(user_ids[2], UserStatus::Suspended)
            .await
            .unwrap();

        // 搜索忙碌状态的用户
        let search_request = UserSearchRequest {
            query: "user".to_string(),
            page: 1,
            page_size: 10,
            status_filter: Some(UserStatus::Active),
        };

        let result = service.search_users(search_request).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        // 仅有1个 Active 用户
        assert_eq!(response.users.len(), 1);
        assert_eq!(response.total, 1);
        assert_eq!(response.users[0].status, UserStatus::Active);
    }

    #[tokio::test]
    async fn test_search_users_pagination() {
        let service = create_test_user_service().await;

        // 创建10个用户
        for i in 0..10 {
            let request = CreateUserRequest {
                username: format!("user{:02}", i),
                email: format!("user{}@example.com", i),
                password: "StrongPass123!".to_string(),
                avatar_url: None,
                display_name: None,
            };
            service.create_user(request).await.unwrap();
        }

        // 第一页，每页3个
        let search_request = UserSearchRequest {
            query: "user".to_string(),
            page: 1,
            page_size: 3,
            status_filter: None,
        };

        let result = service.search_users(search_request).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        assert_eq!(response.users.len(), 3);
        assert_eq!(response.total, 10);
        assert_eq!(response.page, 1);
        assert_eq!(response.page_size, 3);
        assert_eq!(response.total_pages, 4);

        // 第二页
        let search_request = UserSearchRequest {
            query: "user".to_string(),
            page: 2,
            page_size: 3,
            status_filter: None,
        };

        let result = service.search_users(search_request).await;

        assert!(result.is_ok());
        let response = result.unwrap();

        assert_eq!(response.users.len(), 3);
        assert_eq!(response.page, 2);
        assert_eq!(response.total_pages, 4);
    }

    #[tokio::test]
    async fn test_user_extensions() {
        let service = create_test_user_service().await;

        // 创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 添加扩展信息
        let extensions = json!({
            "preferences": {
                "theme": "dark",
                "language": "zh-CN"
            },
            "profile": {
                "bio": "Software developer",
                "location": "Shanghai"
            }
        });

        let result = service
            .update_user_extensions(user.id, extensions.clone())
            .await;
        assert!(result.is_ok());

        // 获取扩展信息
        let result = service.get_user_extensions(user.id).await;
        assert!(result.is_ok());
        let retrieved_extensions = result.unwrap();

        assert_eq!(retrieved_extensions["preferences"]["theme"], "dark");
        assert_eq!(retrieved_extensions["profile"]["bio"], "Software developer");
    }

    #[tokio::test]
    async fn test_user_extensions_too_large() {
        let service = create_test_user_service().await;

        // 创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 创建过大的扩展信息（超过64KB）
        let large_string = "x".repeat(70000); // 70KB
        let extensions = json!({
            "large_field": large_string
        });

        let result = service.update_user_extensions(user.id, extensions).await;

        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::Validation(_)) => {}
            _ => panic!("Expected Validation error"),
        }
    }

    #[tokio::test]
    async fn test_delete_user() {
        let service = create_test_user_service().await;

        // 创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 删除用户
        let result = service.delete_user(user.id).await;
        assert!(result.is_ok());

        // 验证用户已被软删除（状态变为Inactive）
        let result = service.get_user_by_id(user.id).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::UserNotFound(_)) => {}
            _ => panic!("Expected UserNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_user_stats() {
        let service = create_test_user_service().await;

        // 创建多个不同状态的用户
        let mut user_ids = Vec::new();
        for i in 0..5 {
            let request = CreateUserRequest {
                username: format!("user{}", i),
                email: format!("user{}@example.com", i),
                password: "StrongPass123!".to_string(),
                avatar_url: None,
                display_name: None,
            };
            let user = service.create_user(request).await.unwrap();
            user_ids.push(user.id);
        }

        // 设置不同的用户状态（账户状态）
        service
            .update_user_status(user_ids[0], UserStatus::Active)
            .await
            .unwrap();
        service
            .update_user_status(user_ids[1], UserStatus::Active)
            .await
            .unwrap();
        service
            .update_user_status(user_ids[2], UserStatus::Inactive)
            .await
            .unwrap();
        service
            .update_user_status(user_ids[3], UserStatus::Active)
            .await
            .unwrap();
        // user_ids[4] 保持默认Active状态

        // 设置在线状态（Presence）：3个在线（Online/Busy/Away）
        use domain::entities::websocket::UserStatus as PresenceStatus;
        service
            .update_user_presence(user_ids[0], PresenceStatus::Online)
            .await
            .unwrap();
        service
            .update_user_presence(user_ids[1], PresenceStatus::Busy)
            .await
            .unwrap();
        service
            .update_user_presence(user_ids[4], PresenceStatus::Away)
            .await
            .unwrap();

        let result = service.get_user_stats().await;

        assert!(result.is_ok());
        let stats = result.unwrap();

        assert_eq!(stats.total_users, 5);
        assert_eq!(stats.active_users, 4);
        assert_eq!(stats.online_users, 3); // Online/Busy/Away 共3人在线
        assert_eq!(stats.busy_users, 1);
        assert_eq!(stats.away_users, 1);
        assert_eq!(stats.today_new_users, 5); // 今天创建的用户
    }

    #[tokio::test]
    async fn test_verify_credentials() {
        let service = create_test_user_service().await;

        // 创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();

        // 使用用户名验证凭据
        let result = service
            .verify_credentials(&user.username, "StrongPass123!")
            .await;
        assert!(result.is_ok());
        let verified_user = result.unwrap();
        assert_eq!(verified_user.id, user.id);

        // 使用邮箱验证凭据
        let result = service
            .verify_credentials(&user.email, "StrongPass123!")
            .await;
        assert!(result.is_ok());
        let verified_user = result.unwrap();
        assert_eq!(verified_user.id, user.id);

        // 错误密码
        let result = service
            .verify_credentials(&user.username, "wrongpassword")
            .await;
        assert!(result.is_err());
        match result.err().unwrap() {
            ApplicationError::User(UserError::InvalidCredentials) => {}
            _ => panic!("Expected InvalidCredentials error"),
        }
    }

    #[tokio::test]
    async fn test_update_last_activity() {
        let service = create_test_user_service().await;

        // 创建用户
        let user = service
            .create_user(create_test_user_request())
            .await
            .unwrap();
        let original_activity = user.last_active_at;

        // 等待一小段时间确保时间戳不同
        tokio::time::sleep(Duration::from_millis(10)).await;

        // 更新最后活跃时间
        let result = service.update_last_activity(user.id).await;
        assert!(result.is_ok());

        // 验证最后活跃时间已更新
        let updated_user = service.get_user_by_id(user.id).await.unwrap();
        assert!(updated_user.last_active_at > original_activity);
    }

    #[tokio::test]
    async fn test_batch_user_status_query() {
        let service = create_test_user_service().await;

        // 创建50个用户
        let mut user_ids = Vec::new();
        for i in 0..50 {
            let request = CreateUserRequest {
                username: format!("user{}", i),
                email: format!("user{}@example.com", i),
                password: "StrongPass123!".to_string(),
                avatar_url: None,
                display_name: None,
            };
            let user = service.create_user(request).await.unwrap();
            user_ids.push(user.id);
        }

        // 批量查询用户状态，测试性能
        let start = Instant::now();
        let result = service.get_users_status(&user_ids).await;
        let duration = start.elapsed();

        assert!(result.is_ok());
        let status_map = result.unwrap();
        assert_eq!(status_map.len(), 50);

        // 验证查询性能（应该在100ms内完成）
        assert!(duration < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_user_service_concurrent_operations() {
        use std::sync::Arc;

        let service = Arc::new(create_test_user_service().await);
        let mut handles = vec![];

        // 并发创建50个用户
        for i in 0..50 {
            let service_clone = Arc::clone(&service);
            let handle = tokio::spawn(async move {
                let request = CreateUserRequest {
                    username: format!("concurrentuser{}", i),
                    email: format!("concurrent{}@example.com", i),
                    password: "StrongPass123!".to_string(),
                    avatar_url: None,
                    display_name: None,
                };
                service_clone.create_user(request).await
            });
            handles.push(handle);
        }

        // 等待所有操作完成
        let mut success_count = 0;
        for handle in handles {
            if handle.await.unwrap().is_ok() {
                success_count += 1;
            }
        }

        // 所有操作都应该成功
        assert_eq!(success_count, 50);

        // 验证用户总数
        let user_count = service.get_user_count().await;
        assert_eq!(user_count, 50);
    }
}
