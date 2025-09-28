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
async fn test_message_persistence() {
    let pool = setup_test_db().await;
    let repo = PgMessageRepository::new(pool.clone());

    let (room_id, sender_id) = create_test_data(&pool).await;
    let room_id = RoomId::from(room_id);
    let sender_id = UserId::from(sender_id);

    // 测试保存消息
    let message = create_test_message(room_id, sender_id, "Hello, World!");
    let message_id = repo.save_message(message.clone()).await.unwrap();

    assert_eq!(message_id, message.id);

    // 测试通过ID查找消息
    let retrieved = repo.find_by_id(message_id).await.unwrap().unwrap();
    assert_eq!(retrieved.content.as_str(), "Hello, World!");
    assert_eq!(retrieved.sender_id, sender_id);
    assert_eq!(retrieved.room_id, room_id);
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_get_recent_messages() {
    let pool = setup_test_db().await;
    let repo = PgMessageRepository::new(pool.clone());

    let (room_id, sender_id) = create_test_data(&pool).await;
    let room_id = RoomId::from(room_id);
    let sender_id = UserId::from(sender_id);

    // 保存多条消息
    for i in 1..=5 {
        let message = create_test_message(room_id, sender_id, &format!("Message {}", i));
        repo.save_message(message).await.unwrap();
        // 稍微延迟以确保时间戳不同
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // 获取最近的3条消息
    let recent_messages = repo.get_recent_messages(room_id, 3, None).await.unwrap();

    assert_eq!(recent_messages.len(), 3);
    // 消息应该按时间倒序排列（最新的在前）
    assert_eq!(recent_messages[0].content.as_str(), "Message 5");
    assert_eq!(recent_messages[1].content.as_str(), "Message 4");
    assert_eq!(recent_messages[2].content.as_str(), "Message 3");
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_get_messages_since() {
    let pool = setup_test_db().await;
    let repo = PgMessageRepository::new(pool.clone());

    let (room_id, sender_id) = create_test_data(&pool).await;
    let room_id = RoomId::from(room_id);
    let sender_id = UserId::from(sender_id);

    // 保存几条旧消息
    for i in 1..=3 {
        let message = create_test_message(room_id, sender_id, &format!("Old message {}", i));
        repo.save_message(message).await.unwrap();
    }

    // 等待一会儿以确保时间戳分离
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 记录当前时间作为分界点
    let cutoff_time = Utc::now();

    // 再等待一会儿以确保新消息的时间戳在分界点之后
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 保存几条新消息
    for i in 1..=2 {
        let message = create_test_message(room_id, sender_id, &format!("New message {}", i));
        repo.save_message(message).await.unwrap();
    }

    // 获取分界时间之后的消息
    let new_messages = repo.get_messages_since(room_id, cutoff_time).await.unwrap();

    assert_eq!(new_messages.len(), 2);
    // 消息应该按时间正序排列（旧的在前）
    assert_eq!(new_messages[0].content.as_str(), "New message 1");
    assert_eq!(new_messages[1].content.as_str(), "New message 2");
}

#[tokio::test]
#[ignore = "requires database"]
async fn test_performance_single_message_under_10ms() {
    let pool = setup_test_db().await;
    let repo = PgMessageRepository::new(pool.clone());

    let (room_id, sender_id) = create_test_data(&pool).await;
    let room_id = RoomId::from(room_id);
    let sender_id = UserId::from(sender_id);

    let message = create_test_message(room_id, sender_id, "Performance test message");

    let start = std::time::Instant::now();
    let _message_id = repo.save_message(message).await.unwrap();
    let duration = start.elapsed();

    // 验收标准：单条消息存储 < 10ms
    assert!(
        duration.as_millis() < 10,
        "Message save took {}ms, should be < 10ms",
        duration.as_millis()
    );
}
