//! 文件上传Repository接口定义

use crate::entities::FileUpload;
use crate::errors::DomainResult;
use crate::repositories::{Pagination, PaginatedResult, SortConfig};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 文件分享实体
#[derive(Debug, Clone)]
pub struct FileShare {
    pub id: Uuid,
    pub file_id: Uuid,
    pub shared_by: Uuid,
    pub share_token: String,
    pub share_name: Option<String>,
    pub password_hash: Option<String>,
    pub download_limit: Option<i32>,
    pub download_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// 文件访问日志实体
#[derive(Debug, Clone)]
pub struct FileAccessLog {
    pub id: Uuid,
    pub file_id: Uuid,
    pub user_id: Option<Uuid>,
    pub share_id: Option<Uuid>,
    pub access_type: String, // view, download, share, preview
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 文件统计信息
#[derive(Debug, Clone)]
pub struct FileStatistics {
    pub total_files: u64,
    pub total_size_bytes: i64,
    pub public_files: u64,
    pub temporary_files: u64,
    pub files_by_type: std::collections::HashMap<String, u64>,
    pub files_uploaded_today: u64,
    pub storage_usage_by_type: std::collections::HashMap<String, i64>,
}

/// 文件搜索参数
#[derive(Debug, Clone, Default)]
pub struct FileSearchParams {
    pub user_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub filename: Option<String>,
    pub mime_type: Option<String>,
    pub storage_type: Option<String>,
    pub is_public: Option<bool>,
    pub is_temporary: Option<bool>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

/// 文件上传Repository接口
#[async_trait]
pub trait FileUploadRepository: Send + Sync {
    /// 创建文件记录
    async fn create(&self, file: &FileUpload) -> DomainResult<FileUpload>;

    /// 根据ID查找文件
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<FileUpload>>;

    /// 根据校验和查找文件
    async fn find_by_checksum(&self, checksum: &str) -> DomainResult<Option<FileUpload>>;

    /// 更新文件信息
    async fn update(&self, file: &FileUpload) -> DomainResult<FileUpload>;

    /// 增加下载计数
    async fn increment_download_count(&self, file_id: Uuid) -> DomainResult<()>;

    /// 标记文件为非临时
    async fn mark_as_permanent(&self, file_id: Uuid) -> DomainResult<()>;

    /// 删除文件记录
    async fn delete(&self, file_id: Uuid) -> DomainResult<bool>;

    /// 获取用户上传的文件
    async fn find_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<FileUpload>>;

    /// 获取房间的文件
    async fn find_by_room(&self, room_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<FileUpload>>;

    /// 获取公开文件
    async fn find_public_files(&self, pagination: Pagination) -> DomainResult<PaginatedResult<FileUpload>>;

    /// 根据条件搜索文件
    async fn search(
        &self,
        params: &FileSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<FileUpload>>;

    /// 获取临时文件列表
    async fn find_temporary_files(&self, older_than: DateTime<Utc>) -> DomainResult<Vec<FileUpload>>;

    /// 获取过期文件列表
    async fn find_expired_files(&self) -> DomainResult<Vec<FileUpload>>;

    /// 清理临时文件
    async fn cleanup_temporary_files(&self, older_than_hours: u32) -> DomainResult<u64>;

    /// 清理过期文件
    async fn cleanup_expired_files(&self) -> DomainResult<u64>;

    /// 获取文件统计信息
    async fn get_statistics(&self) -> DomainResult<FileStatistics>;

    /// 根据存储类型获取文件
    async fn find_by_storage_type(&self, storage_type: &str, pagination: Pagination) -> DomainResult<PaginatedResult<FileUpload>>;

    /// 计算用户存储使用量
    async fn calculate_user_storage_usage(&self, user_id: Uuid) -> DomainResult<i64>;

    /// 计算房间存储使用量
    async fn calculate_room_storage_usage(&self, room_id: Uuid) -> DomainResult<i64>;

    /// 获取重复文件（相同校验和）
    async fn find_duplicate_files(&self, checksum: &str) -> DomainResult<Vec<FileUpload>>;
}

/// 文件分享Repository接口
#[async_trait]
pub trait FileShareRepository: Send + Sync {
    /// 创建分享链接
    async fn create(&self, share: &FileShare) -> DomainResult<FileShare>;

    /// 根据token查找分享
    async fn find_by_token(&self, token: &str) -> DomainResult<Option<FileShare>>;

    /// 根据文件ID查找分享
    async fn find_by_file(&self, file_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<FileShare>>;

    /// 根据分享者查找分享
    async fn find_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<FileShare>>;

    /// 增加下载计数
    async fn increment_download_count(&self, share_id: Uuid) -> DomainResult<()>;

    /// 停用分享链接
    async fn deactivate(&self, share_id: Uuid) -> DomainResult<()>;

    /// 删除分享链接
    async fn delete(&self, share_id: Uuid) -> DomainResult<bool>;

    /// 验证分享访问权限
    async fn validate_access(&self, token: &str, password: Option<&str>) -> DomainResult<Option<(FileUpload, FileShare)>>;

    /// 清理过期分享链接
    async fn cleanup_expired_shares(&self) -> DomainResult<u64>;

    /// 获取活跃分享链接
    async fn find_active_shares(&self, pagination: Pagination) -> DomainResult<PaginatedResult<FileShare>>;
}

/// 文件访问日志Repository接口
#[async_trait]
pub trait FileAccessLogRepository: Send + Sync {
    /// 记录文件访问
    async fn log_access(&self, log: &FileAccessLog) -> DomainResult<FileAccessLog>;

    /// 获取文件访问历史
    async fn find_by_file(&self, file_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<FileAccessLog>>;

    /// 获取用户访问历史
    async fn find_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<FileAccessLog>>;

    /// 根据访问类型统计
    async fn count_by_access_type(&self, file_id: Uuid, access_type: &str) -> DomainResult<u64>;

    /// 获取文件下载统计
    async fn get_download_stats(&self, file_id: Uuid, days: u32) -> DomainResult<Vec<(chrono::NaiveDate, u64)>>;

    /// 清理旧的访问日志
    async fn cleanup_old_logs(&self, older_than: DateTime<Utc>) -> DomainResult<u64>;
}