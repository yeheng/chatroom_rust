//! Database utilities and repositories (Core DB Layer)

use sqlx::{Pool, Postgres};
use std::sync::Arc;

pub type DbPool = Pool<Postgres>;

pub struct Db;

impl Db {
    pub async fn create_pool(database_url: &str, max_size: u32) -> Result<DbPool, sqlx::Error> {
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_size)
            .connect(database_url)
            .await
    }
}

pub mod repositories {
    use super::*;
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    #[derive(Debug, Clone)]
    pub struct Pagination {
        pub page: u32,
        pub page_size: u32,
    }
    impl Pagination {
        pub fn new(page: u32, page_size: u32) -> Self {
            Self { page, page_size }
        }
    }

    // Simple user model for DB layer
    #[derive(Debug, Clone, sqlx::FromRow)]
    pub struct DbUser {
        pub id: Uuid,
        pub username: String,
        pub email: String,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
    }

    pub struct UserRepository {
        pool: Arc<DbPool>,
    }
    impl UserRepository {
        pub fn new(pool: Arc<DbPool>) -> Self {
            Self { pool }
        }

        pub async fn create(&self, username: &str, email: &str) -> Result<DbUser, sqlx::Error> {
            sqlx::query_as::<_, DbUser>(
                r#"INSERT INTO users (username, email) VALUES ($1, $2)
                   RETURNING id, username, email, created_at, updated_at"#,
            )
            .bind(username)
            .bind(email)
            .fetch_one(&*self.pool)
            .await
        }

        pub async fn find_by_id(&self, id: Uuid) -> Result<DbUser, sqlx::Error> {
            sqlx::query_as::<_, DbUser>(
                r#"SELECT id, username, email, created_at, updated_at FROM users WHERE id = $1"#,
            )
            .bind(id)
            .fetch_one(&*self.pool)
            .await
        }

        pub async fn delete(&self, id: Uuid) -> Result<(), sqlx::Error> {
            sqlx::query("DELETE FROM users WHERE id = $1")
                .bind(id)
                .execute(&*self.pool)
                .await?;
            Ok(())
        }

        /// Example: wrap a query with retry using infrastructure::retry
        pub async fn find_by_id_with_retry(
            &self,
            id: Uuid,
            cfg: crate::retry::RetryConfig,
        ) -> Result<DbUser, sqlx::Error> {
            crate::retry::retry_async(cfg, || async { self.find_by_id(id).await }).await
        }
    }

    #[derive(Debug, Clone, sqlx::FromRow)]
    pub struct DbRoom {
        pub id: Uuid,
        pub name: String,
        pub description: Option<String>,
        pub owner_id: Uuid,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
        pub deleted_at: Option<DateTime<Utc>>,
    }

    pub struct ChatRoomRepository {
        pool: Arc<DbPool>,
    }
    impl ChatRoomRepository {
        pub fn new(pool: Arc<DbPool>) -> Self {
            Self { pool }
        }

        pub async fn create(
            &self,
            name: &str,
            description: Option<&str>,
            owner_id: Uuid,
        ) -> Result<DbRoom, sqlx::Error> {
            sqlx::query_as::<_, DbRoom>(
                r#"INSERT INTO chat_rooms (name, description, owner_id)
                   VALUES ($1, $2, $3)
                   RETURNING id, name, description, owner_id, created_at, updated_at, deleted_at"#,
            )
            .bind(name)
            .bind(description)
            .bind(owner_id)
            .fetch_one(&*self.pool)
            .await
        }

        pub async fn find_by_id(&self, id: Uuid) -> Result<DbRoom, sqlx::Error> {
            sqlx::query_as::<_, DbRoom>(
                r#"SELECT id, name, description, owner_id, created_at, updated_at, deleted_at
                   FROM chat_rooms WHERE id = $1"#,
            )
            .bind(id)
            .fetch_one(&*self.pool)
            .await
        }
    }

    #[derive(Debug, Clone, sqlx::FromRow)]
    pub struct DbMessage {
        pub id: Uuid,
        pub room_id: Uuid,
        pub user_id: Uuid,
        pub content: String,
        pub message_type: String,
        pub created_at: DateTime<Utc>,
    }

    pub struct MessageRepository {
        pool: Arc<DbPool>,
    }
    impl MessageRepository {
        pub fn new(pool: Arc<DbPool>) -> Self {
            Self { pool }
        }

        pub async fn create(
            &self,
            room_id: Uuid,
            user_id: Uuid,
            content: &str,
            message_type: &str,
        ) -> Result<DbMessage, sqlx::Error> {
            sqlx::query_as::<_, DbMessage>(
                r#"INSERT INTO messages (room_id, user_id, content, message_type)
                   VALUES ($1, $2, $3, $4)
                   RETURNING id, room_id, user_id, content, message_type, created_at"#,
            )
            .bind(room_id)
            .bind(user_id)
            .bind(content)
            .bind(message_type)
            .fetch_one(&*self.pool)
            .await
        }

        pub async fn find_by_room(
            &self,
            room_id: Uuid,
            pagination: Pagination,
        ) -> Result<(Vec<DbMessage>, bool), sqlx::Error> {
            let limit = pagination.page_size as i64;
            let offset = ((pagination.page - 1) * pagination.page_size) as i64;
            let rows = sqlx::query_as::<_, DbMessage>(
                r#"SELECT id, room_id, user_id, content, message_type, created_at
                   FROM messages WHERE room_id = $1
                   ORDER BY created_at DESC
                   LIMIT $2 OFFSET $3"#,
            )
            .bind(room_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&*self.pool)
            .await?;
            let has_more = rows.len() as u32 == pagination.page_size;
            Ok((rows, has_more))
        }
    }
}
