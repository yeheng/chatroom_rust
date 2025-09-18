//! 文件上传实体定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 文件上传实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUpload {
    /// 文件ID
    pub id: Uuid,
    /// 上传用户ID
    pub user_id: Uuid,
    /// 所属房间ID（可选）
    pub room_id: Option<Uuid>,
    /// 文件名
    pub filename: String,
    /// 原始文件名
    pub original_filename: String,
    /// 文件大小（字节）
    pub file_size: i64,
    /// MIME类型
    pub mime_type: String,
    /// 存储路径
    pub storage_path: String,
    /// 存储类型 (local, s3, minio, azure, gcs)
    pub storage_type: String,
    /// 文件校验和
    pub checksum: Option<String>,
    /// 缩略图路径
    pub thumbnail_path: Option<String>,
    /// 是否公开
    pub is_public: bool,
    /// 是否临时文件
    pub is_temporary: bool,
    /// 下载次数
    pub download_count: u32,
    /// 过期时间
    pub expires_at: Option<DateTime<Utc>>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl FileUpload {
    /// 创建新的文件上传记录
    pub fn new(
        user_id: Uuid,
        room_id: Option<Uuid>,
        filename: String,
        original_filename: String,
        file_size: i64,
        mime_type: String,
        storage_path: String,
        storage_type: String,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            user_id,
            room_id,
            filename,
            original_filename,
            file_size,
            mime_type,
            storage_path,
            storage_type,
            checksum: None,
            thumbnail_path: None,
            is_public: false,
            is_temporary: false,
            download_count: 0,
            expires_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 设置校验和
    pub fn set_checksum(&mut self, checksum: String) {
        self.checksum = Some(checksum);
        self.updated_at = Utc::now();
    }

    /// 设置缩略图路径
    pub fn set_thumbnail(&mut self, thumbnail_path: String) {
        self.thumbnail_path = Some(thumbnail_path);
        self.updated_at = Utc::now();
    }

    /// 标记为公开文件
    pub fn make_public(&mut self) {
        self.is_public = true;
        self.updated_at = Utc::now();
    }

    /// 标记为私有文件
    pub fn make_private(&mut self) {
        self.is_public = false;
        self.updated_at = Utc::now();
    }

    /// 标记为永久文件
    pub fn make_permanent(&mut self) {
        self.is_temporary = false;
        self.expires_at = None;
        self.updated_at = Utc::now();
    }

    /// 设置过期时间
    pub fn set_expiry(&mut self, expires_at: DateTime<Utc>) {
        self.expires_at = Some(expires_at);
        self.updated_at = Utc::now();
    }

    /// 增加下载次数
    pub fn increment_download_count(&mut self) {
        self.download_count += 1;
        self.updated_at = Utc::now();
    }

    /// 检查文件是否已过期
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }

    /// 获取文件扩展名
    pub fn extension(&self) -> Option<&str> {
        std::path::Path::new(&self.filename)
            .extension()
            .and_then(|ext| ext.to_str())
    }

    /// 检查是否为图片文件
    pub fn is_image(&self) -> bool {
        self.mime_type.starts_with("image/")
    }

    /// 检查是否为视频文件
    pub fn is_video(&self) -> bool {
        self.mime_type.starts_with("video/")
    }

    /// 检查是否为音频文件
    pub fn is_audio(&self) -> bool {
        self.mime_type.starts_with("audio/")
    }
}