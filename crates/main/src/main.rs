//! 主应用程序入口
//!
//! 启动 Axum Web API 服务。

use application::{
    create_pg_pool,
    services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies},
    BcryptPasswordHasher, LocalMessageBroadcaster, PgChatRoomRepository, PgMessageRepository,
    PgRoomMemberRepository, PgUserRepository, SystemClock,
};
use config::AppConfig;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use web_api::{router, AppState, JwtService};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 加载统一配置
    let config = AppConfig::from_env();

    // 验证配置
    config
        .validate()
        .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;

    tracing::info!(
        "连接数据库: {}",
        config.database.url.split('@').last().unwrap_or("unknown")
    );

    // 直接创建 PostgreSQL 连接池
    let pg_pool = create_pg_pool(&config.database.url).await?;

    // 运行迁移
    sqlx::migrate!("../../migrations").run(&pg_pool).await?;

    // 创建具体的 repository 实例 - 不需要 Arc 包装
    let user_repository = PgUserRepository::new(pg_pool.clone());
    let room_repository = PgChatRoomRepository::new(pg_pool.clone());
    let member_repository = PgRoomMemberRepository::new(pg_pool.clone());
    let message_repository = PgMessageRepository::new(pg_pool);

    // 创建其他服务
    let password_hasher: Arc<dyn application::PasswordHasher> =
        Arc::new(BcryptPasswordHasher::default());
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

    // 创建 JWT 服务
    let jwt_service = Arc::new(JwtService::new(config.jwt));

    // 创建简单的内存在线状态管理器 - 生产环境可以用 Redis
    let presence_manager = Arc::new(application::presence::memory::MemoryPresenceManager::new());

    // 创建应用状态
    let state = AppState::new(
        Arc::new(user_service),
        Arc::new(chat_service),
        broadcaster,
        jwt_service,
        presence_manager,
    );

    // 启动 Web 服务器
    let app = router(state);
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", config.server.host, config.server.port))
            .await?;

    tracing::info!(
        "聊天室服务器启动在 http://{}:{}",
        config.server.host,
        config.server.port
    );
    axum::serve(listener, app).await?;

    Ok(())
}
