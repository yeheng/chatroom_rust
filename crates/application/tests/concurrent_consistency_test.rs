//! 并发数据一致性测试
//!
//! 验证在高并发场景下Redis-based系统的数据一致性

use application::{MessageRateLimiter, PresenceManager, RedisPresenceManager};
use domain::{RoomId, UserId};
use redis::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// 测试配置
struct TestConfig {
    redis_url: String,
    _database_url: String,
    room_id: RoomId,
    user_ids: Vec<UserId>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            _database_url: "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string(),
            room_id: RoomId::from(Uuid::new_v4()),
            user_ids: (0..10).map(|_| UserId::from(Uuid::new_v4())).collect(),
        }
    }
}

/// 测试辅助结构：封装测试服务
struct TestServices {
    redis_client: Arc<Client>,
    presence_manager: Arc<RedisPresenceManager>,
    rate_limiter: Arc<MessageRateLimiter>,
}

impl TestServices {
    async fn new(config: &TestConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let redis_client = Arc::new(Client::open(config.redis_url.clone())?);

        let presence_manager = Arc::new(RedisPresenceManager::new(redis_client.clone()));
        let rate_limiter = Arc::new(MessageRateLimiter::new(redis_client.clone(), 10, 3));

        Ok(Self {
            redis_client,
            presence_manager,
            rate_limiter,
        })
    }
}

/// 测试并发用户连接/断开连接的数据一致性
#[tokio::test]
async fn test_concurrent_connection_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let services = TestServices::new(&config).await?;

    // 清理测试环境
    cleanup_test_environment(&services.redis_client).await?;

    let room_id = config.room_id;
    let user_ids = &config.user_ids[..5]; // 使用前5个用户进行测试

    // 并发连接用户
    let connect_tasks: Vec<_> = user_ids
        .iter()
        .enumerate()
        .map(|(i, &user_id)| {
            let presence_manager = services.presence_manager.clone();
            let room_id = room_id;
            tokio::spawn(async move {
                // 模拟连接延迟
                sleep(Duration::from_millis(i as u64 * 10)).await;
                presence_manager.user_connected(room_id, user_id).await
            })
        })
        .collect();

    // 等待所有连接任务完成
    let connect_results: Vec<_> = futures::future::join_all(connect_tasks)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // 验证所有连接都成功
    assert!(connect_results.iter().all(|result| result.is_ok()));

    // 等待Redis同步
    sleep(Duration::from_millis(100)).await;

    // 验证在线用户数量
    let online_count = services.presence_manager.get_online_count(room_id).await?;
    assert_eq!(online_count, user_ids.len() as u64);

    // 并发断开连接
    let disconnect_tasks: Vec<_> = user_ids
        .iter()
        .enumerate()
        .map(|(i, &user_id)| {
            let presence_manager = services.presence_manager.clone();
            let room_id = room_id;
            tokio::spawn(async move {
                // 模拟断开延迟
                sleep(Duration::from_millis(i as u64 * 10)).await;
                presence_manager.user_disconnected(room_id, user_id).await
            })
        })
        .collect();

    // 等待所有断开任务完成
    let disconnect_results: Vec<_> = futures::future::join_all(disconnect_tasks)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // 验证所有断开都成功
    assert!(disconnect_results.iter().all(|result| result.is_ok()));

    // 等待Redis同步
    sleep(Duration::from_millis(100)).await;

    // 验证所有用户都已离线
    let online_count = services.presence_manager.get_online_count(room_id).await?;
    assert_eq!(online_count, 0);

    println!("✅ 并发连接/断开一致性测试通过");
    Ok(())
}

/// 测试并发限流的一致性
#[tokio::test]
async fn test_concurrent_rate_limiting_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let services = TestServices::new(&config).await?;

    // 清理测试环境
    cleanup_test_environment(&services.redis_client).await?;

    let user_id = config.user_ids[0];
    let rate_limiter = services.rate_limiter.clone();

    // 并发发送消息测试限流
    let message_tasks: Vec<_> = (0..20)
        .map(|i| {
            let rate_limiter = rate_limiter.clone();
            let user_id = user_id;
            tokio::spawn(async move {
                // 模拟消息发送间隔
                sleep(Duration::from_millis(i as u64 * 5)).await;
                rate_limiter.check_message_rate(user_id).await
            })
        })
        .collect();

    // 等待所有消息检查完成
    let message_results: Vec<_> = futures::future::join_all(message_tasks)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // 统计成功和失败的消息数量
    let successful_messages = message_results
        .iter()
        .filter(|result| result.is_ok())
        .count();
    let rejected_messages = message_results
        .iter()
        .filter(|result| result.is_err())
        .count();

    println!(
        "成功消息: {}, 被拒绝消息: {}",
        successful_messages, rejected_messages
    );

    // 验证限流效果（每分钟最多10条消息）
    assert!(successful_messages <= 10);
    assert!(rejected_messages >= 10); // 至少有10条消息应该被拒绝

    // 验证最终状态一致性
    let final_status = rate_limiter.get_user_status(user_id).await?;
    println!(
        "最终状态: 消息数={}, 连接数={}",
        final_status.0, final_status.1
    );

    println!("✅ 并发限流一致性测试通过");
    Ok(())
}

/// 测试混合并发操作（连接+消息+断开）的数据一致性
#[tokio::test]
async fn test_mixed_concurrent_operations_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let services = TestServices::new(&config).await?;

    // 清理测试环境
    cleanup_test_environment(&services.redis_client).await?;

    let room_id = config.room_id;
    let user_ids = &config.user_ids[..3]; // 使用3个用户

    // 创建混合操作任务
    let mut tasks = Vec::new();

    // 用户1：连接并发送消息
    for i in 0..5 {
        let presence_manager = services.presence_manager.clone();
        let rate_limiter = services.rate_limiter.clone();
        let room_id = room_id;
        let user_id = user_ids[0];
        tasks.push(tokio::spawn(async move {
            presence_manager
                .user_connected(room_id, user_id)
                .await
                .unwrap();
            sleep(Duration::from_millis(i as u64 * 20)).await;
            let _ = rate_limiter.check_message_rate(user_id).await;
        }));
    }

    // 用户2：发送消息并断开连接
    for i in 0..5 {
        let presence_manager = services.presence_manager.clone();
        let rate_limiter = services.rate_limiter.clone();
        let room_id = room_id;
        let user_id = user_ids[1];
        tasks.push(tokio::spawn(async move {
            let _ = rate_limiter.check_message_rate(user_id).await;
            sleep(Duration::from_millis(i as u64 * 20)).await;
            presence_manager
                .user_disconnected(room_id, user_id)
                .await
                .unwrap();
        }));
    }

    // 用户3：混合操作
    for i in 0..10 {
        let presence_manager = services.presence_manager.clone();
        let rate_limiter = services.rate_limiter.clone();
        let room_id = room_id;
        let user_id = user_ids[2];
        tasks.push(tokio::spawn(async move {
            if i % 2 == 0 {
                let _ = rate_limiter.check_message_rate(user_id).await;
            } else {
                presence_manager
                    .user_connected(room_id, user_id)
                    .await
                    .unwrap();
            }
            sleep(Duration::from_millis(i as u64 * 10)).await;
        }));
    }

    // 等待所有任务完成
    let _results: Vec<_> = futures::future::join_all(tasks)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // 等待Redis同步
    sleep(Duration::from_millis(200)).await;

    // 验证最终状态的一致性
    let online_users = services.presence_manager.get_online_users(room_id).await?;
    let online_count = services.presence_manager.get_online_count(room_id).await?;

    println!("混合操作后在线用户数: {}", online_count);
    println!("在线用户列表: {:?}", online_users);

    // 验证数据一致性：在线用户列表和数量应该匹配
    assert_eq!(online_users.len() as u64, online_count);

    // 验证用户状态一致性
    for &user_id in user_ids {
        let user_status = services
            .presence_manager
            .is_user_online(room_id, user_id)
            .await?;
        let in_list = online_users.contains(&user_id);

        // 两种查询方式应该得到一致的结果
        assert_eq!(user_status, in_list);
    }

    println!("✅ 混合并发操作一致性测试通过");
    Ok(())
}

/// 测试竞态条件下的数据一致性
#[tokio::test]
async fn test_race_condition_consistency() -> Result<(), Box<dyn std::error::Error>> {
    let config = TestConfig::default();
    let services = TestServices::new(&config).await?;

    // 清理测试环境
    cleanup_test_environment(&services.redis_client).await?;

    let room_id = config.room_id;
    let user_id = config.user_ids[0];
    let presence_manager = services.presence_manager.clone();

    // 创建竞态条件：同时进行连接和断开操作
    let mut race_tasks = Vec::new();

    // 同时启动多个连接和断开操作
    for i in 0..10 {
        let presence_manager = presence_manager.clone();
        let room_id = room_id;
        let user_id = user_id;
        let operation = if i % 2 == 0 { "connect" } else { "disconnect" };

        race_tasks.push(tokio::spawn(async move {
            sleep(Duration::from_millis(i as u64 * 5)).await;
            match operation {
                "connect" => presence_manager.user_connected(room_id, user_id).await,
                "disconnect" => presence_manager.user_disconnected(room_id, user_id).await,
                _ => Ok(()),
            }
        }));
    }

    // 等待所有竞态操作完成
    let _race_results: Vec<_> = futures::future::join_all(race_tasks)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // 等待Redis同步
    sleep(Duration::from_millis(200)).await;

    // 验证系统仍然处于一致状态
    let online_count = services.presence_manager.get_online_count(room_id).await?;
    let online_users = services.presence_manager.get_online_users(room_id).await?;

    println!("竞态条件后在线用户数: {}", online_count);

    // 数据一致性检查
    assert_eq!(online_users.len() as u64, online_count);

    // 验证用户状态
    let is_online = presence_manager.is_user_online(room_id, user_id).await?;
    let in_list = online_users.contains(&user_id);
    assert_eq!(is_online, in_list);

    println!("✅ 竞态条件一致性测试通过");
    Ok(())
}

/// 清理测试环境
async fn cleanup_test_environment(
    redis_client: &Arc<Client>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

    // 清理Redis中的测试数据
    let _: () = redis::cmd("FLUSHDB").query_async(&mut redis_conn).await?;

    Ok(())
}
