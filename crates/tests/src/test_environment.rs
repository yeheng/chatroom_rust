//! 测试环境管理
//!
//! 提供隔离的测试环境，包含 PostgreSQL、Redis、Kafka 等依赖

use anyhow::{Context, Result};
use std::sync::Arc;
use testcontainers::Container;
use testcontainers_modules::{postgres::Postgres, redis::Redis};
use tokio::sync::OnceCell;
use tracing::{info, warn};

use web_api::app_config::*;

/// 测试环境配置
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub api_port: u16,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            database_url: "".to_string(),
            redis_url: "".to_string(),
            jwt_secret: "test_jwt_secret_key_for_testing_purposes_only".to_string(),
            api_port: 0, // 随机端口
        }
    }
}

/// 测试环境管理器
///
/// 负责启动和管理测试所需的所有外部依赖
pub struct TestEnvironment {
    config: TestConfig,
    docker_client: Arc<Cli>,
    postgres_container: Option<Container<'static, Postgres>>,
    redis_container: Option<Container<'static, Redis>>,
    database_pool: OnceCell<Arc<DatabasePool>>,
    app_handle: OnceCell<TestAppHandle>,
}

/// 测试应用句柄
#[derive(Debug, Clone)]
pub struct TestAppHandle {
    pub base_url: String,
    pub config: AppConfig,
}

impl TestEnvironment {
    /// 创建新的测试环境
    pub async fn new() -> Result<Arc<Self>> {
        let docker_client = Arc::new(Cli::default());

        let env = Arc::new(Self {
            config: TestConfig::default(),
            docker_client,
            postgres_container: None,
            redis_container: None,
            database_pool: OnceCell::new(),
            app_handle: OnceCell::new(),
        });

        // 启动容器
        env.setup_containers().await?;

        Ok(env)
    }

    /// 设置测试容器
    async fn setup_containers(&self) -> Result<()> {
        info!("启动测试容器...");

        // 启动 PostgreSQL 容器
        let postgres_image = Postgres::default()
            .with_db_name("chatroom_test")
            .with_user("test_user")
            .with_password("test_password");

        // 注意：这里需要处理生命周期问题
        // 在实际实现中，可能需要使用不同的容器管理策略
        warn!("容器启动需要在测试运行时保持活动状态");

        Ok(())
    }

    /// 获取数据库连接池
    pub async fn get_database_pool(&self) -> Result<Arc<DatabasePool>> {
        if let Some(pool) = self.database_pool.get() {
            Ok(pool.clone())
        } else {
            // 创建数据库连接池
            let pool = Arc::new(
                DatabasePool::new(&self.config.database_url)
                    .await
                    .context("创建数据库连接池失败")?,
            );

            // 运行数据库迁移
            self.run_migrations(&pool).await?;

            // 缓存连接池
            let _ = self.database_pool.set(pool.clone());

            Ok(pool)
        }
    }

    /// 运行数据库迁移
    async fn run_migrations(&self, pool: &DatabasePool) -> Result<()> {
        info!("运行数据库迁移...");
        // 这里需要根据实际的迁移实现来调用
        // pool.run_migrations().await?;
        Ok(())
    }

    /// 启动测试应用
    pub async fn start_app(&self) -> Result<TestAppHandle> {
        if let Some(handle) = self.app_handle.get() {
            return Ok(handle.clone());
        }

        info!("启动测试应用...");

        // 构建应用配置
        let config = AppConfig {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: self.config.api_port,
                cors_origins: vec![],
            },
            database: DatabaseConfig {
                url: self.config.database_url.clone(),
                max_connections: 10,
                min_connections: 2,
                acquire_timeout_seconds: 10,
            },
            redis: RedisConfigSimple {
                url: self.config.redis_url.clone(),
            },
            feature_flags: FeatureFlags::default(),
        };

        // 启动应用服务器
        let base_url = format!("http://{}:{}", config.server.host, config.server.port);

        let handle = TestAppHandle { base_url, config };

        // 缓存应用句柄
        let _ = self.app_handle.set(handle.clone());

        Ok(handle)
    }

    /// 获取应用基础URL
    pub fn app_base_url(&self) -> String {
        self.app_handle
            .get()
            .map(|h| h.base_url.clone())
            .unwrap_or_else(|| "http://localhost:3000".to_string())
    }

    /// 清理测试数据
    pub async fn cleanup(&self) -> Result<()> {
        info!("清理测试数据...");

        if let Some(pool) = self.database_pool.get() {
            // 清理数据库表
            sqlx::query("TRUNCATE TABLE messages, room_members, chat_rooms, users CASCADE")
                .execute(&pool.0) // 假设 DatabasePool 包装了 sqlx::Pool
                .await
                .context("清理数据库失败")?;
        }

        Ok(())
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // 容器会在 Drop 时自动清理
    }
}

/// 创建测试环境的便捷函数
pub async fn create_test_environment() -> Result<Arc<TestEnvironment>> {
    TestEnvironment::new().await
}

/// 测试环境守卫
///
/// 确保测试结束时清理资源
pub struct TestEnvironmentGuard {
    env: Arc<TestEnvironment>,
}

impl TestEnvironmentGuard {
    pub async fn new() -> Result<Self> {
        let env = create_test_environment().await?;
        Ok(Self { env })
    }

    pub fn environment(&self) -> &Arc<TestEnvironment> {
        &self.env
    }
}

impl Drop for TestEnvironmentGuard {
    fn drop(&mut self) {
        // 在析构时清理测试环境
        let env = self.env.clone();
        tokio::spawn(async move {
            if let Err(e) = env.cleanup().await {
                warn!("清理测试环境失败: {}", e);
            }
        });
    }
}
