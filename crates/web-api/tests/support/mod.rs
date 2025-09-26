use std::{env, sync::Arc};

use application::{
    BcryptPasswordHasher, LocalMessageBroadcaster, SystemClock,
    create_pg_pool,
    services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies},
    PgChatRoomRepository, PgMessageRepository, PgRoomMemberRepository, PgUserRepository,
    presence::memory::MemoryPresenceManager,
};
use sqlx::PgPool;
use axum::Router;

// 清理数据库中的所有数据，为测试提供干净的环境
async fn cleanup_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // 按照外键依赖关系的反序删除数据
    sqlx::query("DELETE FROM room_members").execute(pool).await?;
    sqlx::query("DELETE FROM messages").execute(pool).await?;
    sqlx::query("DELETE FROM chat_rooms").execute(pool).await?;
    sqlx::query("DELETE FROM users").execute(pool).await?;
    Ok(())
}

use web_api::{router as build_router_fn, AppState, JwtConfig, JwtService};

// 使用内存实现的在线状态管理器进行测试
pub type TestPresenceManager = MemoryPresenceManager;

pub async fn build_router() -> Router {
    // 使用真实的 PostgreSQL 数据库进行测试
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string());

    let pg_pool = create_pg_pool(&database_url).await.expect("Failed to create database pool");

    // 清理数据库表中的所有数据
    cleanup_database(&pg_pool).await.expect("Failed to cleanup database");

    // 运行迁移确保表结构正确
    // 注意：如果类型已存在，这个会失败，但我们可以忽略
    let _ = sqlx::migrate!("../../migrations").run(&pg_pool).await;

    // 创建 repositories
    let user_repository = PgUserRepository::new(pg_pool.clone());
    let room_repository = PgChatRoomRepository::new(pg_pool.clone());
    let member_repository = PgRoomMemberRepository::new(pg_pool.clone());
    let message_repository = PgMessageRepository::new(pg_pool);

    // 创建服务
    let password_hasher: Arc<dyn application::PasswordHasher> = Arc::new(BcryptPasswordHasher::default());
    let clock: Arc<dyn application::Clock> = Arc::new(SystemClock::default());
    let broadcaster = Arc::new(LocalMessageBroadcaster::new());

    let user_service = UserService::new(UserServiceDependencies {
        user_repository,
        password_hasher: password_hasher.clone(),
        clock: clock.clone(),
    });

    let chat_service = ChatService::new(ChatServiceDependencies {
        room_repository,
        member_repository,
        message_repository,
        password_hasher,
        clock,
        broadcaster: broadcaster.clone() as Arc<dyn application::MessageBroadcaster>,
    });

    let jwt_service = Arc::new(JwtService::new(JwtConfig {
        secret: "test-secret-key".to_string(),
        expiration_hours: 24,
    }));

    let presence_manager: Arc<dyn application::PresenceManager> = Arc::new(TestPresenceManager::default());

    let state = AppState::new(
        Arc::new(user_service),
        Arc::new(chat_service),
        broadcaster,
        jwt_service,
        presence_manager,
    );

    build_router_fn(state)
}