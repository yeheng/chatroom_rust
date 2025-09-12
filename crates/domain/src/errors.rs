//! 领域模型错误定义
//!
//! 定义了系统中所有可能的错误类型，提供清晰的错误上下文。

use thiserror::Error;

/// 领域模型错误类型
#[derive(Error, Debug, Clone, PartialEq)]
pub enum DomainError {
    /// 用户相关错误
    #[error("用户错误: {message}")]
    UserError { message: String },

    /// 聊天室相关错误
    #[error("聊天室错误: {message}")]
    ChatRoomError { message: String },

    /// 消息相关错误
    #[error("消息错误: {message}")]
    MessageError { message: String },

    /// 密码验证错误
    #[error("密码验证失败")]
    PasswordVerificationError,

    /// 密码哈希错误
    #[error("密码哈希失败: {message}")]
    PasswordHashError { message: String },

    /// 权限错误
    #[error("权限不足: {action}")]
    PermissionDenied { action: String },

    /// 资源不存在错误
    #[error("资源不存在: {resource_type} ID {resource_id}")]
    ResourceNotFound {
        resource_type: String,
        resource_id: String,
    },

    /// 资源已存在错误
    #[error("资源已存在: {resource_type} {identifier}")]
    ResourceAlreadyExists {
        resource_type: String,
        identifier: String,
    },

    /// 验证错误
    #[error("验证失败: {field}: {message}")]
    ValidationError { field: String, message: String },

    /// 业务规则违反错误
    #[error("业务规则违反: {rule}")]
    BusinessRuleViolation { rule: String },
}

impl DomainError {
    /// 创建用户错误
    pub fn user_error(message: impl Into<String>) -> Self {
        Self::UserError {
            message: message.into(),
        }
    }

    /// 创建聊天室错误
    pub fn chatroom_error(message: impl Into<String>) -> Self {
        Self::ChatRoomError {
            message: message.into(),
        }
    }

    /// 创建消息错误
    pub fn message_error(message: impl Into<String>) -> Self {
        Self::MessageError {
            message: message.into(),
        }
    }

    /// 创建验证错误
    pub fn validation_error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ValidationError {
            field: field.into(),
            message: message.into(),
        }
    }

    /// 创建权限错误
    pub fn permission_denied(action: impl Into<String>) -> Self {
        Self::PermissionDenied {
            action: action.into(),
        }
    }

    /// 创建资源不存在错误
    pub fn resource_not_found(
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
    ) -> Self {
        Self::ResourceNotFound {
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
        }
    }

    /// 创建资源已存在错误
    pub fn resource_already_exists(
        resource_type: impl Into<String>,
        identifier: impl Into<String>,
    ) -> Self {
        Self::ResourceAlreadyExists {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
        }
    }

    /// 创建业务规则违反错误
    pub fn business_rule_violation(rule: impl Into<String>) -> Self {
        Self::BusinessRuleViolation { rule: rule.into() }
    }
}

/// 领域模型结果类型
pub type DomainResult<T> = Result<T, DomainError>;
