//! WebSocket相关实体
//!
//! 定义WebSocket消息、连接等实体。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// WebSocket消息类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    /// 客户端到服务器的消息
    ClientToServer(ClientMessage),
    /// 服务器到客户端的消息
    ServerToClient(ServerMessage),
}

/// 客户端消息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    /// 加入房间
    JoinRoom {
        /// 房间ID
        room_id: String,
        /// 房间密码（可选）
        password: Option<String>,
    },
    /// 离开房间
    LeaveRoom {
        /// 房间ID
        room_id: String,
    },
    /// 发送消息
    SendMessage {
        /// 房间ID
        room_id: String,
        /// 消息内容
        content: String,
        /// 消息类型（文本/图片等）
        message_type: String,
        /// 回复的消息ID（可选）
        reply_to: Option<Uuid>,
    },
    /// 编辑消息
    EditMessage {
        /// 消息ID
        message_id: Uuid,
        /// 新内容
        content: String,
    },
    /// 删除消息
    DeleteMessage {
        /// 消息ID
        message_id: Uuid,
    },
    /// 心跳响应
    Pong,
    /// 获取房间历史消息
    GetHistory {
        /// 房间ID
        room_id: String,
        /// 分页数量
        limit: Option<u32>,
        /// 最后一条消息ID
        last_message_id: Option<Uuid>,
    },
}

/// 服务器消息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    /// 消息已发送
    MessageSent {
        /// 消息ID
        message_id: Uuid,
        /// 房间ID
        room_id: String,
        /// 发送者用户ID
        sender_id: Uuid,
        /// 发送者用户名
        sender_username: String,
        /// 内容
        content: String,
        /// 消息类型
        message_type: String,
        /// 时间戳
        timestamp: DateTime<Utc>,
        /// 回复的消息ID（可选）
        reply_to: Option<Uuid>,
    },
    /// 消息已编辑
    MessageEdited {
        /// 消息ID
        message_id: Uuid,
        /// 新内容
        content: String,
        /// 编辑时间
        edited_at: DateTime<Utc>,
    },
    /// 消息已删除
    MessageDeleted {
        /// 消息ID
        message_id: Uuid,
        /// 删除时间
        deleted_at: DateTime<Utc>,
    },
    /// 用户加入房间
    UserJoined {
        /// 用户ID
        user_id: Uuid,
        /// 用户名
        username: String,
        /// 房间ID
        room_id: String,
        /// 加入时间
        joined_at: DateTime<Utc>,
    },
    /// 用户离开房间
    UserLeft {
        /// 用户ID
        user_id: Uuid,
        /// 用户名
        username: String,
        /// 房间ID
        room_id: String,
        /// 离开时间
        left_at: DateTime<Utc>,
    },
    /// 房间加入成功
    RoomJoined {
        /// 房间ID
        room_id: String,
        /// 房间名称
        room_name: String,
        /// 当前用户数
        user_count: u32,
        /// 加入时间
        joined_at: DateTime<Utc>,
    },
    /// 房间离开成功
    RoomLeft {
        /// 房间ID
        room_id: String,
        /// 离开时间
        left_at: DateTime<Utc>,
    },
    /// 错误消息
    Error {
        /// 错误代码
        code: String,
        /// 错误消息
        message: String,
        /// 错误详情（可选）
        details: Option<HashMap<String, String>>,
    },
    /// 心跳请求
    Ping,
    /// 历史消息
    History {
        /// 房间ID
        room_id: String,
        /// 消息列表
        messages: Vec<SentMessage>,
        /// 是否还有更多
        has_more: bool,
    },
    /// 用户状态更新
    UserStatusUpdate {
        /// 用户ID
        user_id: Uuid,
        /// 用户名
        username: String,
        /// 状态
        status: UserStatus,
        /// 更新时间
        updated_at: DateTime<Utc>,
    },
    /// 房间信息更新
    RoomInfoUpdate {
        /// 房间ID
        room_id: String,
        /// 房间名称
        room_name: Option<String>,
        /// 房间描述
        room_description: Option<String>,
        /// 用户数
        user_count: Option<u32>,
        /// 更新时间
        updated_at: DateTime<Utc>,
    },
}

/// 用户状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserStatus {
    /// 在线
    Online,
    /// 离开
    Away,
    /// 忙碌
    Busy,
    /// 离线
    Offline,
}

/// 已发送的消息（用于历史记录）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SentMessage {
    /// 消息ID
    pub message_id: Uuid,
    /// 发送者用户ID
    pub sender_id: Uuid,
    /// 发送者用户名
    pub sender_username: String,
    /// 内容
    pub content: String,
    /// 消息类型
    pub message_type: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 回复的消息ID（可选）
    pub reply_to: Option<Uuid>,
}

/// WebSocket消息帧
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketFrame {
    /// 消息ID（用于追踪）
    pub message_id: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 消息类型
    pub message_type: MessageType,
    /// 消息载荷
    pub payload: serde_json::Value,
}

/// WebSocket连接信息
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// 连接ID
    pub connection_id: Uuid,
    /// 用户ID
    pub user_id: Uuid,
    /// 用户名
    pub username: String,
    /// 连接建立时间
    pub connected_at: DateTime<Utc>,
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
    /// 当前房间列表
    pub rooms: Vec<String>,
    /// 连接状态
    pub status: ConnectionStatus,
    /// 客户端信息
    pub client_info: ClientInfo,
}

/// 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    /// 已连接
    Connected,
    /// 已认证
    Authenticated,
    /// 已断开
    Disconnected,
}

/// 客户端信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// 用户代理
    pub user_agent: Option<String>,
    /// IP地址
    pub ip_address: Option<String>,
    /// 客户端版本
    pub client_version: Option<String>,
    /// 设备类型
    pub device_type: Option<String>,
    /// 浏览器信息
    pub browser_info: Option<HashMap<String, String>>,
}

/// WebSocket错误类型
#[derive(Debug, Clone, thiserror::Error)]
pub enum WebSocketError {
    /// 消息格式错误
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    /// 消息类型错误
    #[error("Invalid message type: {0}")]
    InvalidType(String),
    /// 认证失败
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    /// 权限不足
    #[error("Insufficient permissions: {0}")]
    PermissionDenied(String),
    /// 房间不存在
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    /// 房间已满
    #[error("Room is full: {0}")]
    RoomFull(String),
    /// 无效的房间密码
    #[error("Invalid room password: {0}")]
    InvalidPassword(String),
    /// 用户未在房间中
    #[error("User not in room: {0}")]
    UserNotInRoom(String),
    /// 消息发送失败
    #[error("Failed to send message: {0}")]
    SendFailed(String),
    /// 连接已关闭
    #[error("Connection closed: {0}")]
    ConnectionClosed(String),
    /// 内部错误
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// 心跳配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    /// 心跳间隔（秒）
    pub interval_seconds: u64,
    /// 心跳超时（秒）
    pub timeout_seconds: u64,
    /// 最大丢失心跳次数
    pub max_missed_heartbeats: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 30,
            timeout_seconds: 90,
            max_missed_heartbeats: 3,
        }
    }
}

/// WebSocket服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketServerConfig {
    /// 最大连接数
    pub max_connections: usize,
    /// 消息大小限制（字节）
    pub max_message_size: usize,
    /// 心跳配置
    pub heartbeat: HeartbeatConfig,
    /// 认证令牌过期时间（秒）
    pub auth_token_expiry: u64,
    /// 连接清理间隔（秒）
    pub cleanup_interval_seconds: u64,
}

impl Default for WebSocketServerConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            max_message_size: 1024 * 1024, // 1MB
            heartbeat: HeartbeatConfig::default(),
            auth_token_expiry: 3600,       // 1小时
            cleanup_interval_seconds: 300, // 5分钟
        }
    }
}

impl WebSocketFrame {
    /// 创建新的WebSocket消息帧
    pub fn new(message_type: MessageType, payload: serde_json::Value) -> Self {
        Self {
            message_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            message_type,
            payload,
        }
    }

    /// 创建错误响应
    pub fn error(code: String, message: String, details: Option<HashMap<String, String>>) -> Self {
        let error_message = ServerMessage::Error {
            code,
            message,
            details,
        };
        Self::new(
            MessageType::ServerToClient(error_message),
            serde_json::Value::Null,
        )
    }

    /// 创建心跳响应
    pub fn pong() -> Self {
        Self::new(
            MessageType::ClientToServer(ClientMessage::Pong),
            serde_json::Value::Null,
        )
    }

    /// 创建心跳请求
    pub fn ping() -> Self {
        Self::new(
            MessageType::ServerToClient(ServerMessage::Ping),
            serde_json::Value::Null,
        )
    }

    /// 序列化为JSON字符串
    pub fn to_json(&self) -> Result<String, WebSocketError> {
        serde_json::to_string(self).map_err(|e| WebSocketError::InvalidFormat(e.to_string()))
    }

    /// 从JSON字符串反序列化
    pub fn from_json(json: &str) -> Result<Self, WebSocketError> {
        serde_json::from_str(json).map_err(|e| WebSocketError::InvalidFormat(e.to_string()))
    }
}

impl ConnectionInfo {
    /// 创建新的连接信息
    pub fn new(user_id: Uuid, username: String, client_info: ClientInfo) -> Self {
        let now = Utc::now();
        Self {
            connection_id: Uuid::new_v4(),
            user_id,
            username,
            connected_at: now,
            last_active: now,
            rooms: Vec::new(),
            status: ConnectionStatus::Connected,
            client_info,
        }
    }

    /// 更新最后活跃时间
    pub fn update_activity(&mut self) {
        self.last_active = Utc::now();
    }

    /// 加入房间
    pub fn join_room(&mut self, room_id: String) {
        if !self.rooms.contains(&room_id) {
            self.rooms.push(room_id);
        }
    }

    /// 离开房间
    pub fn leave_room(&mut self, room_id: &str) {
        self.rooms.retain(|r| r != room_id);
    }

    /// 检查是否在房间中
    pub fn is_in_room(&self, room_id: &str) -> bool {
        self.rooms.contains(&room_id.to_string())
    }

    /// 获取房间数量
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }
}

impl ClientInfo {
    /// 创建新的客户端信息
    pub fn new(user_agent: Option<String>, ip_address: Option<String>) -> Self {
        Self {
            user_agent,
            ip_address,
            client_version: None,
            device_type: None,
            browser_info: None,
        }
    }

    /// 从用户代理字符串提取浏览器信息
    pub fn extract_browser_info(&mut self) {
        if let Some(ua) = &self.user_agent {
            let mut info = HashMap::new();

            // 简单的浏览器检测
            if ua.contains("Chrome") {
                info.insert("browser".to_string(), "Chrome".to_string());
            } else if ua.contains("Firefox") {
                info.insert("browser".to_string(), "Firefox".to_string());
            } else if ua.contains("Safari") {
                info.insert("browser".to_string(), "Safari".to_string());
            }

            // 操作系统检测
            if ua.contains("Windows") {
                info.insert("os".to_string(), "Windows".to_string());
            } else if ua.contains("Mac") {
                info.insert("os".to_string(), "macOS".to_string());
            } else if ua.contains("Linux") {
                info.insert("os".to_string(), "Linux".to_string());
            }

            if !info.is_empty() {
                self.browser_info = Some(info);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_frame_serialization() {
        let frame = WebSocketFrame::new(
            MessageType::ClientToServer(ClientMessage::Pong),
            serde_json::Value::Null,
        );

        let json = frame.to_json().unwrap();
        let deserialized = WebSocketFrame::from_json(&json).unwrap();

        assert_eq!(frame.message_id, deserialized.message_id);
        assert_eq!(frame.message_type, deserialized.message_type);
    }

    #[test]
    fn test_connection_info() {
        let mut connection = ConnectionInfo::new(
            Uuid::new_v4(),
            "testuser".to_string(),
            ClientInfo::new(Some("Mozilla/5.0".to_string()), None),
        );

        assert_eq!(connection.room_count(), 0);
        assert!(!connection.is_in_room("test_room"));

        connection.join_room("test_room".to_string());
        assert_eq!(connection.room_count(), 1);
        assert!(connection.is_in_room("test_room"));

        connection.leave_room("test_room");
        assert_eq!(connection.room_count(), 0);
        assert!(!connection.is_in_room("test_room"));
    }

    #[test]
    fn test_client_info() {
        let mut client_info = ClientInfo::new(
            Some("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".to_string()),
            Some("192.168.1.1".to_string()),
        );

        client_info.extract_browser_info();

        assert!(client_info.browser_info.is_some());
        let browser_info = client_info.browser_info.as_ref().unwrap();
        assert_eq!(browser_info.get("browser"), Some(&"Chrome".to_string()));
        assert_eq!(browser_info.get("os"), Some(&"Windows".to_string()));
    }
}
