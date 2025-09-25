//! 主应用程序入口
//!
//! 启动 Axum Web API 服务。

use application::clock::SystemClock;
use application::services::{
    ChatService, ChatServiceDependencies, UserService, UserServiceDependencies,
};
use infrastructure::{Infrastructure, InfrastructureConfig};
use std::env;
use tracing_subscriber::EnvFilter;
use web_api::{router, AppState, JwtConfig, JwtService};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 读取环境变量配置
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/chatroom".to_string());

    let redis_url = env::var("REDIS_URL").ok(); // 可选的 Redis URL

    let config = InfrastructureConfig {
        database_url,
        redis_url,
        ..InfrastructureConfig::default()
    };

    // 连接基础设施
    tracing::info!("正在连接基础设施...");
    let infrastructure = Infrastructure::connect(config).await?;

    // 根据广播器类型记录信息
    match &infrastructure.broadcaster {
        infrastructure::BroadcasterType::Local(_) => {
            tracing::info!("使用本地内存广播器");
        }
        infrastructure::BroadcasterType::Redis(_) => {
            tracing::info!("使用 Redis Pub/Sub 广播器");
        }
    }

    // 创建 JWT 服务
    let jwt_config = JwtConfig {
        secret: env::var("JWT_SECRET").unwrap_or_else(|_| {
            "your-256-bit-secret-key-here-please-change-in-production".to_string()
        }),
        expiration_hours: 24,
    };
    let jwt_service = std::sync::Arc::new(JwtService::new(jwt_config));

    // 创建应用层服务
    let clock = std::sync::Arc::new(SystemClock::default());

    let user_service = UserService::new(UserServiceDependencies {
        user_repository: infrastructure.storage.user_repository.clone(),
        password_hasher: infrastructure.password_hasher_trait(),
        clock: clock.clone(),
    });

    let chat_service = ChatService::new(ChatServiceDependencies {
        room_repository: infrastructure.storage.room_repository.clone(),
        member_repository: infrastructure.storage.member_repository.clone(),
        message_repository: infrastructure.storage.message_repository.clone(),
        password_hasher: infrastructure.password_hasher_trait(),
        clock,
        broadcaster: std::sync::Arc::new(infrastructure.broadcaster.clone())
            as std::sync::Arc<dyn application::MessageBroadcaster>,
    });

    // 创建 PresenceManager（暂时使用内存版本，生产环境应该使用Redis版本）
    let presence_manager =
        std::sync::Arc::new(application::presence::memory::MemoryPresenceManager::new())
            as std::sync::Arc<dyn application::PresenceManager>;

    // 创建应用状态
    let state = AppState::new(
        std::sync::Arc::new(user_service),
        std::sync::Arc::new(chat_service),
        infrastructure.broadcaster,
        jwt_service,      // 传递 JWT 服务
        presence_manager, // 传递在线状态管理器
    );

    // 启动 Web 服务器
    let app = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;

    tracing::info!("聊天室服务器启动在 http://127.0.0.1:8080");
    axum::serve(listener, app).await?;

    Ok(())
}
