//! 端到端事件流集成测试
//!
//! 验证：RedisPresenceManager -> Redis Stream -> Stats Consumer -> PostgreSQL

use application::{PresenceEventType, PresenceManager, RedisPresenceManager, UserPresenceEvent};
use chrono::Utc;
use config::AppConfig;
use domain::{RoomId, UserId};
use redis::Client as RedisClient;
use stats_consumer::{create_event_storage, EventStorage, PgEventStorage};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// 测试配置
#[derive(Clone)]
struct E2ETestConfig {
    redis_url: String,
    database_url: String,
    stream_name: String,
    room_id: RoomId,
    user_id: UserId,
}

impl Default for E2ETestConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://test:test@localhost:5432/chatroom_test".to_string()
            }),
            stream_name: "presence_events_e2e_test".to_string(),
            room_id: RoomId::from(Uuid::new_v4()),
            user_id: UserId::from(Uuid::new_v4()),
        }
    }
}

/// 端到端测试辅助结构
struct E2ETestHelper {
    redis_client: Arc<RedisClient>,
    presence_manager: Arc<dyn PresenceManager>,
    event_storage: Arc<dyn EventStorage>,
    config: E2ETestConfig,
}

impl E2ETestHelper {
    async fn new(config: E2ETestConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // 创建 Redis 客户端
        let redis_client = Arc::new(RedisClient::open(config.redis_url.clone())?);

        // 创建带自定义流名称的 PresenceManager
        let presence_manager = Arc::new(RedisPresenceManager::with_stream_name(
            redis_client.clone(),
            config.stream_name.clone(),
        ));

        // 创建数据库连接池
        let pg_pool = infrastructure::create_pg_pool(&config.database_url, 5).await?;

        // 运行迁移
        sqlx::migrate!("../../../migrations").run(&pg_pool).await?;

        // 创建事件存储
        let event_storage = create_event_storage(pg_pool);

        // 清理测试环境
        Self::cleanup_test_environment(&redis_client, &config.stream_name).await?;

        Ok(Self {
            redis_client,
            presence_manager,
            event_storage,
            config,
        })
    }

    /// 清理测试环境
    async fn cleanup_test_environment(
        redis_client: &Arc<RedisClient>,
        stream_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = redis_client.get_multiplexed_async_connection().await?;

        // 删除流
        let _: () = redis::cmd("DEL")
            .arg(stream_name)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }

    /// 模拟 Stats Consumer 处理流中的事件
    async fn process_stream_events(&self) -> Result<usize, Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 读取流中的所有消息
        let messages: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREAD")
            .arg("COUNT")
            .arg(100)
            .arg("STREAMS")
            .arg(&self.config.stream_name)
            .arg("0") // 从开头读取
            .query_async(&mut conn)
            .await?;

        if messages.is_empty() {
            return Ok(0);
        }

        let mut events = Vec::new();

        // 解析消息
        for (_stream_name, stream_messages) in &messages {
            for (_message_id, fields) in stream_messages {
                if let Some(event) = self.parse_event_from_fields(fields) {
                    events.push(event);
                }
            }
        }

        if events.is_empty() {
            return Ok(0);
        }

        // 批量写入数据库
        self.event_storage.insert_events(&events).await?;

        Ok(events.len())
    }

    /// 从字段数组中解析事件
    fn parse_event_from_fields(&self, fields: &[(String, String)]) -> Option<UserPresenceEvent> {
        let mut event_id = None;
        let mut user_id = None;
        let mut room_id = None;
        let mut event_type = None;
        let mut timestamp = None;
        let mut session_id = None;
        let mut user_ip = None;
        let mut user_agent = None;

        for (key, value) in fields {
            match key.as_str() {
                "event_id" => event_id = Uuid::parse_str(value).ok(),
                "user_id" => user_id = Some(UserId::from(Uuid::parse_str(value).ok()?)),
                "room_id" => room_id = Some(RoomId::from(Uuid::parse_str(value).ok()?)),
                "event_type" => event_type = Some(value.clone()),
                "timestamp" => timestamp = value.parse::<chrono::DateTime<chrono::Utc>>().ok(),
                "session_id" => session_id = Uuid::parse_str(value).ok(),
                "user_ip" => {
                    user_ip = if value.is_empty() {
                        None
                    } else {
                        Some(value.clone())
                    }
                }
                "user_agent" => {
                    user_agent = if value.is_empty() {
                        None
                    } else {
                        Some(value.clone())
                    }
                }
                _ => {}
            }
        }

        // 构建事件对象
        if let (
            Some(event_id),
            Some(user_id),
            Some(room_id),
            Some(event_type),
            Some(timestamp),
            Some(session_id),
        ) = (
            event_id, user_id, room_id, event_type, timestamp, session_id,
        ) {
            let presence_type = match event_type.as_str() {
                "Connected" => PresenceEventType::Connected,
                "Disconnected" => PresenceEventType::Disconnected,
                "Heartbeat" => PresenceEventType::Heartbeat,
                _ => return None,
            };

            Some(UserPresenceEvent {
                event_id,
                user_id,
                room_id,
                event_type: presence_type,
                timestamp,
                session_id,
                user_ip,
                user_agent,
            })
        } else {
            None
        }
    }

    /// 获取数据库中的事件数量
    async fn get_database_event_count(&self) -> Result<i64, Box<dyn std::error::Error>> {
        let count = self.event_storage.get_event_count().await?;
        Ok(count)
    }
}

/// 测试完整的事件流：PresenceManager -> Redis Stream -> Database
#[tokio::test]
async fn test_e2e_event_flow() -> Result<(), Box<dyn std::error::Error>> {
    let config = E2ETestConfig::default();
    let helper = E2ETestHelper::new(config.clone()).await?;

    // 1. 创建并记录事件
    let event = UserPresenceEvent {
        event_id: Uuid::new_v4(),
        user_id: config.user_id,
        room_id: config.room_id,
        event_type: PresenceEventType::Connected,
        timestamp: Utc::now(),
        session_id: Uuid::new_v4(),
        user_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-agent".to_string()),
    };

    // 2. 通过 PresenceManager 记录事件（写入 Redis Stream）
    helper
        .presence_manager
        .record_presence_event(event.clone())
        .await?;

    // 等待写入完成
    sleep(Duration::from_millis(100)).await;

    // 3. 模拟 Stats Consumer 处理事件（从 Redis Stream 读取并写入数据库）
    let processed_count = helper.process_stream_events().await?;
    assert_eq!(processed_count, 1, "应该处理1个事件");

    // 4. 验证事件已写入数据库
    let db_count = helper.get_database_event_count().await?;
    assert_eq!(db_count, 1, "数据库中应该有1个事件");

    println!("✅ 端到端事件流测试通过");
    Ok(())
}

/// 测试多个事件的端到端流处理
#[tokio::test]
async fn test_e2e_multiple_events_flow() -> Result<(), Box<dyn std::error::Error>> {
    let config = E2ETestConfig::default();
    let helper = E2ETestHelper::new(config.clone()).await?;

    // 创建多个事件
    let events = vec![
        UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: config.user_id,
            room_id: config.room_id,
            event_type: PresenceEventType::Connected,
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: Some("192.168.1.1".to_string()),
            user_agent: Some("browser-1".to_string()),
        },
        UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: UserId::from(Uuid::new_v4()),
            room_id: config.room_id,
            event_type: PresenceEventType::Connected,
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: Some("192.168.1.2".to_string()),
            user_agent: Some("browser-2".to_string()),
        },
        UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: config.user_id,
            room_id: config.room_id,
            event_type: PresenceEventType::Disconnected,
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: Some("192.168.1.1".to_string()),
            user_agent: Some("browser-1".to_string()),
        },
    ];

    // 逐个记录事件
    for event in &events {
        helper
            .presence_manager
            .record_presence_event(event.clone())
            .await?;
        sleep(Duration::from_millis(50)).await;
    }

    // 处理所有事件
    let processed_count = helper.process_stream_events().await?;
    assert_eq!(
        processed_count,
        events.len(),
        "应该处理{}个事件",
        events.len()
    );

    // 验证数据库中的事件数量
    let db_count = helper.get_database_event_count().await?;
    assert_eq!(
        db_count,
        events.len() as i64,
        "数据库中应该有{}个事件",
        events.len()
    );

    println!("✅ 多事件端到端流测试通过");
    Ok(())
}
