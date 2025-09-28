use application::delivery::DeliveryTracker;
use domain::{MessageId, UserId};
use infrastructure::{create_pg_pool, PgDeliveryTracker};
use sqlx::PgPool;
use uuid::Uuid;

async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string());

    let pool = create_pg_pool(&database_url, 5)
        .await
        .expect("Failed to create test database pool");

    // 运行迁移
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // 清理测试数据
    sqlx::query("DELETE FROM message_deliveries WHERE message_id IN ($1, $2)")
        .bind(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap())
        .bind(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap())
        .execute(&pool)
        .await
        .expect("Failed to clean test data");

    pool
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_message_delivery_tracking() {
    let pool = setup_test_db().await;
    let tracker = PgDeliveryTracker::new(pool);

    let msg_id = MessageId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let user_id = UserId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());

    // 标记消息已发送
    tracker.mark_sent(msg_id, user_id).await.unwrap();

    // 检查未送达消息列表
    let undelivered = tracker.get_undelivered(user_id).await.unwrap();
    assert!(undelivered.contains(&msg_id));

    // 标记消息已送达
    tracker.mark_delivered(msg_id, user_id).await.unwrap();

    // 检查未送达消息列表（应该为空）
    let undelivered = tracker.get_undelivered(user_id).await.unwrap();
    assert!(!undelivered.contains(&msg_id));
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_multiple_users_delivery() {
    let pool = setup_test_db().await;
    let tracker = PgDeliveryTracker::new(pool);

    let msg_id = MessageId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let user1_id = UserId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());
    let user2_id = UserId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap());

    // 向两个用户发送同一条消息
    tracker.mark_sent(msg_id, user1_id).await.unwrap();
    tracker.mark_sent(msg_id, user2_id).await.unwrap();

    // 只有user1确认收到
    tracker.mark_delivered(msg_id, user1_id).await.unwrap();

    // 检查各自的未送达列表
    let user1_undelivered = tracker.get_undelivered(user1_id).await.unwrap();
    let user2_undelivered = tracker.get_undelivered(user2_id).await.unwrap();

    assert!(!user1_undelivered.contains(&msg_id)); // user1已送达
    assert!(user2_undelivered.contains(&msg_id)); // user2未送达
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_cleanup_delivered_messages() {
    let pool = setup_test_db().await;
    let tracker = PgDeliveryTracker::new(pool);

    let msg_id = MessageId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());
    let user_id = UserId::from(Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap());

    // 发送并确认消息
    tracker.mark_sent(msg_id, user_id).await.unwrap();
    tracker.mark_delivered(msg_id, user_id).await.unwrap();

    // 清理1小时前的已送达消息（这里应该清理0条，因为消息刚刚送达）
    let cleaned = tracker.cleanup_delivered(1).await.unwrap();
    assert_eq!(cleaned, 0);

    // 清理所有已送达消息（设置为0小时前）
    let cleaned = tracker.cleanup_delivered(0).await.unwrap();
    assert!(cleaned > 0);
}
