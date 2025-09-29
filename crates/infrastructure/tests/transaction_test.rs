/// 简化的事务测试 - 验证事务性的create_room操作
use std::sync::Arc;
use tokio;

use application::{
    services::{ChatService, ChatServiceDependencies, CreateRoomRequest},
    repository::{ChatRoomRepository, RoomMemberRepository},
    Clock, MessageBroadcaster, PasswordHasher, ApplicationError,
    broadcaster::BroadcastError
};
use async_trait::async_trait;
use domain::{ChatRoomVisibility, RoomRole};
use infrastructure::{SimpleTransactionManager, PgStorage};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

/// 测试用的Clock实现
#[derive(Clone)]
struct TestClock {
    now: OffsetDateTime,
}

impl TestClock {
    fn new() -> Self {
        Self {
            now: OffsetDateTime::now_utc(),
        }
    }
}

impl Clock for TestClock {
    fn now(&self) -> OffsetDateTime {
        self.now
    }
}

/// 测试用的密码哈希器
#[derive(Clone)]
struct TestPasswordHasher;

#[async_trait]
impl PasswordHasher for TestPasswordHasher {
    async fn hash(&self, password: &str) -> Result<domain::PasswordHash, application::PasswordHasherError> {
        Ok(domain::PasswordHash::new(format!("hashed_{}", password)).unwrap())
    }

    async fn verify(&self, password: &str, hash: &domain::PasswordHash) -> Result<bool, application::PasswordHasherError> {
        Ok(hash.as_str() == &format!("hashed_{}", password))
    }
}

/// 测试用的消息广播器
#[derive(Clone)]
struct TestBroadcaster;

#[async_trait]
impl MessageBroadcaster for TestBroadcaster {
    async fn broadcast(&self, _broadcast: application::MessageBroadcast) -> Result<(), BroadcastError> {
        Ok(())
    }

    async fn subscribe(&self, _room_id: domain::RoomId) -> Result<application::MessageStream, BroadcastError> {
        // 测试中不需要实际的订阅功能
        Err(BroadcastError::failed("Not implemented for test"))
    }
}

async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string());

    infrastructure::create_pg_pool(&database_url, 5).await.unwrap()
}

#[tokio::test]
async fn test_transactional_create_room() {
    let pool = setup_test_db().await;
    let storage = PgStorage::new(pool.clone());
    let tx_manager = SimpleTransactionManager::new(pool);

    let chat_service = ChatService::new(ChatServiceDependencies {
        room_repository: storage.room_repository.clone(),
        member_repository: storage.member_repository.clone(),
        message_repository: storage.message_repository.clone(),
        password_hasher: Arc::new(TestPasswordHasher),
        clock: Arc::new(TestClock::new()),
        broadcaster: Arc::new(TestBroadcaster),
        transaction_manager: Some(Arc::new(tx_manager)),
    });

    let request = CreateRoomRequest {
        name: "测试房间".to_string(),
        owner_id: Uuid::new_v4(),
        visibility: ChatRoomVisibility::Public,
        password: None,
    };

    // 测试正常情况
    let result = chat_service.create_room(request.clone()).await;

    if let Err(ref e) = result {
        eprintln!("创建房间失败: {:?}", e);
    }

    assert!(result.is_ok(), "房间创建应该成功: {:?}", result.err());

    let room = result.unwrap();

    // 验证房间存在
    let found_room = storage.room_repository.find_by_id(room.id).await.unwrap();
    assert!(found_room.is_some(), "房间应该存在于数据库中");

    // 验证成员存在
    let found_member = storage.member_repository.find_member(room.id, room.owner_id).await.unwrap();
    assert!(found_member.is_some(), "成员应该存在于数据库中");
    assert_eq!(found_member.unwrap().role, RoomRole::Owner, "成员角色应该是Owner");
}