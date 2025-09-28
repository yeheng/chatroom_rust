use async_trait::async_trait;
use domain::{MessageId, RepositoryError, UserId};

/// 消息传递追踪器 - 确保消息可靠传输
/// 每条消息分配唯一ID，追踪送达状态，确保可靠传输
#[async_trait]
pub trait DeliveryTracker: Send + Sync {
    /// 标记消息已发送给指定用户
    async fn mark_sent(&self, message_id: MessageId, user_id: UserId) -> Result<(), RepositoryError>;

    /// 标记消息已成功送达给指定用户
    async fn mark_delivered(&self, message_id: MessageId, user_id: UserId) -> Result<(), RepositoryError>;

    /// 获取用户的所有未送达消息ID列表
    async fn get_undelivered(&self, user_id: UserId) -> Result<Vec<MessageId>, RepositoryError>;

    /// 清理已送达的旧记录（用于定期清理）
    async fn cleanup_delivered(&self, older_than_hours: u32) -> Result<u64, RepositoryError>;
}