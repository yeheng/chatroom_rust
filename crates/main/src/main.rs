//! 主应用程序入口
//!
//! 启动 Axum Web API 服务。

use application::repository::{
    ChatRoomRepository, MessageRepository, RoomMemberRepository, UserRepository,
};
use application::{
    services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies},
    Clock, MessageBroadcaster, PasswordHasher, SystemClock,
};
use config::AppConfig;
use infrastructure::{
    create_pg_pool, BcryptPasswordHasher, LocalMessageBroadcaster, PgChatRoomRepository,
    PgMessageRepository, PgRoomMemberRepository, PgUserRepository, RedisMessageBroadcaster,
    StatsAggregationService,
};
use redis::Client as RedisClient;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use web_api::{router, AppState, JwtService};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 加载统一配置 - 使用分层配置加载
    let config = if cfg!(test) || std::env::var("CHATROOM_ENV").as_deref() == Ok("development") {
        // 测试和开发环境使用统一加载（有fallback）
        AppConfig::load().unwrap_or_else(|e| {
            tracing::warn!("配置加载失败，使用fallback: {}", e);
            AppConfig::default()
        })
    } else {
        // 生产环境要求严格的配置加载（设置APP_ENV=production）
        AppConfig::load().map_err(|e| anyhow::anyhow!("Configuration load failed: {}", e))?
    };

    // 验证配置（生产环境强制验证）
    if let Err(e) = config.validate() {
        tracing::error!("❌ 配置验证失败: {}", e);
        return Err(anyhow::anyhow!("Configuration validation failed: {}", e));
    }

    tracing::info!(
        "📦 连接数据库: {} (环境: {})",
        config.database.url.split('@').last().unwrap_or("unknown"),
        if config.database.url.contains("127.0.0.1") || config.database.url.contains("localhost") {
            "开发环境"
        } else {
            "生产环境"
        }
    );

    // 直接创建 PostgreSQL 连接池
    let pg_pool = create_pg_pool(&config.database.url, config.database.max_connections).await?;

    // 运行迁移
    sqlx::migrate!("../../migrations").run(&pg_pool).await?;

    // 创建具体的 repository 实例
    let user_repository: Arc<dyn UserRepository> = Arc::new(PgUserRepository::new(pg_pool.clone()));
    let room_repository: Arc<dyn ChatRoomRepository> =
        Arc::new(PgChatRoomRepository::new(pg_pool.clone()));
    let member_repository: Arc<dyn RoomMemberRepository> =
        Arc::new(PgRoomMemberRepository::new(pg_pool.clone()));
    let message_repository: Arc<dyn MessageRepository> =
        Arc::new(PgMessageRepository::new(pg_pool.clone()));

    // 创建其他服务
    let password_hasher: Arc<dyn PasswordHasher> = Arc::new(BcryptPasswordHasher::default());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::default());
    let broadcaster: Arc<dyn MessageBroadcaster> =
        if let Some(redis_url) = &config.broadcast.redis_url {
            let client = RedisClient::open(redis_url.clone())?;
            Arc::new(RedisMessageBroadcaster::new(client))
        } else {
            Arc::new(LocalMessageBroadcaster::new(config.broadcast.capacity))
        };

    // 创建统计相关服务
    let stats_service = Arc::new(StatsAggregationService::new(pg_pool.clone()));

    // 创建应用层服务
    let presence_manager: Arc<dyn application::PresenceManager> =
        if let Some(redis_url) = &config.broadcast.redis_url {
            let redis_client = Arc::new(RedisClient::open(redis_url.clone())?);
            Arc::new(application::RedisPresenceManager::from_app_config(redis_client, &config))
        } else {
            Arc::new(application::presence::memory::MemoryPresenceManager::new())
        };

    let user_service = UserService::new(UserServiceDependencies {
        user_repository,
        password_hasher: password_hasher.clone(),
        clock: clock.clone(),
        presence_manager: presence_manager.clone(),
    });

    let chat_service = ChatService::new(ChatServiceDependencies {
        room_repository,
        member_repository,
        message_repository,
        password_hasher,
        clock,
        broadcaster: broadcaster.clone(),
    });

    // 创建 JWT 服务
    let jwt_service = Arc::new(JwtService::new(config.jwt));

    // 创建应用状态
    let state = AppState::new(
        Arc::new(user_service),
        Arc::new(chat_service),
        broadcaster,
        jwt_service,
        presence_manager,
        stats_service,
    );

    // 启动 Web 服务器
    let app = router(state);
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", config.server.host, config.server.port))
            .await?;

    tracing::info!(
        "🚀 聊天室服务器启动在 http://{}:{} (配置模式: {})",
        config.server.host,
        config.server.port,
        if cfg!(test) || std::env::var("CHATROOM_ENV").as_deref() == Ok("development") {
            "开发环境"
        } else {
            "生产环境"
        }
    );
    axum::serve(listener, app).await?;

    Ok(())
}
