//! 消息Repository接口定义

use crate::entities::message::Message;
use crate::errors::DomainResult;
use crate::repositories::{PaginatedResult, Pagination, SortConfig};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// 消息统计信息
#[derive(Debug, Clone)]
pub struct MessageStatistics {
    pub total_messages: u64,
    pub text_messages: u64,
    pub image_messages: u64,
    pub file_messages: u64,
    pub emoji_messages: u64,
    pub deleted_messages: u64,
    pub messages_today: u64,
    pub avg_messages_per_room: f64,
}

/// 消息搜索参数
#[derive(Debug, Clone, Default)]
pub struct MessageSearchParams {
    pub room_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub content: Option<String>,
    pub message_type: Option<String>,
    pub is_edited: Option<bool>,
    pub is_deleted: Option<bool>,
    pub reply_to_message_id: Option<Uuid>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

/// 回复链信息
#[derive(Debug, Clone)]
pub struct ReplyChain {
    pub message_id: Uuid,
    pub reply_to_message_id: Option<Uuid>,
    pub content: String,
    pub user_id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub level: i32,
}

/// 消息Repository接口
#[async_trait]
pub trait MessageRepository: Send + Sync {
    /// 创建新消息
    async fn create(&self, message: &Message) -> DomainResult<Message>;

    /// 根据ID查找消息
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Option<Message>>;

    /// 更新消息内容
    async fn update(&self, message: &Message) -> DomainResult<Message>;

    /// 标记消息为已编辑
    async fn mark_as_edited(&self, message_id: Uuid, new_content: &str) -> DomainResult<()>;

    /// 软删除消息
    async fn soft_delete(&self, message_id: Uuid) -> DomainResult<()>;

    /// 彻底删除消息
    async fn hard_delete(&self, message_id: Uuid) -> DomainResult<()>;

    /// 获取房间消息历史
    async fn find_by_room(
        &self,
        room_id: Uuid,
        pagination: Pagination,
        include_deleted: bool,
    ) -> DomainResult<PaginatedResult<Message>>;

    /// 获取用户发送的消息
    async fn find_by_user(
        &self,
        user_id: Uuid,
        pagination: Pagination,
        include_deleted: bool,
    ) -> DomainResult<PaginatedResult<Message>>;

    /// 根据条件搜索消息
    async fn search(
        &self,
        params: &MessageSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<Message>>;

    /// 全文搜索消息内容
    async fn full_text_search(
        &self,
        query: &str,
        room_id: Option<Uuid>,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>>;

    /// 获取消息回复链
    async fn get_reply_chain(&self, message_id: Uuid) -> DomainResult<Vec<ReplyChain>>;

    /// 获取消息的回复列表
    async fn find_replies(
        &self,
        message_id: Uuid,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>>;

    /// 获取房间最新消息
    async fn find_latest_by_room(&self, room_id: Uuid, limit: u32) -> DomainResult<Vec<Message>>;

    /// 获取指定时间后的消息
    async fn find_by_room_after(
        &self,
        room_id: Uuid,
        after: DateTime<Utc>,
        limit: u32,
    ) -> DomainResult<Vec<Message>>;

    /// 获取指定时间前的消息
    async fn find_by_room_before(
        &self,
        room_id: Uuid,
        before: DateTime<Utc>,
        limit: u32,
    ) -> DomainResult<Vec<Message>>;

    /// 统计房间消息数量
    async fn count_by_room(&self, room_id: Uuid, include_deleted: bool) -> DomainResult<u64>;

    /// 统计用户消息数量
    async fn count_by_user(&self, user_id: Uuid, include_deleted: bool) -> DomainResult<u64>;

    /// 获取消息统计信息
    async fn get_statistics(&self) -> DomainResult<MessageStatistics>;

    /// 批量获取消息
    async fn find_by_ids(&self, ids: &[Uuid]) -> DomainResult<Vec<Message>>;

    /// 获取房间内包含特定关键词的消息
    async fn find_by_room_with_keyword(
        &self,
        room_id: Uuid,
        keyword: &str,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>>;

    /// 获取特定类型的消息
    async fn find_by_type(
        &self,
        message_type: &str,
        room_id: Option<Uuid>,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<Message>>;

    /// 清理过期的已删除消息
    async fn cleanup_deleted_messages(&self, older_than: DateTime<Utc>) -> DomainResult<u64>;

    /// 获取消息提及的用户
    async fn find_mentions(&self, message_id: Uuid) -> DomainResult<Vec<Uuid>>;

    /// 更新消息元数据
    async fn update_metadata(&self, message_id: Uuid, metadata: &JsonValue) -> DomainResult<()>;

    /// 获取房间消息统计（按天）
    async fn get_room_message_stats_by_day(
        &self,
        room_id: Uuid,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
    ) -> DomainResult<Vec<(DateTime<Utc>, u64)>>;
}
