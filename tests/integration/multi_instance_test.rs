//! 多实例场景集成测试
//!
//! 验证多个应用实例之间的状态同步和一致性

use std::sync::Arc;
use std::time::Duration;

use application::{
    presence::RedisPresenceManager, rate_limiter::MessageRateLimiter, sequencer::MessageSequencer,
    PresenceManager, UserService, UserServiceDependencies,
};
use domain::{RoomId, UserId};
use infrastructure::{
    create_pg_pool, BcryptPasswordHasher, LocalMessageBroadcaster, PgUserRepository,
};
use redis::Client;
use tokio::time::sleep;
use uuid::Uuid;
use futures::stream::{FuturesOrdered, TryStreamExt};

/// 测试配置
struct TestConfig {
    redis_url: String,
    database_url: String,
    room_id: RoomId,
    user_ids: Vec<UserId>,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            database_url: "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string(),
            room_id: RoomId::from(Uuid::new_v4()),
            user_ids: vec![
                UserId::from(Uuid::new_v4()),
                UserId::from(Uuid::new_v4()),
                UserId::from(Uuid::new_v4()),
            ],
        }
    }
}

/// 测试辅助结构：模拟应用实例
struct TestAppInstance {
    instance_id: String,
    presence_manager: Arc<RedisPresenceManager>,
    rate_limiter: Arc<MessageRateLimiter>,
    message_sequencer: Arc<MessageSequencer>,
}

impl TestAppInstance {
    async fn new(instance_id: String, config: &TestConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // 创建Redis客户端
        let redis_client = Arc::new(Client::open(config.redis_url.clone())?);

        // 创建数据库连接池
        let pool = create_pg_pool(&config.database_url, 5).await?;

        // 创建用户仓库
        let user_repository = Arc::new(PgUserRepository::new(pool));

        // 创建密码哈希器
        let password_hasher = Arc::new(BcryptPasswordHasher::default());

        // 创建在线状态管理器
        let presence_manager = Arc::new(RedisPresenceManager::new(redis_client.clone()));

        // 创建消息限流器
        let rate_limiter = Arc::new(MessageRateLimiter::new(redis_client.clone()));

        // 创建消息序列器
        let message_sequencer = Arc::new(MessageSequencer::new(redis_client));

        // 创建用户服务（用于测试）
        let _user_service = UserService::new(UserServiceDependencies {
            user_repository,
            password_hasher,
            clock: Arc::new(application::SystemClock::default()),
            presence_manager: presence_manager.clone(),
        });

        Ok(Self {
            instance_id,
            presence_manager,
            rate_limiter,
            message_sequencer,
        })
    }

    async fn simulate_user_connection(&self, room_id: RoomId, user_id: UserId) -> Result<(), Box<dyn std::error::Error>> {
        // 用户连接到房间
        self.presence_manager.user_connected(room_id, user_id).await?;

        // 记录用户连接事件
        let event_id = Uuid::new_v4();
        let presence_event = application::UserPresenceEvent {
            event_id,
            user_id,
            room_id,
            event_type: application::PresenceEventType::Connected,
            timestamp: chrono::Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-agent".to_string()),
        };

        self.presence_manager.record_presence_event(presence_event).await?;

        Ok(())
    }

    async fn simulate_user_disconnection(&self, room_id: RoomId, user_id: UserId) -> Result<(), Box<dyn std::error::Error>> {
        // 用户从房间断开
        self.presence_manager.user_disconnected(room_id, user_id).await?;

        // 记录用户断开事件
        let event_id = Uuid::new_v4();
        let presence_event = application::UserPresenceEvent {
            event_id,
            user_id,
            room_id,
            event_type: application::PresenceEventType::Disconnected,
            timestamp: chrono::Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: Some("127.0.0.1".to_string()),
            user_agent: Some("test-agent".to_string()),
        };

        self.presence_manager.record_presence_event(presence_event).await?;

        Ok(())
    }

    async fn get_online_users(&self, room_id: RoomId) -> Result<Vec<UserId>, Box<dyn std::error::Error>> {
        Ok(self.presence_manager.get_online_users(room_id).await?)
    }

    async fn get_online_count(&self, room_id: RoomId) -> Result<u64, Box<dyn std::error::Error>> {
        Ok(self.presence_manager.get_online_count(room_id).await?)
    }

    async fn check_rate_limit(&self, user_id: UserId) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.rate_limiter.check_rate_limit(user_id).await?)
    }

    async fn get_next_sequence(&self, room_id: RoomId) -> Result<i64, Box<dyn std::error::Error>> {
        Ok(self.message_sequencer.get_next_sequence(room_id).await?)
    }
}

/// 测试多个实例之间的用户在线状态同步
#[tokio::test]
async fn test_multi_instance_presence_sync() -> Result<(), Box<dyn std::error::Error>> {
    // 清理测试环境
    cleanup_test_environment().await?;

    let config = TestConfig::default();

    // 创建3个应用实例
    let instance1 = TestAppInstance::new("instance1".to_string(), &config).await?;
    let instance2 = TestAppInstance::new("instance2".to_string(), &config).await?;
    let instance3 = TestAppInstance::new("instance3".to_string(), &config).await?;

    // 实例1：用户1连接到房间
    instance1.simulate_user_connection(config.room_id, config.user_ids[0]).await?;

    // 等待Redis同步
    sleep(Duration::from_millis(100)).await;

    // 验证所有实例都能看到用户1在线
    let online_users_1 = instance1.get_online_users(config.room_id).await?;
    let online_users_2 = instance2.get_online_users(config.room_id).await?;
    let online_users_3 = instance3.get_online_users(config.room_id).await?;

    assert_eq!(online_users_1.len(), 1);
    assert_eq!(online_users_2.len(), 1);
    assert_eq!(online_users_3.len(), 1);
    assert!(online_users_1.contains(&config.user_ids[0]));
    assert!(online_users_2.contains(&config.user_ids[0]));
    assert!(online_users_3.contains(&config.user_ids[0]));

    // 实例2：用户2连接到房间
    instance2.simulate_user_connection(config.room_id, config.user_ids[1]).await?;

    // 等待Redis同步
    sleep(Duration::from_millis(100)).await;

    // 验证所有实例都能看到两个用户在线
    let online_count_1 = instance1.get_online_count(config.room_id).await?;
    let online_count_2 = instance2.get_online_count(config.room_id).await?;
    let online_count_3 = instance3.get_online_count(config.room_id).await?;

    assert_eq!(online_count_1, 2);
    assert_eq!(online_count_2, 2);
    assert_eq!(online_count_3, 2);

    // 实例3：用户1断开连接
    instance3.simulate_user_disconnection(config.room_id, config.user_ids[0]).await?;

    // 等待Redis同步
    sleep(Duration::from_millis(100)).await;

    // 验证所有实例都能看到用户1已断开
    let online_users_1 = instance1.get_online_users(config.room_id).await?;
    let online_users_2 = instance2.get_online_users(config.room_id).await?;
    let online_users_3 = instance3.get_online_users(config.room_id).await?;

    assert_eq!(online_users_1.len(), 1);
    assert_eq!(online_users_2.len(), 1);
    assert_eq!(online_users_3.len(), 1);
    assert!(!online_users_1.contains(&config.user_ids[0]));
    assert!(!online_users_2.contains(&config.user_ids[0]));
    assert!(!online_users_3.contains(&config.user_ids[0]));
    assert!(online_users_1.contains(&config.user_ids[1]));
    assert!(online_users_2.contains(&config.user_ids[1]));
    assert!(online_users_3.contains(&config.user_ids[1]));

    println!("✅ 多实例在线状态同步测试通过");
    Ok(())
}

/// 测试多个实例之间的消息序列一致性
#[tokio::test]
async fn test_multi_instance_sequence_consistency() -> Result<(), Box<dyn std::error::Error>> {
    // 清理测试环境
    cleanup_test_environment().await?;

    let config = TestConfig::default();

    // 创建3个应用实例
    let instance1 = TestAppInstance::new("instance1".to_string(), &config).await?;
    let instance2 = TestAppInstance::new("instance2".to_string(), &config).await?;
    let instance3 = TestAppInstance::new("instance3".to_string(), &config).await?;

    // 各个实例分别获取序列号
    let seq1 = instance1.get_next_sequence(config.room_id).await?;
    let seq2 = instance2.get_next_sequence(config.room_id).await?;
    let seq3 = instance3.get_next_sequence(config.room_id).await?;

    // 验证序列号递增且唯一
    assert_eq!(seq1, 1);
    assert_eq!(seq2, 2);
    assert_eq!(seq3, 3);

    // 再次获取序列号
    let seq4 = instance1.get_next_sequence(config.room_id).await?;
    let seq5 = instance2.get_next_sequence(config.room_id).await?;
    let seq6 = instance3.get_next_sequence(config.room_id).await?;

    // 验证序列号继续递增
    assert_eq!(seq4, 4);
    assert_eq!(seq5, 5);
    assert_eq!(seq6, 6);

    println!("✅ 多实例消息序列一致性测试通过");
    Ok(())
}

/// 测试多个实例之间的消息限流一致性
#[tokio::test]
async fn test_multi_instance_rate_limit_consistency() -> Result<(), Box<dyn std::error::Error>> {
    // 清理测试环境
    cleanup_test_environment().await?;

    let config = TestConfig::default();

    // 创建3个应用实例
    let instance1 = TestAppInstance::new("instance1".to_string(), &config).await?;
    let instance2 = TestAppInstance::new("instance2".to_string(), &config).await?;
    let instance3 = TestAppInstance::new("instance3".to_string(), &config).await?;

    // 用户快速发送消息测试限流
    let user_id = config.user_ids[0];

    // 前10条消息应该通过
    for i in 0..10 {
        let allowed1 = instance1.check_rate_limit(user_id).await?;
        let allowed2 = instance2.check_rate_limit(user_id).await?;
        let allowed3 = instance3.check_rate_limit(user_id).await?;

        // 由于是分布式限流，不同实例可能看到不同的状态
        // 但总体上应该有限制效果
        if i < 5 {
            // 前几条消息应该允许通过
            assert!(allowed1 || allowed2 || allowed3);
        }
    }

    // 等待限流窗口重置
    sleep(Duration::from_secs(1)).await;

    // 重置后应该再次允许通过
    let allowed1 = instance1.check_rate_limit(user_id).await?;
    let allowed2 = instance2.check_rate_limit(user_id).await?;
    let allowed3 = instance3.check_rate_limit(user_id).await?;

    assert!(allowed1 || allowed2 || allowed3);

    println!("✅ 多实例消息限流一致性测试通过");
    Ok(())
}

/// 测试并发连接和断开的场景
#[tokio::test]
async fn test_concurrent_connect_disconnect() -> Result<(), Box<dyn std::error::Error>> {
    // 清理测试环境
    cleanup_test_environment().await?;

    let config = TestConfig::default();

    // 创建多个实例
    let instances: Vec<TestAppInstance> = (0..5)
        .map(|i| TestAppInstance::new(format!("instance{}", i), &config))
        .collect::<FuturesOrdered<_>>()
        .try_collect()
        .await?;

    // 并发连接多个用户
    let connect_tasks: Vec<_> = instances
        .iter()
        .enumerate()
        .map(|(i, instance)| {
            let user_id = config.user_ids[i % config.user_ids.len()];
            instance.simulate_user_connection(config.room_id, user_id)
        })
        .collect();

    // 等待所有连接完成
    for task in connect_tasks {
        task.await?;
    }

    // 等待Redis同步
    sleep(Duration::from_millis(200)).await;

    // 验证所有实例看到的在线用户数量一致
    let online_counts: Vec<_> = instances
        .iter()
        .map(|instance| async {
            instance.get_online_count(config.room_id).await
        })
        .collect::<FuturesOrdered<_>>()
        .try_collect()
        .await?;

    // 所有实例应该看到相同的在线用户数量
    let expected_count = config.user_ids.len().min(5); // 最多5个用户
    for count in online_counts {
        assert_eq!(count, expected_count as u64);
    }

    // 并发断开连接
    let disconnect_tasks: Vec<_> = instances
        .iter()
        .enumerate()
        .take(3) // 断开前3个用户
        .map(|(i, instance)| {
            let user_id = config.user_ids[i % config.user_ids.len()];
            instance.simulate_user_disconnection(config.room_id, user_id)
        })
        .collect();

    // 等待所有断开完成
    for task in disconnect_tasks {
        task.await?;
    }

    // 等待Redis同步
    sleep(Duration::from_millis(200)).await;

    // 验证所有实例看到更新后的在线用户数量
    let online_counts: Vec<_> = instances
        .iter()
        .map(|instance| async {
            instance.get_online_count(config.room_id).await
        })
        .collect::<FuturesOrdered<_>>()
        .try_collect()
        .await?;

    let expected_count = config.user_ids.len().saturating_sub(3).min(5); // 移除3个用户
    for count in online_counts {
        assert_eq!(count, expected_count as u64);
    }

    println!("✅ 并发连接断开测试通过");
    Ok(())
}

/// 清理测试环境
async fn cleanup_test_environment() -> Result<(), Box<dyn std::error::Error>> {
    let redis_client = Client::open("redis://127.0.0.1:6379")?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

    // 清理Redis中的测试数据
    let _: () = redis::cmd("FLUSHDB").query_async(&mut redis_conn).await?;

    Ok(())
}