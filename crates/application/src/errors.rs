//! 应用层错误定义
//!
//! 定义应用层特定的错误类型。

use domain::errors::DomainError;
use thiserror::Error;
use uuid::Uuid;

/// 应用层错误类型
#[derive(Debug, Error)]
pub enum ApplicationError {
    /// 聊天室相关错误
    #[error("聊天室错误: {0}")]
    ChatRoom(#[from] ChatRoomError),

    /// 消息相关错误
    #[error("消息错误: {0}")]
    Message(#[from] MessageError),

    /// 用户相关错误
    #[error("用户错误: {0}")]
    User(#[from] UserError),

    /// 领域层错误
    #[error("领域错误: {0}")]
    Domain(#[from] DomainError),

    /// 基础设施层错误
    #[error("基础设施错误: {0}")]
    Infrastructure(String),

    /// 未找到资源
    #[error("资源未找到: {0}")]
    NotFound(String),

    /// 权限不足
    #[error("权限不足: {0}")]
    Unauthorized(String),

    /// 验证错误
    #[error("验证失败: {0}")]
    Validation(String),

    /// 并发冲突
    #[error("并发冲突: {0}")]
    Conflict(String),
}

/// 应用层结果类型
pub type ApplicationResult<T> = Result<T, ApplicationError>;

/// 聊天室服务错误
#[derive(Debug, Error)]
pub enum ChatRoomError {
    /// 房间不存在
    #[error("房间不存在: {0}")]
    RoomNotFound(Uuid),

    /// 房间已满
    #[error("房间已满: {0}")]
    RoomFull(Uuid),

    /// 密码错误
    #[error("密码错误")]
    InvalidPassword,

    /// 用户已在房间中
    #[error("用户已在房间中: {0}")]
    UserAlreadyInRoom(Uuid),

    /// 用户不在房间中
    #[error("用户不在房间中: {0}")]
    UserNotInRoom(Uuid),

    /// 房间名称冲突
    #[error("房间名称冲突: {0}")]
    RoomNameConflict(String),

    /// 房间已删除
    #[error("房间已删除: {0}")]
    RoomDeleted(Uuid),

    /// 权限不足
    #[error("权限不足: {0}")]
    InsufficientPermissions(String),

    /// 操作被限制
    #[error("操作被限制: {0}")]
    RateLimited(String),

    /// 内部错误
    #[error("内部错误: {0}")]
    Internal(String),

    /// 验证错误
    #[error("验证失败: {0}")]
    Validation(String),
}

/// 消息服务错误
#[derive(Debug, Error)]
pub enum MessageError {
    /// 消息内容为空
    #[error("消息内容为空")]
    EmptyContent,

    /// 消息内容过长
    #[error("消息内容过长: {0} > {1}")]
    ContentTooLong(usize, usize),

    /// 敏感内容
    #[error("包含敏感内容")]
    SensitiveContent,

    /// 消息不存在
    #[error("消息不存在: {0}")]
    MessageNotFound(Uuid),

    /// 重复消息
    #[error("重复消息")]
    DuplicateMessage,

    /// 消息格式错误
    #[error("消息格式错误: {0}")]
    InvalidFormat(String),

    /// 发送失败
    #[error("消息发送失败: {0}")]
    SendFailed(String),

    /// 操作被限制
    #[error("操作被限制: {0}")]
    RateLimited(String),

    /// 内部错误
    #[error("内部错误: {0}")]
    Internal(String),

    /// 验证错误
    #[error("验证失败: {0}")]
    Validation(String),

    /// 权限不足
    #[error("权限不足: {0}")]
    Unauthorized(String),
}

/// 用户服务错误
#[derive(Debug, Error)]
pub enum UserError {
    /// 用户不存在
    #[error("用户不存在: {0}")]
    UserNotFound(Uuid),

    /// 根据用户名未找到用户
    #[error("用户名不存在: {0}")]
    UserNotFoundByUsername(String),

    /// 根据邮箱未找到用户
    #[error("邮箱不存在: {0}")]
    UserNotFoundByEmail(String),

    /// 用户名已存在
    #[error("用户名已存在: {0}")]
    UsernameAlreadyExists(String),

    /// 邮箱已存在
    #[error("邮箱已存在: {0}")]
    EmailAlreadyExists(String),

    /// 用户名冲突
    #[error("用户名冲突: {0}")]
    UsernameConflict(String),

    /// 邮箱冲突
    #[error("邮箱冲突: {0}")]
    EmailConflict(String),

    /// 无效凭据
    #[error("用户名或密码错误")]
    InvalidCredentials,

    /// 用户账户已停用
    #[error("用户账户已停用")]
    UserInactive,

    /// 密码强度不足
    #[error("密码强度不足")]
    WeakPassword,

    /// 用户状态错误
    #[error("用户状态错误: {0}")]
    InvalidUserStatus(String),

    /// 头像上传失败
    #[error("头像上传失败: {0}")]
    AvatarUploadFailed(String),

    /// 搜索查询无效
    #[error("搜索查询无效: {0}")]
    InvalidSearchQuery(String),

    /// 扩展字段验证失败
    #[error("扩展字段验证失败: {0}")]
    ExtensionValidationFailed(String),

    /// 内部错误
    #[error("内部错误: {0}")]
    Internal(String),

    /// 验证错误
    #[error("验证失败: {0}")]
    Validation(String),

    /// 权限不足
    #[error("权限不足: {0}")]
    Unauthorized(String),
}

// 为 AuthError 提供 From 实现
impl From<domain::entities::auth::AuthError> for ApplicationError {
    fn from(err: domain::entities::auth::AuthError) -> Self {
        ApplicationError::Infrastructure(format!("认证错误: {}", err))
    }
}
