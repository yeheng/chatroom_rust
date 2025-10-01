use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::errors::DomainError;

/// 统一的时间戳类型。
pub type Timestamp = OffsetDateTime;

/// 用户唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for UserId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<UserId> for Uuid {
    fn from(value: UserId) -> Self {
        value.0
    }
}

/// 聊天室唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoomId(pub Uuid);

impl RoomId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for RoomId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<RoomId> for Uuid {
    fn from(value: RoomId) -> Self {
        value.0
    }
}

/// 消息唯一标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new(id: Uuid) -> Self {
        Self(id)
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for MessageId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<MessageId> for Uuid {
    fn from(value: MessageId) -> Self {
        value.0
    }
}

/// 经过验证的用户名。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Username(String);

impl Username {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err(DomainError::invalid_argument("username", "cannot be empty"));
        }
        if value.len() > 50 {
            return Err(DomainError::invalid_argument("username", "too long"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 组织ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrgId(uuid::Uuid);

impl Default for OrgId {
    fn default() -> Self {
        Self::new()
    }
}

impl OrgId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn from(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl From<OrgId> for uuid::Uuid {
    fn from(id: OrgId) -> Self {
        id.0
    }
}

impl fmt::Display for OrgId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 组织路径(ltree格式)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgPath(String);

impl OrgPath {
    /// 创建根路径
    pub fn root(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// 从字符串解析路径
    pub fn parse(path: impl Into<String>) -> Result<Self, DomainError> {
        let path = path.into();
        // 验证ltree格式: 只能包含字母数字下划线和点
        if !path
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        {
            return Err(DomainError::invalid_argument("org_path", "格式不正确"));
        }
        Ok(Self(path))
    }

    /// 追加子节点
    pub fn append(&self, child: impl Into<String>) -> Result<Self, DomainError> {
        let child = child.into();
        Ok(Self(format!("{}.{}", self.0, child)))
    }

    /// 检查是否是指定路径的后代
    pub fn is_descendant_of(&self, ancestor: &OrgPath) -> bool {
        self.0.starts_with(&format!("{}.", ancestor.0))
    }

    /// 获取父路径
    pub fn parent(&self) -> Option<Self> {
        self.0.rfind('.').map(|pos| Self(self.0[..pos].to_owned()))
    }

    /// 获取层级
    pub fn level(&self) -> i32 {
        self.0.matches('.').count() as i32
    }
}

impl fmt::Display for OrgPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<OrgPath> for String {
    fn from(path: OrgPath) -> Self {
        path.0
    }
}

/// 经过验证的邮箱。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserEmail(String);

impl UserEmail {
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err(DomainError::invalid_argument("email", "cannot be empty"));
        }
        if !value.contains('@') {
            return Err(DomainError::invalid_argument("email", "must contain '@'"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 经过外部服务生成的密码哈希。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let hash = value.into();
        if hash.trim().is_empty() {
            return Err(DomainError::invalid_argument(
                "password_hash",
                "cannot be empty",
            ));
        }
        Ok(Self(hash))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 消息正文内容。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageContent(String);

impl MessageContent {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::invalid_argument(
                "message_content",
                "cannot be empty",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
