use std::{env, sync::Arc};

use application::{
    BcryptPasswordHasher, LocalMessageBroadcaster, SystemClock,
    create_pg_pool,
    services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies},
    PgChatRoomRepository, PgMessageRepository, PgRoomMemberRepository, PgUserRepository,
    PresenceManager,
};
use axum::Router;
use web_api::{router as build_router_fn, AppState, JwtConfig, JwtService};

// 简单的内存在线状态管理器用于测试
#[derive(Default)]
pub struct TestPresenceManager;

#[async_trait::async_trait]
impl PresenceManager for TestPresenceManager {
    async fn user_connected(
        &self,
        _room_id: domain::RoomId,
        _user_id: domain::UserId,
    ) -> Result<(), application::ApplicationError> {
        Ok(())
    }

    async fn user_disconnected(
        &self,
        _room_id: domain::RoomId,
        _user_id: domain::UserId,
    ) -> Result<(), application::ApplicationError> {
        Ok(())
    }

    async fn get_online_users(
        &self,
        _room_id: domain::RoomId,
    ) -> Result<Vec<domain::UserId>, application::ApplicationError> {
        Ok(Vec::new())
    }

    async fn is_user_online(
        &self,
        _room_id: domain::RoomId,
        _user_id: domain::UserId,
    ) -> Result<bool, application::ApplicationError> {
        Ok(false)
    }

    async fn get_user_rooms(
        &self,
        _user_id: domain::UserId,
    ) -> Result<Vec<domain::RoomId>, application::ApplicationError> {
        Ok(Vec::new())
    }

    async fn cleanup_user_presence(
        &self,
        _user_id: domain::UserId,
    ) -> Result<(), application::ApplicationError> {
        Ok(())
    }
}

pub async fn build_router() -> Router {
    // 使用真实的 PostgreSQL 数据库进行测试
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string());

    let pg_pool = create_pg_pool(&database_url).await.expect("Failed to create database pool");

    // 运行迁移
    // sqlx::migrate!("../../migrations")
    //     .run(&pg_pool)
    //     .await
    //     .expect("Failed to run migrations");

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

    let presence_manager: Arc<dyn PresenceManager> = Arc::new(TestPresenceManager);

    let state = AppState::new(
        Arc::new(user_service),
        Arc::new(chat_service),
        broadcaster,
        jwt_service,
        presence_manager,
    );

    build_router_fn(state)
}