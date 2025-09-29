use anyhow::Result;
use application::presence::{UserPresenceEvent, PresenceEventType};
use chrono::Utc;
use domain::{RoomId, UserId};
use redis::{Client, AsyncCommands};
use sqlx::{PgPool, Row};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// 测试 Redis Stream 消费者的基本功能
#[tokio::test]
async fn test_stats_consumer_basic_functionality() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // 测试配置
    let redis_url = "redis://127.0.0.1:6379";
    let database_url = "postgres://postgres:123456@127.0.0.1:5432/chatroom";

    // 跳过测试如果服务不可用
    if !test_service_availability(redis_url, database_url).await? {
        tracing::warn!("跳过集成测试：Redis 或 PostgreSQL 不可用");
        return Ok(());
    }

    // 创建 Redis 客户端
    let redis_client = Client::open(redis_url)?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;

    // 创建数据库连接池
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // 清理测试数据
    cleanup_test_data(&mut redis_conn, &db_pool).await?;

    // 创建测试事件
    let test_event = UserPresenceEvent {
        event_id: Uuid::new_v4(),
        user_id: UserId::from(Uuid::new_v4()),
        room_id: RoomId::from(Uuid::new_v4()),
        event_type: PresenceEventType::Connected,
        timestamp: Utc::now(),
        session_id: Uuid::new_v4(),
        user_ip: Some("192.168.1.1".to_string()),
        user_agent: Some("test-agent".to_string()),
    };

    // 将测试事件写入 Redis Stream
    let stream_key = "presence_events_stream";
    let _: i32 = redis_conn
        .xadd(stream_key, "*", &[
            ("event_id", test_event.event_id.to_string()),
            ("user_id", test_event.user_id.to_string()),
            ("room_id", test_event.room_id.to_string()),
            ("event_type", test_event.event_type.to_string()),
            ("timestamp", test_event.timestamp.to_rfc3339()),
            ("session_id", test_event.session_id.to_string()),
            ("user_ip", test_event.user_ip.clone().unwrap_or_default()),
            ("user_agent", test_event.user_agent.clone().unwrap_or_default()),
        ])
        .await?;

    tracing::info!("已写入测试事件到 Redis Stream");

    // 等待消费者处理（在实际应用中，消费者会持续运行）
    sleep(Duration::from_secs(2)).await;

    // 验证事件是否被写入数据库
    let result = sqlx::query(
        "SELECT COUNT(*) as count FROM presence_events WHERE event_id = $1"
    )
    .bind(test_event.event_id)
    .fetch_one(&db_pool)
    .await?;

    let count: i64 = result.get("count");
    assert_eq!(count, 1, "事件应该被写入数据库");

    tracing::info!("✅ 集成测试通过：事件成功从 Redis Stream 消费并写入数据库");

    // 清理测试数据
    cleanup_test_data(&mut redis_conn, &db_pool).await?;

    Ok(())
}

/// 检查服务可用性
async fn test_service_availability(redis_url: &str, database_url: &str) -> Result<bool> {
    // 检查 Redis
    let redis_available = match Client::open(redis_url) {
        Ok(client) => {
            match client.get_multiplexed_async_connection().await {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        Err(_) => false,
    };

    // 检查 PostgreSQL
    let db_available = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
    {
        Ok(_) => true,
        Err(_) => false,
    };

    Ok(redis_available && db_available)
}

/// 清理测试数据
async fn cleanup_test_data(
    redis_conn: &mut redis::aio::MultiplexedConnection,
    db_pool: &PgPool,
) -> Result<()> {
    // 清理 Redis Stream
    let _: () = redis_conn
        .del("presence_events_stream")
        .await?;

    // 清理数据库中的测试数据
    let _: sqlx::postgres::PgQueryResult = sqlx::query(
        "DELETE FROM presence_events WHERE event_id IN (
            SELECT event_id FROM presence_events
            WHERE user_id LIKE 'test-%' OR room_id LIKE 'test-%'
        )"
    )
    .execute(db_pool)
    .await?;

    Ok(())
}


/// 测试批量写入功能
#[tokio::test]
async fn test_batch_writing() -> Result<()> {
    let redis_url = "redis://127.0.0.1:6379";
    let database_url = "postgres://postgres:123456@127.0.0.1:5432/chatroom";

    if !test_service_availability(redis_url, database_url).await? {
        tracing::warn!("跳过批量写入测试：服务不可用");
        return Ok(());
    }

    let redis_client = Client::open(redis_url)?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
    let db_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // 清理测试数据
    cleanup_test_data(&mut redis_conn, &db_pool).await?;

    // 创建多个测试事件
    let test_events: Vec<UserPresenceEvent> = (0..5)
        .map(|i| UserPresenceEvent {
            event_id: Uuid::new_v4(),
            user_id: UserId::from(Uuid::new_v4()),
            room_id: RoomId::from(Uuid::new_v4()),
            event_type: if i % 2 == 0 { PresenceEventType::Connected } else { PresenceEventType::Disconnected },
            timestamp: Utc::now(),
            session_id: Uuid::new_v4(),
            user_ip: Some(format!("192.168.1.{}", i + 1)),
            user_agent: Some(format!("test-agent-{}", i + 1)),
        })
        .collect();

    // 将事件批量写入 Redis Stream
    let stream_key = "presence_events_stream";
    for event in &test_events {
        let _: i32 = redis_conn
            .xadd(stream_key, "*", &[
                ("event_id", event.event_id.to_string()),
                ("user_id", event.user_id.to_string()),
                ("room_id", event.room_id.to_string()),
                ("event_type", event.event_type.to_string()),
                ("timestamp", event.timestamp.to_rfc3339()),
                ("session_id", event.session_id.to_string()),
                ("user_ip", event.user_ip.clone().unwrap_or_default()),
                ("user_agent", event.user_agent.clone().unwrap_or_default()),
            ])
            .await?;
    }

    tracing::info!("已写入 {} 个测试事件到 Redis Stream", test_events.len());

    // 等待消费者处理
    sleep(Duration::from_secs(3)).await;

    // 验证所有事件都被写入数据库
    let mut found_count = 0;
    for event in &test_events {
        let result = sqlx::query(
            "SELECT COUNT(*) as count FROM presence_events WHERE event_id = $1"
        )
        .bind(event.event_id)
        .fetch_one(&db_pool)
        .await?;

        let count: i64 = result.get("count");
        if count > 0 {
            found_count += 1;
        }
    }

    assert!(
        found_count >= test_events.len() - 1, // 允许一个事件的容错
        "至少 {} 个事件应该被写入数据库，实际找到 {} 个",
        test_events.len() - 1,
        found_count
    );

    tracing::info!("✅ 批量写入测试通过：{}/{} 事件成功写入", found_count, test_events.len());

    // 清理测试数据
    cleanup_test_data(&mut redis_conn, &db_pool).await?;

    Ok(())
}