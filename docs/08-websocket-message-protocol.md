# WebSocket消息协议

本节详细说明WebSocket通信的消息协议，包括消息格式定义、客户端到服务器消息、服务器到客户端消息、消息流程示例等。

## 📡 协议概述

### 协议版本

- **当前版本**: 1.0.0
- **协议类型**: JSON-based text protocol
- **传输方式**: WebSocket (RFC 6455)
- **字符编码**: UTF-8

### 消息格式

所有WebSocket消息都采用JSON格式，包含以下基本结构：

```json
{
  "type": "message_type",
  "id": "unique_message_id",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    // 消息具体内容
  }
}
```

### 消息类型

| 类型 | 方向 | 描述 |
|------|------|------|
| `ping` | 客户端 → 服务器 | 心跳检测 |
| `pong` | 服务器 → 客户端 | 心跳响应 |
| `message` | 双向 | 聊天消息 |
| `join_room` | 客户端 → 服务器 | 加入房间 |
| `leave_room` | 客户端 → 服务器 | 离开房间 |
| `user_joined` | 服务器 → 客户端 | 用户加入通知 |
| `user_left` | 服务器 → 客户端 | 用户离开通知 |
| `typing_start` | 客户端 → 服务器 | 开始输入 |
| `typing_stop` | 客户端 → 服务器 | 停止输入 |
| `user_typing` | 服务器 → 客户端 | 用户正在输入 |
| `read_receipt` | 客户端 → 服务器 | 消息已读回执 |
| `message_read` | 服务器 → 客户端 | 消息已读通知 |
| `presence_update` | 服务器 → 客户端 | 用户状态更新 |
| `room_updated` | 服务器 → 客户端 | 房间信息更新 |
| `error` | 服务器 → 客户端 | 错误消息 |
| `system` | 服务器 → 客户端 | 系统消息 |

## 🔄 客户端到服务器消息

### 心跳消息

```json
{
  "type": "ping",
  "id": "ping_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {}
}
```

### 发送聊天消息

```json
{
  "type": "message",
  "id": "msg_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "content": "Hello, World!",
    "message_type": "text",
    "reply_to_message_id": null,
    "metadata": {}
  }
}
```

### 加入房间

```json
{
  "type": "join_room",
  "id": "join_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "password": null
  }
}
```

### 离开房间

```json
{
  "type": "leave_room",
  "id": "leave_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

### 开始输入

```json
{
  "type": "typing_start",
  "id": "typing_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

### 停止输入

```json
{
  "type": "typing_stop",
  "id": "typing_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

### 消息已读回执

```json
{
  "type": "read_receipt",
  "id": "read_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "message_id": "550e8400-e29b-41d4-a716-446655440001",
    "read_at": "2024-01-15T10:30:00Z"
  }
}
```

## 📤 服务器到客户端消息

### 心跳响应

```json
{
  "type": "pong",
  "id": "pong_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "server_time": "2024-01-15T10:30:00Z",
    "ping_id": "ping_123456"
  }
}
```

### 聊天消息

```json
{
  "type": "message",
  "id": "msg_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "message": {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "room_id": "550e8400-e29b-41d4-a716-446655440000",
      "user_id": "550e8400-e29b-41d4-a716-446655440002",
      "username": "john_doe",
      "avatar_url": "https://example.com/avatar.jpg",
      "content": "Hello, World!",
      "message_type": "text",
      "reply_to_message_id": null,
      "reply_to_username": null,
      "is_edited": false,
      "created_at": "2024-01-15T10:30:00Z",
      "updated_at": "2024-01-15T10:30:00Z"
    }
  }
}
```

### 用户加入通知

```json
{
  "type": "user_joined",
  "id": "join_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440002",
      "username": "john_doe",
      "avatar_url": "https://example.com/avatar.jpg",
      "joined_at": "2024-01-15T10:30:00Z"
    },
    "member_count": 42
  }
}
```

### 用户离开通知

```json
{
  "type": "user_left",
  "id": "left_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440002",
      "username": "john_doe",
      "avatar_url": "https://example.com/avatar.jpg",
      "left_at": "2024-01-15T10:30:00Z"
    },
    "member_count": 41
  }
}
```

### 用户正在输入

```json
{
  "type": "user_typing",
  "id": "typing_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440002",
      "username": "john_doe",
      "avatar_url": "https://example.com/avatar.jpg"
    },
    "typing": true
  }
}
```

### 消息已读通知

```json
{
  "type": "message_read",
  "id": "read_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "message_id": "550e8400-e29b-41d4-a716-446655440001",
    "read_by": {
      "id": "550e8400-e29b-41d4-a716-446655440003",
      "username": "jane_doe",
      "avatar_url": "https://example.com/avatar2.jpg"
    },
    "read_at": "2024-01-15T10:30:00Z"
  }
}
```

### 用户状态更新

```json
{
  "type": "presence_update",
  "id": "presence_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "user_id": "550e8400-e29b-41d4-a716-446655440002",
    "status": "online",
    "last_seen": "2024-01-15T10:30:00Z",
    "current_room_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

### 房间信息更新

```json
{
  "type": "room_updated",
  "id": "room_update_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "General",
      "description": "General discussion room",
      "is_private": false,
      "member_count": 42,
      "updated_at": "2024-01-15T10:30:00Z"
    },
    "changes": {
      "name": "General Room",
      "description": "Updated description"
    }
  }
}
```

### 错误消息

```json
{
  "type": "error",
  "id": "error_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "code": "ROOM_NOT_FOUND",
    "message": "Room not found",
    "details": {
      "room_id": "550e8400-e29b-41d4-a716-446655440000"
    },
    "request_id": "join_123456"
  }
}
```

### 系统消息

```json
{
  "type": "system",
  "id": "system_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "event": "maintenance",
    "message": "Scheduled maintenance in 30 minutes",
    "severity": "warning",
    "scheduled_at": "2024-01-15T11:00:00Z",
    "duration_minutes": 30
  }
}
```

## 🔄 消息流程示例

### 1. 用户加入房间流程

```json
// 客户端发送加入房间请求
{
  "type": "join_room",
  "id": "join_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}

// 服务器返回加入成功确认
{
  "type": "join_success",
  "id": "join_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "room_name": "General",
    "member_count": 42
  }
}

// 服务器广播用户加入通知给房间内其他用户
{
  "type": "user_joined",
  "id": "join_notify_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440002",
      "username": "john_doe",
      "avatar_url": "https://example.com/avatar.jpg",
      "joined_at": "2024-01-15T10:30:00Z"
    },
    "member_count": 43
  }
}
```

### 2. 发送消息流程

```json
// 客户端开始输入
{
  "type": "typing_start",
  "id": "typing_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}

// 服务器广播输入状态给房间内其他用户
{
  "type": "user_typing",
  "id": "typing_notify_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "user": {
      "id": "550e8400-e29b-41d4-a716-446655440002",
      "username": "john_doe",
      "avatar_url": "https://example.com/avatar.jpg"
    },
    "typing": true
  }
}

// 客户端发送消息
{
  "type": "message",
  "id": "msg_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "room_id": "550e8400-e29b-41d4-a716-446655440000",
    "content": "Hello, World!",
    "message_type": "text"
  }
}

// 服务器确认消息接收
{
  "type": "message_received",
  "id": "msg_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "message_id": "550e8400-e29b-41d4-a716-446655440001",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}

// 服务器广播消息给房间内所有用户
{
  "type": "message",
  "id": "msg_broadcast_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "message": {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "room_id": "550e8400-e29b-41d4-a716-446655440000",
      "user_id": "550e8400-e29b-41d4-a716-446655440002",
      "username": "john_doe",
      "avatar_url": "https://example.com/avatar.jpg",
      "content": "Hello, World!",
      "message_type": "text",
      "created_at": "2024-01-15T10:30:00Z"
    }
  }
}
```

### 3. 心跳保持流程

```json
// 客户端发送心跳
{
  "type": "ping",
  "id": "ping_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {}
}

// 服务器响应心跳
{
  "type": "pong",
  "id": "pong_123456",
  "timestamp": "2024-01-15T10:30:00Z",
  "data": {
    "server_time": "2024-01-15T10:30:00Z",
    "ping_id": "ping_123456"
  }
}
```

## 📋 消息类型定义

### Rust数据结构

```rust
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage<T> {
    pub r#type: MessageType,
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    #[serde(rename = "ping")]
    Ping,
    
    #[serde(rename = "pong")]
    Pong,
    
    #[serde(rename = "message")]
    Message,
    
    #[serde(rename = "join_room")]
    JoinRoom,
    
    #[serde(rename = "leave_room")]
    LeaveRoom,
    
    #[serde(rename = "user_joined")]
    UserJoined,
    
    #[serde(rename = "user_left")]
    UserLeft,
    
    #[serde(rename = "typing_start")]
    TypingStart,
    
    #[serde(rename = "typing_stop")]
    TypingStop,
    
    #[serde(rename = "user_typing")]
    UserTyping,
    
    #[serde(rename = "read_receipt")]
    ReadReceipt,
    
    #[serde(rename = "message_read")]
    MessageRead,
    
    #[serde(rename = "presence_update")]
    PresenceUpdate,
    
    #[serde(rename = "room_updated")]
    RoomUpdated,
    
    #[serde(rename = "error")]
    Error,
    
    #[serde(rename = "system")]
    System,
}

// 客户端消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    pub r#type: MessageType,
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub data: ClientMessageData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessageData {
    Ping(PingData),
    Message(MessageData),
    JoinRoom(JoinRoomData),
    LeaveRoom(LeaveRoomData),
    TypingStart(TypingStartData),
    TypingStop(TypingStopData),
    ReadReceipt(ReadReceiptData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingData {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageData {
    pub room_id: Uuid,
    pub content: String,
    pub message_type: String,
    pub reply_to_message_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRoomData {
    pub room_id: Uuid,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveRoomData {
    pub room_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingStartData {
    pub room_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingStopData {
    pub room_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadReceiptData {
    pub room_id: Uuid,
    pub message_id: Uuid,
    pub read_at: DateTime<Utc>,
}

// 服务器消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMessage {
    pub r#type: MessageType,
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub data: ServerMessageData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessageData {
    Pong(PongData),
    Message(MessageResponse),
    UserJoined(UserJoinedData),
    UserLeft(UserLeftData),
    UserTyping(UserTypingData),
    MessageRead(MessageReadData),
    PresenceUpdate(PresenceUpdateData),
    RoomUpdated(RoomUpdatedData),
    Error(ErrorData),
    System(SystemData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PongData {
    pub server_time: DateTime<Utc>,
    pub ping_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub message: ChatMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub content: String,
    pub message_type: String,
    pub reply_to_message_id: Option<Uuid>,
    pub reply_to_username: Option<String>,
    pub is_edited: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserJoinedData {
    pub room_id: Uuid,
    pub user: UserInfo,
    pub member_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLeftData {
    pub room_id: Uuid,
    pub user: UserInfo,
    pub member_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub joined_at: Option<DateTime<Utc>>,
    pub left_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTypingData {
    pub room_id: Uuid,
    pub user: UserInfo,
    pub typing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReadData {
    pub room_id: Uuid,
    pub message_id: Uuid,
    pub read_by: UserInfo,
    pub read_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceUpdateData {
    pub user_id: Uuid,
    pub status: String,
    pub last_seen: DateTime<Utc>,
    pub current_room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUpdatedData {
    pub room: RoomInfo,
    pub changes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomInfo {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub member_count: u32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemData {
    pub event: String,
    pub message: String,
    pub severity: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub duration_minutes: Option<u32>,
}
```

### 消息处理器

```rust
use async_trait::async_trait;
use tokio::sync::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait WebSocketMessageHandler: Send + Sync {
    async fn handle_message(&self, message: ClientMessage, user_id: Uuid) -> Result<Vec<ServerMessage>>;
    async fn handle_connection(&self, user_id: Uuid) -> Result<()>;
    async fn handle_disconnection(&self, user_id: Uuid) -> Result<()>;
}

pub struct ChatRoomMessageHandler {
    room_manager: Arc<dyn RoomManager>,
    user_manager: Arc<dyn UserManager>,
    message_broadcaster: Arc<dyn MessageBroadcaster>,
}

impl ChatRoomMessageHandler {
    pub fn new(
        room_manager: Arc<dyn RoomManager>,
        user_manager: Arc<dyn UserManager>,
        message_broadcaster: Arc<dyn MessageBroadcaster>,
    ) -> Self {
        Self {
            room_manager,
            user_manager,
            message_broadcaster,
        }
    }
}

#[async_trait]
impl WebSocketMessageHandler for ChatRoomMessageHandler {
    async fn handle_message(&self, message: ClientMessage, user_id: Uuid) -> Result<Vec<ServerMessage>> {
        match message.data {
            ClientMessageData::Ping(_) => {
                // 处理心跳
                let pong_response = ServerMessage {
                    r#type: MessageType::Pong,
                    id: format!("pong_{}", message.id),
                    timestamp: Utc::now(),
                    data: ServerMessageData::Pong(PongData {
                        server_time: Utc::now(),
                        ping_id: message.id,
                    }),
                };
                Ok(vec![pong_response])
            }
            
            ClientMessageData::Message(msg_data) => {
                // 处理消息发送
                let chat_message = ChatMessage {
                    id: Uuid::new_v4(),
                    room_id: msg_data.room_id,
                    user_id,
                    username: self.user_manager.get_username(user_id).await?,
                    avatar_url: self.user_manager.get_avatar_url(user_id).await?,
                    content: msg_data.content,
                    message_type: msg_data.message_type,
                    reply_to_message_id: msg_data.reply_to_message_id,
                    reply_to_username: None,
                    is_edited: false,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                
                // 保存消息到数据库
                self.room_manager.save_message(chat_message.clone()).await?;
                
                // 广播消息给房间内所有用户
                self.message_broadcaster.broadcast_to_room(
                    msg_data.room_id,
                    ServerMessage {
                        r#type: MessageType::Message,
                        id: format!("msg_{}", Uuid::new_v4()),
                        timestamp: Utc::now(),
                        data: ServerMessageData::Message(MessageResponse {
                            message: chat_message,
                        }),
                    },
                ).await?;
                
                // 返回确认消息
                let confirmation = ServerMessage {
                    r#type: MessageType::Message,
                    id: message.id,
                    timestamp: Utc::now(),
                    data: ServerMessageData::Message(MessageResponse {
                        message: chat_message,
                    }),
                };
                
                Ok(vec![confirmation])
            }
            
            ClientMessageData::JoinRoom(join_data) => {
                // 处理加入房间
                self.room_manager.add_user_to_room(user_id, join_data.room_id).await?;
                
                let user_info = UserInfo {
                    id: user_id,
                    username: self.user_manager.get_username(user_id).await?,
                    avatar_url: self.user_manager.get_avatar_url(user_id).await?,
                    joined_at: Some(Utc::now()),
                    left_at: None,
                };
                
                let member_count = self.room_manager.get_room_member_count(join_data.room_id).await?;
                
                // 广播用户加入通知
                self.message_broadcaster.broadcast_to_room(
                    join_data.room_id,
                    ServerMessage {
                        r#type: MessageType::UserJoined,
                        id: format!("join_notify_{}", Uuid::new_v4()),
                        timestamp: Utc::now(),
                        data: ServerMessageData::UserJoined(UserJoinedData {
                            room_id: join_data.room_id,
                            user: user_info.clone(),
                            member_count,
                        }),
                    },
                ).await?;
                
                // 返回加入成功确认
                let success_response = ServerMessage {
                    r#type: MessageType::JoinRoom,
                    id: message.id,
                    timestamp: Utc::now(),
                    data: ServerMessageData::UserJoined(UserJoinedData {
                        room_id: join_data.room_id,
                        user: user_info,
                        member_count,
                    }),
                };
                
                Ok(vec![success_response])
            }
            
            ClientMessageData::LeaveRoom(leave_data) => {
                // 处理离开房间
                self.room_manager.remove_user_from_room(user_id, leave_data.room_id).await?;
                
                let user_info = UserInfo {
                    id: user_id,
                    username: self.user_manager.get_username(user_id).await?,
                    avatar_url: self.user_manager.get_avatar_url(user_id).await?,
                    joined_at: None,
                    left_at: Some(Utc::now()),
                };
                
                let member_count = self.room_manager.get_room_member_count(leave_data.room_id).await?;
                
                // 广播用户离开通知
                self.message_broadcaster.broadcast_to_room(
                    leave_data.room_id,
                    ServerMessage {
                        r#type: MessageType::UserLeft,
                        id: format!("leave_notify_{}", Uuid::new_v4()),
                        timestamp: Utc::now(),
                        data: ServerMessageData::UserLeft(UserLeftData {
                            room_id: leave_data.room_id,
                            user: user_info,
                            member_count,
                        }),
                    },
                ).await?;
                
                // 返回离开成功确认
                let success_response = ServerMessage {
                    r#type: MessageType::LeaveRoom,
                    id: message.id,
                    timestamp: Utc::now(),
                    data: ServerMessageData::UserLeft(UserLeftData {
                        room_id: leave_data.room_id,
                        user: user_info,
                        member_count,
                    }),
                };
                
                Ok(vec![success_response])
            }
            
            ClientMessageData::TypingStart(typing_data) => {
                // 处理开始输入
                let user_info = UserInfo {
                    id: user_id,
                    username: self.user_manager.get_username(user_id).await?,
                    avatar_url: self.user_manager.get_avatar_url(user_id).await?,
                    joined_at: None,
                    left_at: None,
                };
                
                // 广播输入状态给房间内其他用户
                self.message_broadcaster.broadcast_to_room(
                    typing_data.room_id,
                    ServerMessage {
                        r#type: MessageType::UserTyping,
                        id: format!("typing_notify_{}", Uuid::new_v4()),
                        timestamp: Utc::now(),
                        data: ServerMessageData::UserTyping(UserTypingData {
                            room_id: typing_data.room_id,
                            user: user_info,
                            typing: true,
                        }),
                    },
                ).await?;
                
                Ok(vec![])
            }
            
            ClientMessageData::TypingStop(typing_data) => {
                // 处理停止输入
                let user_info = UserInfo {
                    id: user_id,
                    username: self.user_manager.get_username(user_id).await?,
                    avatar_url: self.user_manager.get_avatar_url(user_id).await?,
                    joined_at: None,
                    left_at: None,
                };
                
                // 广播停止输入状态给房间内其他用户
                self.message_broadcaster.broadcast_to_room(
                    typing_data.room_id,
                    ServerMessage {
                        r#type: MessageType::UserTyping,
                        id: format!("typing_notify_{}", Uuid::new_v4()),
                        timestamp: Utc::now(),
                        data: ServerMessageData::UserTyping(UserTypingData {
                            room_id: typing_data.room_id,
                            user: user_info,
                            typing: false,
                        }),
                    },
                ).await?;
                
                Ok(vec![])
            }
            
            ClientMessageData::ReadReceipt(receipt_data) => {
                // 处理消息已读回执
                self.room_manager.mark_message_as_read(
                    user_id,
                    receipt_data.room_id,
                    receipt_data.message_id,
                    receipt_data.read_at,
                ).await?;
                
                let user_info = UserInfo {
                    id: user_id,
                    username: self.user_manager.get_username(user_id).await?,
                    avatar_url: self.user_manager.get_avatar_url(user_id).await?,
                    joined_at: None,
                    left_at: None,
                };
                
                // 广播消息已读通知
                self.message_broadcaster.broadcast_to_room(
                    receipt_data.room_id,
                    ServerMessage {
                        r#type: MessageType::MessageRead,
                        id: format!("read_notify_{}", Uuid::new_v4()),
                        timestamp: Utc::now(),
                        data: ServerMessageData::MessageRead(MessageReadData {
                            room_id: receipt_data.room_id,
                            message_id: receipt_data.message_id,
                            read_by: user_info,
                            read_at: receipt_data.read_at,
                        }),
                    },
                ).await?;
                
                Ok(vec![])
            }
        }
    }
    
    async fn handle_connection(&self, user_id: Uuid) -> Result<()> {
        // 处理新连接
        self.user_manager.update_user_status(user_id, "online").await?;
        
        // 通知用户已上线
        self.message_broadcaster.broadcast_user_status_update(
            user_id,
            "online",
            Utc::now(),
        ).await?;
        
        Ok(())
    }
    
    async fn handle_disconnection(&self, user_id: Uuid) -> Result<()> {
        // 处理连接断开
        self.user_manager.update_user_status(user_id, "offline").await?;
        
        // 通知用户已离线
        self.message_broadcaster.broadcast_user_status_update(
            user_id,
            "offline",
            Utc::now(),
        ).await?;
        
        Ok(())
    }
}
```

## 🛡️ 安全考虑

### 消息验证

```rust
pub fn validate_client_message(message: &ClientMessage) -> Result<(), ValidationError> {
    // 验证消息ID
    if message.id.is_empty() {
        return Err(ValidationError::RequiredField("message_id".to_string()));
    }
    
    // 验证时间戳
    let now = Utc::now();
    let time_diff = now.signed_duration_since(message.timestamp);
    if time_diff.num_seconds().abs() > 300 { // 5分钟
        return Err(ValidationError::InvalidTimestamp(
            "Message timestamp is too old or in the future".to_string()
        ));
    }
    
    // 验证消息数据
    match &message.data {
        ClientMessageData::Message(msg_data) => {
            validate_message_data(msg_data)?;
        }
        ClientMessageData::JoinRoom(join_data) => {
            validate_join_room_data(join_data)?;
        }
        ClientMessageData::LeaveRoom(leave_data) => {
            validate_leave_room_data(leave_data)?;
        }
        ClientMessageData::TypingStart(typing_data) => {
            validate_typing_data(typing_data)?;
        }
        ClientMessageData::TypingStop(typing_data) => {
            validate_typing_data(typing_data)?;
        }
        ClientMessageData::ReadReceipt(receipt_data) => {
            validate_read_receipt_data(receipt_data)?;
        }
        ClientMessageData::Ping(_) => {
            // Ping消息无需额外验证
        }
    }
    
    Ok(())
}

pub fn validate_message_data(data: &MessageData) -> Result<(), ValidationError> {
    // 验证房间ID
    if data.room_id == Uuid::nil() {
        return Err(ValidationError::InvalidUuid("Invalid room ID".to_string()));
    }
    
    // 验证消息内容
    if data.content.is_empty() {
        return Err(ValidationError::RequiredField("content".to_string()));
    }
    
    if data.content.len() > 10000 {
        return Err(ValidationError::StringTooLong(
            "Message content too long".to_string(),
            10000,
        ));
    }
    
    // 验证消息类型
    let valid_types = vec!["text", "image", "file", "system"];
    if !valid_types.contains(&data.message_type.as_str()) {
        return Err(ValidationError::InvalidEnumValue(
            format!("Invalid message type: {}", data.message_type)
        ));
    }
    
    // 验证回复消息ID
    if let Some(reply_id) = data.reply_to_message_id {
        if reply_id == Uuid::nil() {
            return Err(ValidationError::InvalidUuid(
                "Invalid reply_to_message_id".to_string()
            ));
        }
    }
    
    Ok(())
}

pub fn validate_join_room_data(data: &JoinRoomData) -> Result<(), ValidationError> {
    // 验证房间ID
    if data.room_id == Uuid::nil() {
        return Err(ValidationError::InvalidUuid("Invalid room ID".to_string()));
    }
    
    // 验证密码长度
    if let Some(password) = &data.password {
        if password.len() > 100 {
            return Err(ValidationError::StringTooLong(
                "Password too long".to_string(),
                100,
            ));
        }
    }
    
    Ok(())
}
```

### 速率限制

```rust
pub struct WebSocketRateLimiter {
    limits: Arc<RwLock<HashMap<Uuid, RateLimitInfo>>>,
    max_messages_per_minute: u32,
}

#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub message_count: u32,
    pub reset_time: DateTime<Utc>,
}

impl WebSocketRateLimiter {
    pub fn new(max_messages_per_minute: u32) -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
            max_messages_per_minute,
        }
    }
    
    pub async fn check_rate_limit(&self, user_id: Uuid) -> Result<(), RateLimitError> {
        let mut limits = self.limits.write().await;
        let now = Utc::now();
        
        let info = limits.entry(user_id).or_insert(RateLimitInfo {
            message_count: 0,
            reset_time: now + chrono::Duration::minutes(1),
        });
        
        // 重置计数器
        if now > info.reset_time {
            info.message_count = 0;
            info.reset_time = now + chrono::Duration::minutes(1);
        }
        
        // 检查限制
        if info.message_count >= self.max_messages_per_minute {
            return Err(RateLimitError::RateLimitExceeded {
                limit: self.max_messages_per_minute,
                reset_time: info.reset_time,
            });
        }
        
        info.message_count += 1;
        Ok(())
    }
    
    pub async fn cleanup_expired_limits(&self) {
        let mut limits = self.limits.write().await;
        let now = Utc::now();
        
        limits.retain(|_, info| info.reset_time > now);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded: {limit} messages per minute, reset at {reset_time}")]
    RateLimitExceeded {
        limit: u32,
        reset_time: DateTime<Utc>,
    },
}
```

## 📊 性能优化

### 消息压缩

```rust
use flate2::{Compression, GzBuilder};
use flate2::read::GzDecoder;
use std::io::{self, Read, Write};

pub fn compress_message(message: &str) -> Result<Vec<u8>, CompressionError> {
    let mut encoder = GzBuilder::new()
        .compression(Compression::default())
        .write(Vec::new(), message.as_bytes())?;
    
    encoder.finish()
        .map_err(CompressionError::Io)
}

pub fn decompress_message(compressed: &[u8]) -> Result<String, CompressionError> {
    let mut decoder = GzDecoder::new(compressed);
    let mut decompressed = String::new();
    
    decoder.read_to_string(&mut decompressed)
        .map_err(CompressionError::Io)?;
    
    Ok(decompressed)
}

#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Compression error: {0}")]
    Compression(String),
}
```

### 消息批处理

```rust
pub struct MessageBatcher {
    batch_size: usize,
    batch_timeout: Duration,
    pending_messages: Arc<RwLock<Vec<ServerMessage>>>,
    sender: tokio::sync::mpsc::UnboundedSender<ServerMessage>,
}

impl MessageBatcher {
    pub fn new(batch_size: usize, batch_timeout: Duration) -> (Self, tokio::sync::mpsc::UnboundedReceiver<ServerMessage>) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        
        let batcher = Self {
            batch_size,
            batch_timeout,
            pending_messages: Arc::new(RwLock::new(Vec::new())),
            sender,
        };
        
        (batcher, receiver)
    }
    
    pub async fn add_message(&self, message: ServerMessage) -> Result<(), MessageBatchError> {
        self.sender.send(message)?;
        Ok(())
    }
    
    pub async fn start_batching(&self) -> Result<(), MessageBatchError> {
        let mut receiver = self.sender.subscribe();
        let pending_messages = self.pending_messages.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.batch_timeout);
            
            loop {
                tokio::select! {
                    // 接收新消息
                    message = receiver.recv() => {
                        match message {
                            Ok(message) => {
                                let mut messages = pending_messages.write().await;
                                messages.push(message);
                                
                                // 如果达到批次大小，立即发送
                                if messages.len() >= self.batch_size {
                                    if let Err(e) = self.send_batch(&mut messages).await {
                                        tracing::error!("Failed to send message batch: {:?}", e);
                                    }
                                }
                            }
                            Err(_) => {
                                // 通道关闭，发送剩余消息
                                let mut messages = pending_messages.write().await;
                                if !messages.is_empty() {
                                    if let Err(e) = self.send_batch(&mut messages).await {
                                        tracing::error!("Failed to send final message batch: {:?}", e);
                                    }
                                }
                                break;
                            }
                        }
                    }
                    
                    // 定时检查
                    _ = interval.tick() => {
                        let mut messages = pending_messages.write().await;
                        if !messages.is_empty() {
                            if let Err(e) = self.send_batch(&mut messages).await {
                                tracing::error!("Failed to send timed message batch: {:?}", e);
                            }
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    async fn send_batch(&self, messages: &mut Vec<ServerMessage>) -> Result<(), MessageBatchError> {
        if messages.is_empty() {
            return Ok(());
        }
        
        // 发送批次消息
        let batch = MessageBatch {
            messages: messages.clone(),
            batch_id: Uuid::new_v4(),
            timestamp: Utc::now(),
        };
        
        // 这里应该实现实际的发送逻辑
        tracing::info!("Sending message batch with {} messages", messages.len());
        
        messages.clear();
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageBatch {
    pub messages: Vec<ServerMessage>,
    pub batch_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum MessageBatchError {
    #[error("Channel error: {0}")]
    Channel(#[from] tokio::sync::mpsc::error::SendError<ServerMessage>),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Network error: {0}")]
    Network(String),
}
```

---

**文档完成**: 这是聊天室后台系统的完整设计文档。所有主要模块都已按照要求拆分并放置在docs目录中，便于查阅和维护。
