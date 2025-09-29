use application::{MessageRateLimiter, RateLimitError};
use domain::UserId;
use std::sync::Arc;
use uuid::Uuid;
use redis::Client;

#[tokio::test]
async fn test_rate_limiter_integration() {
    // 创建Redis客户端
    let redis_client = Arc::new(Client::open("redis://127.0.0.1:6379").unwrap());
    let limiter = Arc::new(MessageRateLimiter::new(redis_client.clone(), 5, 3)); // 5 msg/min, 3 connections
    let user_id = UserId::from(Uuid::new_v4());

    // 测试连接限制
    assert!(limiter.add_connection(user_id).await.is_ok());
    assert!(limiter.add_connection(user_id).await.is_ok());
    assert!(limiter.add_connection(user_id).await.is_ok());

    // 第4个连接应该被拒绝
    match limiter.add_connection(user_id).await {
        Err(RateLimitError::TooManyConnections { current, max }) => {
            assert_eq!(current, 3);
            assert_eq!(max, 3);
        }
        _ => panic!("Expected TooManyConnections error"),
    }

    // 测试消息限流
    for i in 0..5 {
        match limiter.check_message_rate(user_id).await {
            Ok(_) => println!("Message {} allowed", i + 1),
            Err(e) => panic!("Message {} should be allowed: {:?}", i + 1, e),
        }
    }

    // 第6条消息应该被限流
    match limiter.check_message_rate(user_id).await {
        Err(RateLimitError::RateLimitExceeded { current, max }) => {
            assert_eq!(current, 5);
            assert_eq!(max, 5);
            println!("Rate limit correctly triggered after {} messages", current);
        }
        _ => panic!("Expected RateLimitExceeded error"),
    }

    // 检查用户状态
    let (message_count, connection_count) = limiter.get_user_status(user_id).await.unwrap();
    assert_eq!(message_count, 5);
    assert_eq!(connection_count, 3);

    println!(
        "User status: {} messages, {} connections",
        message_count, connection_count
    );
}

#[tokio::test]
async fn test_rate_limiter_cleanup() {
    let redis_client = Arc::new(Client::open("redis://127.0.0.1:6379").unwrap());
    let limiter = MessageRateLimiter::new(redis_client.clone(), 10, 5);
    let user_id = UserId::from(Uuid::new_v4());

    // 发送一些消息
    for _ in 0..3 {
        limiter.check_message_rate(user_id).await.unwrap();
    }

    let (initial_count, _) = limiter.get_user_status(user_id).await.unwrap();
    assert_eq!(initial_count, 3);

    // 重置用户配额
    limiter.reset_user_quota(user_id).await.unwrap();

    // 检查配额是否被重置
    let (reset_count, _) = limiter.get_user_status(user_id).await.unwrap();
    assert_eq!(reset_count, 0);

    println!("User quota successfully reset");
}

#[tokio::test]
async fn test_concurrent_rate_limiting() {
    let redis_client = Arc::new(Client::open("redis://127.0.0.1:6379").unwrap());
    let limiter = Arc::new(MessageRateLimiter::new(redis_client.clone(), 10, 5));
    let user_id = UserId::from(Uuid::new_v4());

    // 模拟并发消息发送
    let mut handles = Vec::new();

    for i in 0..15 {
        let limiter_clone = limiter.clone();
        let user_id_clone = user_id;
        let handle = tokio::spawn(async move {
            let result = limiter_clone.check_message_rate(user_id_clone).await;
            println!("Message {}: {:?}", i + 1, result.is_ok());
            result.is_ok()
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    let results: Vec<bool> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // 应该有10条消息成功，5条被限流
    let successful = results.iter().filter(|&&success| success).count();
    let rejected = results.len() - successful;

    println!(
        "Concurrent test: {} successful, {} rejected",
        successful, rejected
    );
    assert_eq!(successful, 10);
    assert_eq!(rejected, 5);
}

#[tokio::test]
async fn test_connection_lifecycle() {
    let redis_client = Arc::new(Client::open("redis://127.0.0.1:6379").unwrap());
    let limiter = MessageRateLimiter::new(redis_client.clone(), 10, 2);
    let user_id = UserId::from(Uuid::new_v4());

    // 添加连接
    assert!(limiter.add_connection(user_id).await.is_ok());
    assert!(limiter.add_connection(user_id).await.is_ok());

    let (_, connections) = limiter.get_user_status(user_id).await.unwrap();
    assert_eq!(connections, 2);

    // 移除连接
    limiter.remove_connection(user_id).await;
    let (_, connections) = limiter.get_user_status(user_id).await.unwrap();
    assert_eq!(connections, 1);

    // 应该可以再添加连接
    assert!(limiter.add_connection(user_id).await.is_ok());
    let (_, connections) = limiter.get_user_status(user_id).await.unwrap();
    assert_eq!(connections, 2);

    // 移除所有连接
    limiter.remove_connection(user_id).await;
    limiter.remove_connection(user_id).await;

    // 用户应该从连接map中被移除
    let status = limiter.get_user_status(user_id).await;
    if let Ok((_, connections)) = status {
        assert_eq!(connections, 0);
    }

    println!("Connection lifecycle test completed");
}
