//! Redis Stream 事件写入集成测试
//!
//! 验证用户状态事件能够正确写入Redis Stream

use application::{RedisPresenceManager, PresenceManager};
use domain::{RoomId, UserId};
use std::sync::Arc;
use tokio::time::sleep;
use uuid::Uuid;
use redis::Client;
use chrono::Utc;

/// 测试配置
#[derive(Clone)]
struct StreamTestConfig {
    redis_url: String,
    room_id: RoomId,
    user_id: UserId,
    stream_name: String,
}

impl Default for StreamTestConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            room_id: RoomId::from(Uuid::new_v4()),
            user_id: UserId::from(Uuid::new_v4()),
            stream_name: "presence_events_stream".to_string(),
        }
    }
}

/// 测试辅助结构：封装Redis Stream操作
struct StreamTestHelper {
    redis_client: Arc<Client>,
    config: StreamTestConfig,
}

impl StreamTestHelper {
    async fn new(config: StreamTestConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let redis_client = Arc::new(Client::open(config.redis_url.clone())?);

        // 清理测试环境
        Self::cleanup_test_environment(&redis_client, &config.stream_name).await?;

        Ok(Self {
            redis_client,
            config,
        })
    }

    /// 读取流中的消息（使用结构化类型）
    async fn read_stream_messages(&self, count: i64) -> Result<Vec<(String, Vec<(String, Vec<(String, String)>)>)>, Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let messages: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREAD")
            .arg("COUNT")
            .arg(count)
            .arg("STREAMS")
            .arg(&self.config.stream_name)
            .arg("0")  // 从开头读取
            .query_async(&mut conn)
            .await?;

        Ok(messages)
    }

    /// 获取流长度
    async fn get_stream_length(&self) -> Result<i64, Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let length: i64 = redis::cmd("XLEN")
            .arg(&self.config.stream_name)
            .query_async(&mut conn)
            .await?;

        Ok(length)
    }

    /// 清理测试环境
    async fn cleanup_test_environment(
        redis_client: &Arc<Client>,
        stream_name: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = redis_client.get_multiplexed_async_connection().await?;

        // 删除流
        let _: () = redis::cmd("DEL").arg(stream_name).query_async(&mut conn).await?;

        Ok(())
    }

    /// 从结构化消息中提取事件数据
    fn extract_event_from_messages(&self, messages: &[(String, Vec<(String, Vec<(String, String)>)>)]) -> Vec<application::UserPresenceEvent> {
        let mut events = Vec::new();

        for (_stream_name, stream_messages) in messages {
            for (_message_id, fields) in stream_messages {
                if let Some(event) = self.extract_event_from_fields(fields) {
                    events.push(event);
                }
            }
        }

        events
    }

    /// 从字段数组中提取事件数据
    fn extract_event_from_fields(&self, fields: &[(String, String)]) -> Option<application::UserPresenceEvent> {
        let mut event_id = None;
        let mut user_id = None;
        let mut room_id = None;
        let mut event_type = None;
        let mut timestamp = None;
        let mut session_id = None;

        for (key, value) in fields {
            match key.as_str() {
                "event_id" => event_id = Uuid::parse_str(value).ok(),
                "user_id" => user_id = Some(UserId::from(Uuid::parse_str(value).ok()?)),
                "room_id" => room_id = Some(RoomId::from(Uuid::parse_str(value).ok()?)),
                "event_type" => event_type = Some(value.clone()),
                "timestamp" => timestamp = value.parse::<chrono::DateTime<chrono::Utc>>().ok(),
                "session_id" => session_id = Uuid::parse_str(value).ok(),
                _ => {}
            }
        }

        // 构建事件对象
        if let (Some(event_id), Some(user_id), Some(room_id), Some(event_type), Some(timestamp), Some(session_id)) =
            (event_id, user_id, room_id, event_type, timestamp, session_id) {

            let presence_type = match event_type.as_str() {
                "Connected" => application::PresenceEventType::Connected,
                "Disconnected" => application::PresenceEventType::Disconnected,
                "Heartbeat" => application::PresenceEventType::Heartbeat,
                _ => return None,
            };

            Some(application::UserPresenceEvent {
                event_id,
                user_id,
                room_id,
                event_type: presence_type,
                timestamp,
                session_id,
                user_ip: None,
                user_agent: None,
            })
        } else {
            None
        }
    }
}

/// 测试用户连接事件写入Redis Stream
#[tokio::test]
async fn test_user_connected_event_stream_writing() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamTestConfig::default();
    let helper = StreamTestHelper::new(config.clone()).await?;

    // 创建RedisPresenceManager
    let presence_manager = Arc::new(RedisPresenceManager::new(helper.redis_client.clone()));

    // 记录用户连接事件
    let event = application::UserPresenceEvent {
        event_id: Uuid::new_v4(),
        user_id: config.user_id,
        room_id: config.room_id,
        event_type: application::PresenceEventType::Connected,
        timestamp: Utc::now(),
        session_id: Uuid::new_v4(),
        user_ip: Some("127.0.0.1".to_string()),
        user_agent: Some("test-agent".to_string()),
    };

    presence_manager.record_presence_event(event.clone()).await?;

    // 等待写入完成
    sleep(tokio::time::Duration::from_millis(100)).await;

    // 检查流长度
    let stream_length = helper.get_stream_length().await?;
    assert_eq!(stream_length, 1, "应该有1条消息写入Redis Stream");

    // 读取流中的消息
    let messages = helper.read_stream_messages(10).await?;

    // 验证消息存在并解析事件
    let events = helper.extract_event_from_messages(&messages);
    assert_eq!(events.len(), 1, "应该解析出1个事件");

    let parsed_event = &events[0];
    assert_eq!(parsed_event.event_id, event.event_id);
    assert_eq!(parsed_event.user_id, event.user_id);
    assert_eq!(parsed_event.room_id, event.room_id);
    assert_eq!(parsed_event.event_type, event.event_type);
    assert_eq!(parsed_event.session_id, event.session_id);

    println!("✅ 用户连接事件Redis Stream写入测试通过");
    Ok(())
}

/// 测试用户断开事件写入Redis Stream
#[tokio::test]
async fn test_user_disconnected_event_stream_writing() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamTestConfig::default();
    let helper = StreamTestHelper::new(config.clone()).await?;

    let presence_manager = Arc::new(RedisPresenceManager::new(helper.redis_client.clone()));

    // 记录用户断开事件
    let event = application::UserPresenceEvent {
        event_id: Uuid::new_v4(),
        user_id: config.user_id,
        room_id: config.room_id,
        event_type: application::PresenceEventType::Disconnected,
        timestamp: Utc::now(),
        session_id: Uuid::new_v4(),
        user_ip: Some("192.168.1.1".to_string()),
        user_agent: Some("test-disconnect-agent".to_string()),
    };

    presence_manager.record_presence_event(event.clone()).await?;

    // 等待写入完成
    sleep(tokio::time::Duration::from_millis(100)).await;

    // 检查流长度
    let stream_length = helper.get_stream_length().await?;
    assert_eq!(stream_length, 1, "应该有1条消息写入Redis Stream");

    // 读取并验证消息
    let messages = helper.read_stream_messages(10).await?;
    let events = helper.extract_event_from_messages(&messages);

    assert_eq!(events.len(), 1, "应该解析出1个事件");
    let parsed_event = &events[0];

    assert_eq!(parsed_event.event_id, event.event_id);
    assert_eq!(parsed_event.user_id, event.user_id);
    assert_eq!(parsed_event.room_id, event.room_id);
    assert_eq!(parsed_event.event_type, event.event_type);

    println!("✅ 用户断开事件Redis Stream写入测试通过");
    Ok(())
}

/// 测试多个事件顺序写入
#[tokio::test]
async fn test_multiple_events_sequential_writing() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamTestConfig::default();
    let helper = StreamTestHelper::new(config.clone()).await?;

    let presence_manager = Arc::new(RedisPresenceManager::new(helper.redis_client.clone()));

    // 创建多个事件
    let events = vec![
        application::UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: config.user_id,
            room_id: config.room_id,
            event_type: application::PresenceEventType::Connected,
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: None,
            user_agent: None,
        },
        application::UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: UserId::from(Uuid::new_v4()),
            room_id: config.room_id,
            event_type: application::PresenceEventType::Connected,
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: None,
            user_agent: None,
        },
        application::UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: config.user_id,
            room_id: config.room_id,
            event_type: application::PresenceEventType::Disconnected,
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: None,
            user_agent: None,
        },
    ];

    // 顺序写入事件
    for event in &events {
        presence_manager.record_presence_event(event.clone()).await?;
        sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // 检查流长度
    let stream_length = helper.get_stream_length().await?;
    assert_eq!(stream_length, events.len() as i64, "应该写入{}个事件", events.len());

    // 读取所有消息
    let messages = helper.read_stream_messages(10).await?;
    let parsed_events = helper.extract_event_from_messages(&messages);

    // 验证事件数量和顺序
    assert_eq!(parsed_events.len(), events.len(), "应该解析出{}个事件", events.len());

    for (i, (parsed_event, original_event)) in parsed_events.iter().zip(events.iter()).enumerate() {
        assert_eq!(parsed_event.event_id, original_event.event_id, "第{}个事件的ID应该匹配", i + 1);
        assert_eq!(parsed_event.event_type, original_event.event_type, "第{}个事件的类型应该匹配", i + 1);
    }

    println!("✅ 多个事件顺序写入测试通过");
    Ok(())
}

/// 测试混合事件类型写入
#[tokio::test]
async fn test_mixed_event_types_writing() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamTestConfig::default();
    let helper = StreamTestHelper::new(config.clone()).await?;

    let presence_manager = Arc::new(RedisPresenceManager::new(helper.redis_client.clone()));

    // 创建不同用户的事件
    let user1 = UserId::from(Uuid::new_v4());
    let user2 = UserId::from(Uuid::new_v4());

    let events = vec![
        (user1, application::PresenceEventType::Connected),
        (user2, application::PresenceEventType::Connected),
        (user1, application::PresenceEventType::Disconnected),
        (user2, application::PresenceEventType::Disconnected),
    ];

    // 写入事件
    for (user_id, event_type) in &events {
        let event = application::UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: *user_id,
            room_id: config.room_id,
            event_type: *event_type,
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: None,
            user_agent: None,
        };

        presence_manager.record_presence_event(event).await?;
        sleep(tokio::time::Duration::from_millis(30)).await;
    }

    // 验证所有事件都被写入
    let stream_length = helper.get_stream_length().await?;
    assert_eq!(stream_length, events.len() as i64, "应该写入{}个事件", events.len());

    // 读取并验证事件
    let messages = helper.read_stream_messages(10).await?;
    let parsed_events = helper.extract_event_from_messages(&messages);

    assert_eq!(parsed_events.len(), events.len(), "应该解析出{}个事件", events.len());

    // 验证事件类型的正确性
    let mut connect_count = 0;
    let mut disconnect_count = 0;

    for event in &parsed_events {
        match event.event_type {
            application::PresenceEventType::Connected => connect_count += 1,
            application::PresenceEventType::Disconnected => disconnect_count += 1,
            application::PresenceEventType::Heartbeat => {
                // 忽略心跳事件，在此测试中不会产生
            }
        }
    }

    assert_eq!(connect_count, 2, "应该有2个连接事件");
    assert_eq!(disconnect_count, 2, "应该有2个断开事件");

    println!("✅ 混合事件类型写入测试通过");
    Ok(())
}

/// 测试大量事件写入的性能
#[tokio::test]
async fn test_bulk_events_writing() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamTestConfig::default();
    let helper = StreamTestHelper::new(config.clone()).await?;

    let presence_manager = Arc::new(RedisPresenceManager::new(helper.redis_client.clone()));

    let event_count = 50;
    let start_time = std::time::Instant::now();

    // 批量写入事件
    for i in 0..event_count {
        let event = application::UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: UserId::from(Uuid::new_v4()),
            room_id: config.room_id,
            event_type: if i % 2 == 0 {
                application::PresenceEventType::Connected
            } else {
                application::PresenceEventType::Disconnected
            },
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: None,
            user_agent: None,
        };

        presence_manager.record_presence_event(event).await?;

        // 减少延迟以提高性能
        if i % 10 == 0 {
            sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    let duration = start_time.elapsed();
    println!("写入{}个事件耗时: {:?}", event_count, duration);

    // 验证所有事件都被写入
    let stream_length = helper.get_stream_length().await?;
    assert_eq!(stream_length, event_count, "应该写入{}个事件", event_count);

    // 读取并验证部分事件
    let messages = helper.read_stream_messages(20).await?;
    let parsed_events = helper.extract_event_from_messages(&messages);

    assert!(!parsed_events.is_empty(), "应该能解析出事件");
    assert!(parsed_events.len() <= 20, "最多应该读取20个事件");

    println!("✅ 批量事件写入测试通过 ({}个事件, {:?})", event_count, duration);
    Ok(())
}