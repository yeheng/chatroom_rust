//! 用户Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    entities::user::{User, UserStatus}, errors::{DomainError, DomainResult}, repositories::{
        PaginatedResult, Pagination, SortConfig
    }, UserRepository, UserSearchParams, UserStatistics
};
use sqlx::{query, query_as, FromRow, Row};
use std::sync::Arc;
use uuid::Uuid;

/// 数据库用户模型
#[derive(Debug, Clone, FromRow)]
struct DbUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: Option<String>,
    pub avatar_url: Option<String>,
    pub status: String,
    pub last_active_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DbUser> for User {
    fn from(db_user: DbUser) -> Self {
        let status = match db_user.status.as_str() {
            "active" => UserStatus::Active,
            "inactive" => UserStatus::Inactive,
            "suspended" => UserStatus::Suspended,
            "deleted" => UserStatus::Deleted,
            _ => UserStatus::Active,
        };

        User {
            id: db_user.id,
            username: db_user.username,
            email: db_user.email,
            password_hash: db_user.password_hash,
            avatar_url: db_user.avatar_url,
            status,
            last_active_at: db_user.last_active_at,
            created_at: db_user.created_at,
            updated_at: db_user.updated_at,
            display_name: None, // 数据库中暂未存储，使用默认值
        }
    }
}

impl From<&User> for DbUser {
    fn from(user: &User) -> Self {
        DbUser {
            id: user.id,
            username: user.username.clone(),
            email: user.email.clone(),
            password_hash: user.password_hash.clone(),
            avatar_url: user.avatar_url.clone(),
            status: user.status.to_string(),
            last_active_at: user.last_active_at,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

/// 用户Repository实现
pub struct PostgresUserRepository {
    pool: Arc<DbPool>,
}

impl PostgresUserRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    /// 构建查询条件
    fn build_search_query(params: &UserSearchParams) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut values = Vec::new();
        let mut param_count = 1;

        if let Some(username) = &params.username {
            conditions.push(format!("username ILIKE ${}", param_count));
            values.push(format!("%{}%", username));
            param_count += 1;
        }

        if let Some(email) = &params.email {
            conditions.push(format!("email ILIKE ${}", param_count));
            values.push(format!("%{}%", email));
            param_count += 1;
        }

        if let Some(status) = &params.status {
            conditions.push(format!("status = ${}", param_count));
            values.push(status.to_string());
            param_count += 1;
        }

        if let Some(created_after) = &params.created_after {
            conditions.push(format!("created_at > ${}", param_count));
            values.push(created_after.to_string());
            param_count += 1;
        }

        if let Some(created_before) = &params.created_before {
            conditions.push(format!("created_at < ${}", param_count));
            values.push(created_before.to_string());
            param_count += 1;
        }

        if let Some(last_active_after) = &params.last_active_after {
            conditions.push(format!("last_active_at > ${}", param_count));
            values.push(last_active_after.to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, values)
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn create(&self, user: &User) -> DomainResult<User> {
        let db_user = DbUser::from(user);

        let result = query_as::<_, DbUser>(
            r#"
            INSERT INTO users (id, username, email, password_hash, avatar_url, status, last_active_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            "#,
        )
        .bind(db_user.id)
        .bind(&db_user.username)
        .bind(&db_user.email)
        .bind(&db_user.password_hash)
        .bind(&db_user.avatar_url)
        .bind(&db_user.status)
        .bind(db_user.last_active_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<User>> {
        let result = query_as::<_, DbUser>(
            r#"
            SELECT id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            FROM users
            WHERE id = $1 AND status != 'deleted'
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|u| u.into()))
    }

    async fn find_by_username(&self, username: &str) -> DomainResult<Option<User>> {
        let result = query_as::<_, DbUser>(
            r#"
            SELECT id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            FROM users
            WHERE username = $1 AND status != 'deleted'
            "#,
        )
        .bind(username)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|u| u.into()))
    }

    async fn find_by_email(&self, email: &str) -> DomainResult<Option<User>> {
        let result = query_as::<_, DbUser>(
            r#"
            SELECT id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            FROM users
            WHERE email = $1 AND status != 'deleted'
            "#,
        )
        .bind(email)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|u| u.into()))
    }

    async fn update(&self, user: &User) -> DomainResult<User> {
        let db_user = DbUser::from(user);

        let result = query_as::<_, DbUser>(
            r#"
            UPDATE users
            SET username = $2, email = $3, password_hash = $4, avatar_url = $5,
                status = $6, last_active_at = $7, updated_at = NOW()
            WHERE id = $1
            RETURNING id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            "#,
        )
        .bind(db_user.id)
        .bind(&db_user.username)
        .bind(&db_user.email)
        .bind(&db_user.password_hash)
        .bind(&db_user.avatar_url)
        .bind(&db_user.status)
        .bind(db_user.last_active_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_password_hash(&self, user_id: Uuid, password_hash: &str) -> DomainResult<()> {
        query("UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1")
            .bind(user_id)
            .bind(password_hash)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn update_status(&self, user_id: Uuid, status: UserStatus) -> DomainResult<()> {
        query("UPDATE users SET status = $2, updated_at = NOW() WHERE id = $1")
            .bind(user_id)
            .bind(status.to_string())
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn update_last_active(&self, user_id: Uuid, last_active_at: DateTime<Utc>) -> DomainResult<()> {
        query("UPDATE users SET last_active_at = $2, updated_at = NOW() WHERE id = $1")
            .bind(user_id)
            .bind(last_active_at)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn soft_delete(&self, user_id: Uuid) -> DomainResult<()> {
        query("UPDATE users SET status = 'deleted', updated_at = NOW() WHERE id = $1")
            .bind(user_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn search(
        &self,
        params: &UserSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<User>> {
        let (where_clause, values) = Self::build_search_query(params);

        let order_clause = match sort {
            Some(sort_config) => {
                let direction = if sort_config.ascending { "ASC" } else { "DESC" };
                format!("ORDER BY {} {}", sort_config.field, direction)
            }
            None => "ORDER BY created_at DESC".to_string(),
        };

        let base_query = format!(
            "FROM users {} AND status != 'deleted'",
            where_clause
        );

        // 获取总数
        let count_query = format!("SELECT COUNT(*) {}", base_query);
        let total_count: i64 = query(&count_query)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取数据
        let data_query = format!(
            r#"
            SELECT id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            {} {} LIMIT {} OFFSET {}
            "#,
            base_query, order_clause, pagination.limit, pagination.offset
        );

        let users: Vec<DbUser> = query_as(&data_query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        let users: Vec<User> = users.into_iter().map(|u| u.into()).collect();

        Ok(PaginatedResult::new(users, total_count as u64, pagination))
    }

    async fn get_statistics(&self) -> DomainResult<UserStatistics> {
        let row = query(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE status != 'deleted') as total_users,
                COUNT(*) FILTER (WHERE status = 'active') as active_users,
                COUNT(*) FILTER (WHERE status = 'inactive') as inactive_users,
                COUNT(*) FILTER (WHERE status = 'suspended') as suspended_users,
                COUNT(*) FILTER (WHERE created_at::date = CURRENT_DATE) as users_registered_today,
                COUNT(*) FILTER (WHERE last_active_at > NOW() - INTERVAL '15 minutes') as users_online
            FROM users
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(UserStatistics {
            total_users: row.get::<i64, _>("total_users") as u64,
            active_users: row.get::<i64, _>("active_users") as u64,
            inactive_users: row.get::<i64, _>("inactive_users") as u64,
            suspended_users: row.get::<i64, _>("suspended_users") as u64,
            users_registered_today: row.get::<i64, _>("users_registered_today") as u64,
            users_online: row.get::<i64, _>("users_online") as u64,
        })
    }

    async fn username_exists(&self, username: &str) -> DomainResult<bool> {
        let count: i64 = query("SELECT COUNT(*) FROM users WHERE username = $1 AND status != 'deleted'")
            .bind(username)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        Ok(count > 0)
    }

    async fn email_exists(&self, email: &str) -> DomainResult<bool> {
        let count: i64 = query("SELECT COUNT(*) FROM users WHERE email = $1 AND status != 'deleted'")
            .bind(email)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        Ok(count > 0)
    }

    async fn find_by_ids(&self, ids: &[Uuid]) -> DomainResult<Vec<User>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let users: Vec<DbUser> = query_as(
            r#"
            SELECT id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            FROM users
            WHERE id = ANY($1) AND status != 'deleted'
            ORDER BY username
            "#,
        )
        .bind(ids)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(users.into_iter().map(|u| u.into()).collect())
    }

    async fn find_recent_users(&self, limit: u32) -> DomainResult<Vec<User>> {
        let users: Vec<DbUser> = query_as(
            r#"
            SELECT id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            FROM users
            WHERE status != 'deleted'
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(users.into_iter().map(|u| u.into()).collect())
    }

    async fn find_online_users(&self, pagination: Pagination) -> DomainResult<PaginatedResult<User>> {
        // 获取总数
        let total_count: i64 = query(
            "SELECT COUNT(*) FROM users WHERE last_active_at > NOW() - INTERVAL '15 minutes' AND status = 'active'"
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        // 获取在线用户
        let users: Vec<DbUser> = query_as(
            r#"
            SELECT id, username, email, password_hash, avatar_url, status, last_active_at, created_at, updated_at
            FROM users
            WHERE last_active_at > NOW() - INTERVAL '15 minutes' AND status = 'active'
            ORDER BY last_active_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let users: Vec<User> = users.into_iter().map(|u| u.into()).collect();

        Ok(PaginatedResult::new(users, total_count as u64, pagination))
    }

    #[cfg(feature = "enterprise")]
    async fn find_by_role(&self, role_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<User>> {
        // 这里需要与用户角色表关联查询
        // 由于暂时没有实现企业功能，返回空结果
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }

    #[cfg(feature = "enterprise")]
    async fn find_by_organization(&self, org_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<User>> {
        // 这里需要与组织表关联查询
        // 由于暂时没有实现企业功能，返回空结果
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }
}