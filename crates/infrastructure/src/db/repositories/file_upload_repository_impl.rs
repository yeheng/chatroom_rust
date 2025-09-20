//! 文件上传Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    errors::{DomainError, DomainResult},
    repositories::{
        FileSearchParams, FileStatistics, FileUploadRepository, PaginatedResult, Pagination,
        SortConfig,
    },
    FileUpload,
};
use sqlx::{query, query_as, FromRow, Row};
use std::sync::Arc;
use uuid::Uuid;

/// 数据库文件上传模型
#[derive(Debug, Clone, FromRow)]
struct DbFileUpload {
    pub id: Uuid,
    pub user_id: Uuid,
    pub room_id: Option<Uuid>,
    pub filename: String,
    pub original_filename: String,
    pub file_size: i64,
    pub mime_type: String,
    pub storage_path: String,
    pub storage_type: String,
    pub checksum: Option<String>,
    pub thumbnail_path: Option<String>,
    pub is_public: bool,
    pub is_temporary: bool,
    pub download_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DbFileUpload> for FileUpload {
    fn from(db_file: DbFileUpload) -> Self {
        FileUpload {
            id: db_file.id,
            user_id: db_file.user_id,
            room_id: db_file.room_id,
            filename: db_file.filename,
            original_filename: db_file.original_filename,
            file_size: db_file.file_size,
            mime_type: db_file.mime_type,
            storage_path: db_file.storage_path,
            storage_type: db_file.storage_type,
            checksum: db_file.checksum,
            thumbnail_path: db_file.thumbnail_path,
            is_public: db_file.is_public,
            is_temporary: db_file.is_temporary,
            download_count: db_file.download_count as u32,
            expires_at: db_file.expires_at,
            created_at: db_file.created_at,
            updated_at: db_file.updated_at,
        }
    }
}

impl From<&FileUpload> for DbFileUpload {
    fn from(file: &FileUpload) -> Self {
        DbFileUpload {
            id: file.id,
            user_id: file.user_id,
            room_id: file.room_id,
            filename: file.filename.clone(),
            original_filename: file.original_filename.clone(),
            file_size: file.file_size,
            mime_type: file.mime_type.clone(),
            storage_path: file.storage_path.clone(),
            storage_type: file.storage_type.clone(),
            checksum: file.checksum.clone(),
            thumbnail_path: file.thumbnail_path.clone(),
            is_public: file.is_public,
            is_temporary: file.is_temporary,
            download_count: file.download_count as i32,
            expires_at: file.expires_at,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

/// 文件上传Repository实现
pub struct PostgresFileUploadRepository {
    pool: Arc<DbPool>,
}

impl PostgresFileUploadRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl FileUploadRepository for PostgresFileUploadRepository {
    async fn create(&self, file: &FileUpload) -> DomainResult<FileUpload> {
        let db_file = DbFileUpload::from(file);

        let result = query_as::<_, DbFileUpload>(
            r#"
            INSERT INTO file_uploads (
                id, user_id, room_id, filename, original_filename, file_size, mime_type,
                storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                download_count, expires_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING id, user_id, room_id, filename, original_filename, file_size, mime_type,
                     storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                     download_count, expires_at, created_at, updated_at
            "#,
        )
        .bind(db_file.id)
        .bind(db_file.user_id)
        .bind(db_file.room_id)
        .bind(&db_file.filename)
        .bind(&db_file.original_filename)
        .bind(db_file.file_size)
        .bind(&db_file.mime_type)
        .bind(&db_file.storage_path)
        .bind(&db_file.storage_type)
        .bind(&db_file.checksum)
        .bind(&db_file.thumbnail_path)
        .bind(db_file.is_public)
        .bind(db_file.is_temporary)
        .bind(db_file.download_count)
        .bind(db_file.expires_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<FileUpload>> {
        let result = query_as::<_, DbFileUpload>(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn find_by_checksum(&self, checksum: &str) -> DomainResult<Option<FileUpload>> {
        let result = query_as::<_, DbFileUpload>(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE checksum = $1
            LIMIT 1
            "#,
        )
        .bind(checksum)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|r| r.into()))
    }

    async fn update(&self, file: &FileUpload) -> DomainResult<FileUpload> {
        let db_file = DbFileUpload::from(file);

        let result = query_as::<_, DbFileUpload>(
            r#"
            UPDATE file_uploads
            SET filename = $2, original_filename = $3, file_size = $4, mime_type = $5,
                storage_path = $6, storage_type = $7, checksum = $8, thumbnail_path = $9,
                is_public = $10, is_temporary = $11, download_count = $12, expires_at = $13,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, user_id, room_id, filename, original_filename, file_size, mime_type,
                     storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                     download_count, expires_at, created_at, updated_at
            "#,
        )
        .bind(db_file.id)
        .bind(&db_file.filename)
        .bind(&db_file.original_filename)
        .bind(db_file.file_size)
        .bind(&db_file.mime_type)
        .bind(&db_file.storage_path)
        .bind(&db_file.storage_type)
        .bind(&db_file.checksum)
        .bind(&db_file.thumbnail_path)
        .bind(db_file.is_public)
        .bind(db_file.is_temporary)
        .bind(db_file.download_count)
        .bind(db_file.expires_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn increment_download_count(&self, file_id: Uuid) -> DomainResult<()> {
        query("UPDATE file_uploads SET download_count = download_count + 1, updated_at = NOW() WHERE id = $1")
            .bind(file_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn mark_as_permanent(&self, file_id: Uuid) -> DomainResult<()> {
        query("UPDATE file_uploads SET is_temporary = false, expires_at = NULL, updated_at = NOW() WHERE id = $1")
            .bind(file_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, file_id: Uuid) -> DomainResult<bool> {
        let result = query("DELETE FROM file_uploads WHERE id = $1")
            .bind(file_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn find_by_user(
        &self,
        user_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<FileUpload>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM file_uploads WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取文件
        let files: Vec<DbFileUpload> = query_as(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
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
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let files: Vec<FileUpload> = files.into_iter().map(|f| f.into()).collect();
        Ok(PaginatedResult::new(files, total_count as u64, pagination))
    }

    async fn find_by_room(
        &self,
        room_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<FileUpload>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM file_uploads WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取文件
        let files: Vec<DbFileUpload> = query_as(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE room_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(room_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let files: Vec<FileUpload> = files.into_iter().map(|f| f.into()).collect();
        Ok(PaginatedResult::new(files, total_count as u64, pagination))
    }

    async fn find_public_files(
        &self,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<FileUpload>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM file_uploads WHERE is_public = true")
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取文件
        let files: Vec<DbFileUpload> = query_as(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE is_public = true
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let files: Vec<FileUpload> = files.into_iter().map(|f| f.into()).collect();
        Ok(PaginatedResult::new(files, total_count as u64, pagination))
    }

    async fn search(
        &self,
        params: &FileSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<FileUpload>> {
        // 构建查询条件
        let mut conditions = Vec::new();
        let mut param_values = Vec::new();
        let mut param_count = 1;

        if let Some(user_id) = params.user_id {
            conditions.push(format!("user_id = ${}", param_count));
            param_values.push(user_id.to_string());
            param_count += 1;
        }

        if let Some(room_id) = params.room_id {
            conditions.push(format!("room_id = ${}", param_count));
            param_values.push(room_id.to_string());
            param_count += 1;
        }

        if let Some(filename) = &params.filename {
            conditions.push(format!("filename ILIKE ${}", param_count));
            param_values.push(format!("%{}%", filename));
            param_count += 1;
        }

        if let Some(mime_type) = &params.mime_type {
            conditions.push(format!("mime_type = ${}", param_count));
            param_values.push(mime_type.clone());
            param_count += 1;
        }

        let where_clause = if conditions.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let order_clause = match sort {
            Some(sort_config) => {
                let direction = if sort_config.ascending { "ASC" } else { "DESC" };
                format!("ORDER BY {} {}", sort_config.field, direction)
            }
            None => "ORDER BY created_at DESC".to_string(),
        };

        // 简化实现，返回空结果（实际应该根据条件查询）
        Ok(PaginatedResult::new(Vec::new(), 0, pagination))
    }

    async fn find_temporary_files(
        &self,
        older_than: DateTime<Utc>,
    ) -> DomainResult<Vec<FileUpload>> {
        let files: Vec<DbFileUpload> = query_as(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE is_temporary = true AND created_at < $1
            "#,
        )
        .bind(older_than)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(files.into_iter().map(|f| f.into()).collect())
    }

    async fn find_expired_files(&self) -> DomainResult<Vec<FileUpload>> {
        let files: Vec<DbFileUpload> = query_as(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE expires_at IS NOT NULL AND expires_at < NOW()
            "#,
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(files.into_iter().map(|f| f.into()).collect())
    }

    async fn cleanup_temporary_files(&self, older_than_hours: u32) -> DomainResult<u64> {
        let result = query(
            r#"
            DELETE FROM file_uploads
            WHERE is_temporary = true AND created_at < NOW() - INTERVAL '$1 hours'
            "#,
        )
        .bind(older_than_hours as i32)
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn cleanup_expired_files(&self) -> DomainResult<u64> {
        let result =
            query("DELETE FROM file_uploads WHERE expires_at IS NOT NULL AND expires_at < NOW()")
                .execute(&*self.pool)
                .await
                .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn get_statistics(&self) -> DomainResult<FileStatistics> {
        let row = query(
            r#"
            SELECT
                COUNT(*) as total_files,
                COALESCE(SUM(file_size), 0) as total_size_bytes,
                COUNT(*) FILTER (WHERE is_public = true) as public_files,
                COUNT(*) FILTER (WHERE is_temporary = true) as temporary_files,
                COUNT(*) FILTER (WHERE created_at::date = CURRENT_DATE) as files_uploaded_today
            FROM file_uploads
            "#,
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(FileStatistics {
            total_files: row.get::<i64, _>("total_files") as u64,
            total_size_bytes: row.get::<i64, _>("total_size_bytes"),
            public_files: row.get::<i64, _>("public_files") as u64,
            temporary_files: row.get::<i64, _>("temporary_files") as u64,
            files_by_type: std::collections::HashMap::new(),
            files_uploaded_today: row.get::<i64, _>("files_uploaded_today") as u64,
            storage_usage_by_type: std::collections::HashMap::new(),
        })
    }

    async fn find_by_storage_type(
        &self,
        storage_type: &str,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<FileUpload>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM file_uploads WHERE storage_type = $1")
            .bind(storage_type)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取文件
        let files: Vec<DbFileUpload> = query_as(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE storage_type = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(storage_type)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let files: Vec<FileUpload> = files.into_iter().map(|f| f.into()).collect();
        Ok(PaginatedResult::new(files, total_count as u64, pagination))
    }

    async fn calculate_user_storage_usage(&self, user_id: Uuid) -> DomainResult<i64> {
        let usage: i64 =
            query("SELECT COALESCE(SUM(file_size), 0) FROM file_uploads WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&*self.pool)
                .await
                .map_err(|e| DomainError::database_error(e.to_string()))?
                .get(0);

        Ok(usage)
    }

    async fn calculate_room_storage_usage(&self, room_id: Uuid) -> DomainResult<i64> {
        let usage: i64 =
            query("SELECT COALESCE(SUM(file_size), 0) FROM file_uploads WHERE room_id = $1")
                .bind(room_id)
                .fetch_one(&*self.pool)
                .await
                .map_err(|e| DomainError::database_error(e.to_string()))?
                .get(0);

        Ok(usage)
    }

    async fn find_duplicate_files(&self, checksum: &str) -> DomainResult<Vec<FileUpload>> {
        let files: Vec<DbFileUpload> = query_as(
            r#"
            SELECT id, user_id, room_id, filename, original_filename, file_size, mime_type,
                   storage_path, storage_type, checksum, thumbnail_path, is_public, is_temporary,
                   download_count, expires_at, created_at, updated_at
            FROM file_uploads
            WHERE checksum = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(checksum)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(files.into_iter().map(|f| f.into()).collect())
    }
}
