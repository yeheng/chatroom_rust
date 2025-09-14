//! 消息历史查询与缓存服务测试（Task 6）

#[cfg(test)]
mod message_history_service_tests {
    use crate::services::message_history_service::*;
    use domain::message::Message;
    use uuid::Uuid;

    async fn create_svc_and_room_with_user() -> (MessageHistoryServiceImpl, Uuid, Uuid) {
        let svc = MessageHistoryServiceImpl::new();
        let room_id = svc.create_public_room("Room", Uuid::new_v4()).await;
        let user_id = Uuid::new_v4();
        // 确保用户存在并加入
        svc.join_room(room_id, user_id, None).await.unwrap();
        (svc, room_id, user_id)
    }

    #[tokio::test]
    async fn test_message_history_pagination() {
        let (svc, room_id, user_id) = create_svc_and_room_with_user().await;

        // 插入100条消息
        for i in 0..100 {
            let m = Message::new_text(room_id, user_id, format!("Message {}", i)).unwrap();
            svc.add_message(m).await;
        }

        // 第1页
        let q1 = GetRoomHistoryQuery {
            room_id,
            user_id,
            page_size: 20,
            cursor: None,
            message_type: None,
            include_deleted: false,
        };
        let p1 = svc.get_room_history(q1).await.unwrap();
        assert_eq!(p1.messages.len(), 20);
        assert!(p1.has_more);
        assert!(p1.next_cursor.is_some());

        // 第2页
        let q2 = GetRoomHistoryQuery {
            room_id,
            user_id,
            page_size: 20,
            cursor: p1.next_cursor.clone(),
            message_type: None,
            include_deleted: false,
        };
        let p2 = svc.get_room_history(q2).await.unwrap();
        assert_eq!(p2.messages.len(), 20);

        // 不重复
        let s1: std::collections::HashSet<_> = p1.messages.iter().map(|m| m.id).collect();
        let s2: std::collections::HashSet<_> = p2.messages.iter().map(|m| m.id).collect();
        assert!(s1.is_disjoint(&s2));
    }

    #[tokio::test]
    async fn test_private_room_permission_check() {
        let svc = MessageHistoryServiceImpl::new();
        let owner = Uuid::new_v4();
        let room_id = svc
            .create_private_room("Private", owner, "password123")
            .await;
        let authorized = Uuid::new_v4();
        let unauthorized = Uuid::new_v4();
        svc.join_room(room_id, authorized, Some("password123".to_string()))
            .await
            .unwrap();

        // 填充几条消息
        for i in 0..5 {
            let m = Message::new_text(room_id, authorized, format!("M {}", i)).unwrap();
            svc.add_message(m).await;
        }

        // 授权者可查
        let q_ok = GetRoomHistoryQuery {
            room_id,
            user_id: authorized,
            page_size: 10,
            cursor: None,
            message_type: None,
            include_deleted: false,
        };
        assert!(svc.get_room_history(q_ok).await.is_ok());

        // 未授权者不可查
        let q_no = GetRoomHistoryQuery {
            room_id,
            user_id: unauthorized,
            page_size: 10,
            cursor: None,
            message_type: None,
            include_deleted: false,
        };
        let err = svc.get_room_history(q_no).await.err().unwrap();
        match err {
            crate::errors::ApplicationError::Unauthorized(_) => {}
            _ => panic!("Expected Unauthorized"),
        }
    }

    #[tokio::test]
    async fn test_message_history_caching() {
        let (svc, room_id, user_id) = create_svc_and_room_with_user().await;
        for i in 0..30 {
            let m = Message::new_text(room_id, user_id, format!("C {}", i)).unwrap();
            svc.add_message(m).await;
        }

        let q = GetRoomHistoryQuery {
            room_id,
            user_id,
            page_size: 10,
            cursor: None,
            message_type: None,
            include_deleted: false,
        };
        let _ = svc.get_room_history(q.clone()).await.unwrap();
        let s1 = svc.get_cache_stats().await;
        let _ = svc.get_room_history(q).await.unwrap();
        let s2 = svc.get_cache_stats().await;
        // 第二次应命中缓存
        assert!(s2.0 > s1.0);
    }

    #[tokio::test]
    async fn test_message_search() {
        let (svc, room_id, user_id) = create_svc_and_room_with_user().await;
        let msgs = ["Hello world", "Goodbye world", "Random message"];
        for c in msgs.iter() {
            let m = Message::new_text(room_id, user_id, c.to_string()).unwrap();
            svc.add_message(m).await;
        }

        let q = MessageSearchQuery {
            room_id,
            user_id,
            keyword: "world".to_string(),
            page_size: 10,
            cursor: None,
            message_type: None,
        };
        let page = svc.search_messages(q).await.unwrap();
        assert_eq!(page.messages.len(), 2);
        assert!(page.messages.iter().all(|m| m.content.contains("world")));
    }

    #[tokio::test]
    async fn test_large_dataset_performance_smoke() {
        // 仅做运行性检查，避免严格时间断言
        let (svc, room_id, user_id) = create_svc_and_room_with_user().await;
        for i in 0..10_000 {
            let m = Message::new_text(room_id, user_id, format!("Message {}", i)).unwrap();
            svc.add_message(m).await;
        }
        let q = GetRoomHistoryQuery {
            room_id,
            user_id,
            page_size: 50,
            cursor: None,
            message_type: None,
            include_deleted: false,
        };
        let page = svc.get_room_history(q).await.unwrap();
        assert_eq!(page.messages.len(), 50);
    }
}
