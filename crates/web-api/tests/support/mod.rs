use std::{env, sync::Arc};

use application::{
    create_pg_pool,
    presence::memory::MemoryPresenceManager,
    services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies},
    BcryptPasswordHasher, LocalMessageBroadcaster, PasswordHasher, PgChatRoomRepository,
    PgMessageRepository, PgRoomMemberRepository, PgUserRepository, SystemClock,
};
use axum::Router;
use config::AppConfig;
use sqlx::PgPool;
use web_api::{router as build_router_fn, AppState, JwtService};

/// 测试专用的在线状态管理器类型
pub type TestPresenceManager = MemoryPresenceManager;

/// 测试配置
pub struct TestConfig {
    pub app_config: AppConfig,
    pub database_url: String,
}

impl Default for TestConfig {
    fn default() -> Self {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:123456@127.0.0.1:5432/chatroom".to_string());

        let mut app_config = AppConfig::from_env();
        // 覆盖测试专用配置
        app_config.database.url = database_url.clone();
        app_config.jwt.secret = "test-secret-key-that-is-at-least-32-bytes-long".to_string();
        app_config.jwt.expiration_hours = 24;

        Self {
            app_config,
            database_url,
        }
    }
}

/// 测试应用状态，包含所有需要的组件
pub struct TestAppState {
    pub router: Router,
    pub _pool: PgPool,
    pub _config: TestConfig,
}

/// 清理数据库中的所有数据，为测试提供干净的环境
async fn cleanup_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // 按照外键依赖关系的反序删除数据
    sqlx::query("DELETE FROM room_members")
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM messages").execute(pool).await?;
    sqlx::query("DELETE FROM chat_rooms").execute(pool).await?;
    sqlx::query("DELETE FROM users").execute(pool).await?;
    Ok(())
}

/// 创建测试用的数据库连接池
async fn create_test_pool(database_url: &str) -> PgPool {
    create_pg_pool(database_url)
        .await
        .expect("Failed to create database pool for testing")
}

/// 创建所有需要的服务
fn create_services(
    pool: &PgPool,
    config: &AppConfig,
) -> (
    Arc<UserService>,
    Arc<ChatService>,
    Arc<LocalMessageBroadcaster>,
) {
    // 创建 repositories
    let user_repository = PgUserRepository::new(pool.clone());
    let room_repository = PgChatRoomRepository::new(pool.clone());
    let member_repository = PgRoomMemberRepository::new(pool.clone());
    let message_repository = PgMessageRepository::new(pool.clone());

    // 创建核心服务
    let password_hasher: Arc<dyn application::PasswordHasher> =
        Arc::new(BcryptPasswordHasher::new(config.server.bcrypt_cost));
    let clock: Arc<dyn application::Clock> = Arc::new(SystemClock::default());
    let broadcaster = Arc::new(LocalMessageBroadcaster::new());

    // 创建应用层服务
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

    (Arc::new(user_service), Arc::new(chat_service), broadcaster)
}

/// 构建测试用的应用状态
pub async fn setup_test_app() -> TestAppState {
    let config = TestConfig::default();

    // 创建数据库连接池
    let pool = create_test_pool(&config.database_url).await;

    // 清理数据库
    cleanup_database(&pool)
        .await
        .expect("Failed to cleanup database for testing");

    // 运行数据库迁移
    let _ = sqlx::migrate!("../../migrations").run(&pool).await;

    // 创建所有服务
    let (user_service, chat_service, broadcaster) = create_services(&pool, &config.app_config);

    // 创建 JWT 服务
    let jwt_service = Arc::new(JwtService::new(config.app_config.jwt.clone()));

    // 创建在线状态管理器
    let presence_manager: Arc<dyn application::PresenceManager> =
        Arc::new(TestPresenceManager::default());

    // 创建应用状态
    let app_state = AppState::new(
        user_service,
        chat_service,
        broadcaster.clone(),
        jwt_service,
        presence_manager,
    );

    // 构建路由器
    let router = build_router_fn(app_state);

    TestAppState {
        router,
        _pool: pool,
        _config: config,
    }
}

/// 便捷函数：直接获取路由器（为了向后兼容）
pub async fn build_router() -> Router {
    setup_test_app().await.router
}

/// 测试助手函数：创建测试用户
pub async fn _create_test_user(pool: &PgPool, username: &str, email: &str) -> uuid::Uuid {
    let user_id = uuid::Uuid::new_v4();
    let password_hash = BcryptPasswordHasher::default()
        .hash("password123")
        .await
        .expect("Failed to hash password");

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, created_at, updated_at)
         VALUES ($1, $2, $3, $4, NOW(), NOW())",
    )
    .bind(user_id)
    .bind(username)
    .bind(email)
    .bind(password_hash.as_str())
    .execute(pool)
    .await
    .expect("Failed to create test user");

    user_id
}

/// 测试助手函数：创建测试聊天室
pub async fn _create_test_room(pool: &PgPool, name: &str, owner_id: uuid::Uuid) -> uuid::Uuid {
    let room_id = uuid::Uuid::new_v4();

    sqlx::query(
        "INSERT INTO chat_rooms (id, name, owner_id, visibility, created_at, updated_at)
         VALUES ($1, $2, $3, 'public', NOW(), NOW())",
    )
    .bind(room_id)
    .bind(name)
    .bind(owner_id)
    .execute(pool)
    .await
    .expect("Failed to create test room");

    room_id
}

/// 测试助手函数：添加用户到聊天室
pub async fn _add_user_to_room(pool: &PgPool, room_id: uuid::Uuid, user_id: uuid::Uuid) {
    sqlx::query(
        "INSERT INTO room_members (room_id, user_id, joined_at)
         VALUES ($1, $2, NOW())",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("Failed to add user to room");
}
