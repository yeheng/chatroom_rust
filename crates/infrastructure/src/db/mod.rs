//! 数据库基础设施模块
//!
//! 提供数据库连接池和Repository实现

use sqlx::{Pool, Postgres};

pub type DbPool = Pool<Postgres>;

/// 数据库连接管理
pub struct Db;

impl Db {
    /// 创建数据库连接池
    pub async fn create_pool(database_url: &str, max_size: u32) -> Result<DbPool, sqlx::Error> {
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_size)
            .connect(database_url)
            .await
    }
}

// Repository实现模块
pub mod repositories;

// 重新导出Repository实现
pub use repositories::*;