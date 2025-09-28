use application::repository::MessageRepository;
use chrono::Utc;
use domain::{Message, MessageContent, MessageId, MessageType, RoomId, UserId};
use infrastructure::{create_pg_pool, PgMessageRepository};
use sqlx::PgPool;
use time::OffsetDateTime;
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

    pool
}

async fn create_test_data(pool: &PgPool) -> (Uuid, Uuid) {
    // 使用随机UUID避免测试冲突
    let test_user_id = Uuid::new_v4();
    let test_room_id = Uuid::new_v4();

    // 创建测试用户
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, status)
         VALUES ($1, $2, $3, 'hashed_password', 'active'::user_status)",
    )
    .bind(test_user_id)
    .bind(format!(
        "testuser_{}",
        test_user_id.to_string()[0..8].to_string()
    ))
    .bind(format!(
        "test_{}@example.com",
        test_user_id.to_string()[0..8].to_string()
    ))
    .execute(pool)
    .await
    .expect("Failed to create test user");

    // 创建测试房间
    sqlx::query(
        "INSERT INTO chat_rooms (id, name, owner_id, is_private)
         VALUES ($1, $2, $3, FALSE)",
    )
    .bind(test_room_id)
    .bind(format!(
        "test_room_{}",
        test_room_id.to_string()[0..8].to_string()
    ))
    .bind(test_user_id)
    .execute(pool)
    .await
    .expect("Failed to create test room");

    (test_room_id, test_user_id)
}

fn create_test_message(room_id: RoomId, sender_id: UserId, content: &str) -> Message {
    Message {
        id: MessageId::from(Uuid::new_v4()),
        room_id,
        sender_id,
        content: MessageContent::new(content.to_string()).unwrap(),
        message_type: MessageType::Text,
        reply_to: None,
        created_at: OffsetDateTime::now_utc(),
        last_revision: None,
        is_deleted: false,
    }
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_admin_message_history() {
    let pool = setup_test_db().await;
    let repo = PgMessageRepository::new(pool.clone());

    let (room_id, sender_id) = create_test_data(&pool).await;
    let room_id = RoomId::from(room_id);
    let sender_id = UserId::from(sender_id);

    // 创建测试消息
    let mut messages = Vec::new();
    for i in 1..=5 {
        let message = create_test_message(room_id, sender_id, &format!("Test message {}", i));
        repo.save_message(message.clone()).await.unwrap();
        messages.push(message);

        // 稍微延迟以确保时间戳不同
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // 软删除一条消息
    let deleted_message_id = messages[2].id;
    sqlx::query("UPDATE messages SET is_deleted = TRUE WHERE id = $1")
        .bind(Uuid::from(deleted_message_id))
        .execute(&pool)
        .await
        .expect("Failed to delete message");

    // 测试管理员查询（不包含已删除消息）
    let history = repo
        .get_admin_message_history(room_id, None, Some(10), false)
        .await
        .unwrap();

    assert_eq!(history.len(), 4); // 5条消息减去1条删除的
                                  // 应该按时间倒序排列（最新的在前）
    assert_eq!(history[0].content.as_str(), "Test message 5");
    assert_eq!(history[3].content.as_str(), "Test message 1");

    // 测试管理员查询（包含已删除消息）
    let history_with_deleted = repo
        .get_admin_message_history(room_id, None, Some(10), true)
        .await
        .unwrap();

    assert_eq!(history_with_deleted.len(), 5); // 包含删除的消息
                                               // 检查已删除的消息确实存在
    let deleted_msg = history_with_deleted
        .iter()
        .find(|m| m.id == deleted_message_id)
        .expect("Deleted message should be present");
    assert_eq!(deleted_msg.is_deleted, true);
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_admin_message_history_with_time_filter() {
    let pool = setup_test_db().await;
    let repo = PgMessageRepository::new(pool.clone());

    let (room_id, sender_id) = create_test_data(&pool).await;
    let room_id = RoomId::from(room_id);
    let sender_id = UserId::from(sender_id);

    // 创建一些旧消息
    for i in 1..=3 {
        let message = create_test_message(room_id, sender_id, &format!("Old message {}", i));
        repo.save_message(message).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // 记录分界时间
    let cutoff_time = Utc::now();
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 创建一些新消息
    for i in 1..=2 {
        let message = create_test_message(room_id, sender_id, &format!("New message {}", i));
        repo.save_message(message).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // 测试时间过滤：只获取分界时间之前的消息
    let old_messages = repo
        .get_admin_message_history(room_id, Some(cutoff_time), Some(10), false)
        .await
        .unwrap();

    assert_eq!(old_messages.len(), 3);
    // 验证都是旧消息
    for msg in &old_messages {
        assert!(msg.content.as_str().starts_with("Old message"));
    }
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_admin_message_history_limit() {
    let pool = setup_test_db().await;
    let repo = PgMessageRepository::new(pool.clone());

    let (room_id, sender_id) = create_test_data(&pool).await;
    let room_id = RoomId::from(room_id);
    let sender_id = UserId::from(sender_id);

    // 创建10条消息
    for i in 1..=10 {
        let message = create_test_message(room_id, sender_id, &format!("Message {}", i));
        repo.save_message(message).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    }

    // 测试限制数量
    let limited_messages = repo
        .get_admin_message_history(room_id, None, Some(3), false)
        .await
        .unwrap();

    assert_eq!(limited_messages.len(), 3);
    // 应该是最新的3条
    assert_eq!(limited_messages[0].content.as_str(), "Message 10");
    assert_eq!(limited_messages[1].content.as_str(), "Message 9");
    assert_eq!(limited_messages[2].content.as_str(), "Message 8");
}
