//! 会话Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    errors::{DomainError, DomainResult},
    repositories::{SessionRepository, Pagination, PaginatedResult},
};
use sqlx::{query, query_as, FromRow};
use std::sync::Arc;
use uuid::Uuid;

/// 会话实体
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub refresh_token: Option<String>,
    pub device_type: Option<String>,
    pub device_info: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub is_active: bool,
    pub expires_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 会话统计
#[derive(Debug, Clone)]
pub struct SessionStatistics {
    pub total_sessions: u64,
    pub active_sessions: u64,
    pub sessions_today: u64,
    pub unique_users_today: u64,
    pub avg_session_duration_minutes: f64,
}

/// 数据库会话模型
#[derive(Debug, Clone, FromRow)]
struct DbSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub refresh_token: Option<String>,
    pub device_type: Option<String>,
    pub device_info: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub is_active: bool,
    pub expires_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DbSession> for Session {
    fn from(db_session: DbSession) -> Self {
        Session {
            id: db_session.id,
            user_id: db_session.user_id,
            token_hash: db_session.token_hash,
            refresh_token: db_session.refresh_token,
            device_type: db_session.device_type,
            device_info: db_session.device_info,
            ip_address: db_session.ip_address,
            user_agent: db_session.user_agent,
            is_active: db_session.is_active,
            expires_at: db_session.expires_at,
            last_accessed_at: db_session.last_accessed_at,
            created_at: db_session.created_at,
            updated_at: db_session.updated_at,
        }
    }
}

impl From<&Session> for DbSession {
    fn from(session: &Session) -> Self {
        DbSession {
            id: session.id,
            user_id: session.user_id,
            token_hash: session.token_hash.clone(),
            refresh_token: session.refresh_token.clone(),
            device_type: session.device_type.clone(),
            device_info: session.device_info.clone(),
            ip_address: session.ip_address.clone(),
            user_agent: session.user_agent.clone(),
            is_active: session.is_active,
            expires_at: session.expires_at,
            last_accessed_at: session.last_accessed_at,
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }
}

/// 会话Repository实现
pub struct PostgresSessionRepository {
    pool: Arc<DbPool>,
}

impl PostgresSessionRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn create(&self, session: &Session) -> DomainResult<Session> {
        let db_session = DbSession::from(session);

        let result = query_as::<_, DbSession>(
            r#"
            INSERT INTO sessions (
                id, user_id, token_hash, refresh_token, device_type, device_info,
                ip_address, user_agent, is_active, expires_at, last_accessed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, user_id, token_hash, refresh_token, device_type, device_info,
                     ip_address, user_agent, is_active, expires_at, last_accessed_at,
                     created_at, updated_at
            "#,
        )
        .bind(db_session.id)
        .bind(db_session.user_id)
        .bind(&db_session.token_hash)
        .bind(&db_session.refresh_token)
        .bind(&db_session.device_type)
        .bind(&db_session.device_info)
        .bind(&db_session.ip_address)
        .bind(&db_session.user_agent)
        .bind(db_session.is_active)
        .bind(db_session.expires_at)
        .bind(db_session.last_accessed_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.into())
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<Session>> {
        let result = query_as::<_, DbSession>(
            r#"
            SELECT id, user_id, token_hash, refresh_token, device_type, device_info,
                   ip_address, user_agent, is_active, expires_at, last_accessed_at,
                   created_at, updated_at
            FROM sessions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn find_by_token_hash(&self, token_hash: &str) -> DomainResult<Option<Session>> {
        let result = query_as::<_, DbSession>(
            r#"
            SELECT id, user_id, token_hash, refresh_token, device_type, device_info,
                   ip_address, user_agent, is_active, expires_at, last_accessed_at,
                   created_at, updated_at
            FROM sessions
            WHERE token_hash = $1 AND is_active = true AND expires_at > NOW()
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn find_by_refresh_token(&self, refresh_token: &str) -> DomainResult<Option<Session>> {
        let result = query_as::<_, DbSession>(
            r#"
            SELECT id, user_id, token_hash, refresh_token, device_type, device_info,
                   ip_address, user_agent, is_active, expires_at, last_accessed_at,
                   created_at, updated_at
            FROM sessions
            WHERE refresh_token = $1 AND is_active = true
            "#,
        )
        .bind(refresh_token)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn update(&self, session: &Session) -> DomainResult<Session> {
        let db_session = DbSession::from(session);

        let result = query_as::<_, DbSession>(
            r#"
            UPDATE sessions
            SET token_hash = $2, refresh_token = $3, device_type = $4, device_info = $5,
                ip_address = $6, user_agent = $7, is_active = $8, expires_at = $9,
                last_accessed_at = $10, updated_at = NOW()
            WHERE id = $1
            RETURNING id, user_id, token_hash, refresh_token, device_type, device_info,
                     ip_address, user_agent, is_active, expires_at, last_accessed_at,
                     created_at, updated_at
            "#,
        )
        .bind(db_session.id)
        .bind(&db_session.token_hash)
        .bind(&db_session.refresh_token)
        .bind(&db_session.device_type)
        .bind(&db_session.device_info)
        .bind(&db_session.ip_address)
        .bind(&db_session.user_agent)
        .bind(db_session.is_active)
        .bind(db_session.expires_at)
        .bind(db_session.last_accessed_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_last_accessed(&self, session_id: Uuid, last_accessed_at: DateTime<Utc>) -> DomainResult<()> {
        query("UPDATE sessions SET last_accessed_at = $2, updated_at = NOW() WHERE id = $1")
            .bind(session_id)
            .bind(last_accessed_at)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn refresh_token(&self, session_id: Uuid, new_token_hash: &str, new_refresh_token: Option<&str>) -> DomainResult<()> {
        query("UPDATE sessions SET token_hash = $2, refresh_token = $3, updated_at = NOW() WHERE id = $1")
            .bind(session_id)
            .bind(new_token_hash)
            .bind(new_refresh_token)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn invalidate(&self, session_id: Uuid) -> DomainResult<()> {
        query("UPDATE sessions SET is_active = false, updated_at = NOW() WHERE id = $1")
            .bind(session_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, session_id: Uuid) -> DomainResult<bool> {
        let result = query("DELETE FROM sessions WHERE id = $1")
            .bind(session_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn find_active_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<Session>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM sessions WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        // 获取会话
        let sessions: Vec<DbSession> = query_as(
            r#"
            SELECT id, user_id, token_hash, refresh_token, device_type, device_info,
                   ip_address, user_agent, is_active, expires_at, last_accessed_at,
                   created_at, updated_at
            FROM sessions
            WHERE user_id = $1 AND is_active = true
            ORDER BY last_accessed_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let sessions: Vec<Session> = sessions.into_iter().map(|s| s.into()).collect();
        Ok(PaginatedResult::new(sessions, total_count as u64, pagination))
    }

    async fn find_all_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<Session>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        // 获取会话
        let sessions: Vec<DbSession> = query_as(
            r#"
            SELECT id, user_id, token_hash, refresh_token, device_type, device_info,
                   ip_address, user_agent, is_active, expires_at, last_accessed_at,
                   created_at, updated_at
            FROM sessions
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let sessions: Vec<Session> = sessions.into_iter().map(|s| s.into()).collect();
        Ok(PaginatedResult::new(sessions, total_count as u64, pagination))
    }

    async fn cleanup_expired(&self) -> DomainResult<u64> {
        let result = query("DELETE FROM sessions WHERE expires_at < NOW() OR (is_active = false AND updated_at < NOW() - INTERVAL '7 days')")
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn count_active(&self) -> DomainResult<u64> {
        let count: i64 = query("SELECT COUNT(*) FROM sessions WHERE is_active = true AND expires_at > NOW()")
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        Ok(count as u64)
    }

    async fn count_active_by_user(&self, user_id: Uuid) -> DomainResult<u64> {
        let count: i64 = query("SELECT COUNT(*) FROM sessions WHERE user_id = $1 AND is_active = true AND expires_at > NOW()")
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        Ok(count as u64)
    }

    async fn get_statistics(&self) -> DomainResult<SessionStatistics> {
        let row = query(
            r#"
            SELECT
                COUNT(*) as total_sessions,
                COUNT(*) FILTER (WHERE is_active = true AND expires_at > NOW()) as active_sessions,
                COUNT(*) FILTER (WHERE created_at::date = CURRENT_DATE) as sessions_today,
                COUNT(DISTINCT user_id) FILTER (WHERE created_at::date = CURRENT_DATE) as unique_users_today,
                COALESCE(AVG(EXTRACT(EPOCH FROM (updated_at - created_at)) / 60), 0) as avg_session_duration_minutes
            FROM sessions
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(SessionStatistics {
            total_sessions: row.get::<i64, _>("total_sessions") as u64,
            active_sessions: row.get::<i64, _>("active_sessions") as u64,
            sessions_today: row.get::<i64, _>("sessions_today") as u64,
            unique_users_today: row.get::<i64, _>("unique_users_today") as u64,
            avg_session_duration_minutes: row.get::<f64, _>("avg_session_duration_minutes"),
        })
    }

    async fn find_by_ip(&self, ip_address: &str, pagination: Pagination) -> DomainResult<PaginatedResult<Session>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM sessions WHERE ip_address = $1")
            .bind(ip_address)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        // 获取会话
        let sessions: Vec<DbSession> = query_as(
            r#"
            SELECT id, user_id, token_hash, refresh_token, device_type, device_info,
                   ip_address, user_agent, is_active, expires_at, last_accessed_at,
                   created_at, updated_at
            FROM sessions
            WHERE ip_address = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(ip_address)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let sessions: Vec<Session> = sessions.into_iter().map(|s| s.into()).collect();
        Ok(PaginatedResult::new(sessions, total_count as u64, pagination))
    }

    async fn find_by_device_type(&self, device_type: &str, pagination: Pagination) -> DomainResult<PaginatedResult<Session>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM sessions WHERE device_type = $1")
            .bind(device_type)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?
            .get(0);

        // 获取会话
        let sessions: Vec<DbSession> = query_as(
            r#"
            SELECT id, user_id, token_hash, refresh_token, device_type, device_info,
                   ip_address, user_agent, is_active, expires_at, last_accessed_at,
                   created_at, updated_at
            FROM sessions
            WHERE device_type = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(device_type)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        let sessions: Vec<Session> = sessions.into_iter().map(|s| s.into()).collect();
        Ok(PaginatedResult::new(sessions, total_count as u64, pagination))
    }

    async fn invalidate_all_user_sessions(&self, user_id: Uuid) -> DomainResult<u64> {
        let result = query("UPDATE sessions SET is_active = false, updated_at = NOW() WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn invalidate_other_user_sessions(&self, user_id: Uuid, current_session_id: Uuid) -> DomainResult<u64> {
        let result = query("UPDATE sessions SET is_active = false, updated_at = NOW() WHERE user_id = $1 AND id != $2 AND is_active = true")
            .bind(user_id)
            .bind(current_session_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::DatabaseError(e.to_string()))?;

        Ok(result.rows_affected())
    }
}