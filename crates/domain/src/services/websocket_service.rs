//! WebSocket服务接口
//!
//! 定义WebSocket连接管理、消息路由等核心接口。

use async_trait::async_trait;
use uuid::Uuid;

use crate::entities::websocket::*;

/// WebSocket连接管理器接口
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    /// 注册新连接
    async fn register_connection(&self, connection: ConnectionInfo) -> Result<(), WebSocketError>;

    /// 注销连接
    async fn unregister_connection(&self, connection_id: Uuid) -> Result<(), WebSocketError>;

    /// 获取连接信息
    async fn get_connection(&self, connection_id: Uuid) -> Option<ConnectionInfo>;

    /// 获取用户的所有连接
    async fn get_user_connections(&self, user_id: Uuid) -> Vec<ConnectionInfo>;

    /// 获取房间中的所有连接
    async fn get_room_connections(&self, room_id: &str) -> Vec<ConnectionInfo>;

    /// 更新连接状态
    async fn update_connection_status(
        &self,
        connection_id: Uuid,
        status: ConnectionStatus,
    ) -> Result<(), WebSocketError>;

    /// 更新连接活跃时间
    async fn update_connection_activity(&self, connection_id: Uuid) -> Result<(), WebSocketError>;

    /// 连接加入房间
    async fn join_room(&self, connection_id: Uuid, room_id: String) -> Result<(), WebSocketError>;

    /// 连接离开房间
    async fn leave_room(&self, connection_id: Uuid, room_id: &str) -> Result<(), WebSocketError>;

    /// 检查连接是否在房间中
    async fn is_connection_in_room(&self, connection_id: Uuid, room_id: &str) -> bool;

    /// 获取连接统计信息
    async fn get_stats(&self) -> ConnectionStats;

    /// 清理不活跃的连接
    async fn cleanup_inactive_connections(
        &self,
        timeout_seconds: u64,
    ) -> Result<usize, WebSocketError>;
}

/// WebSocket消息路由器接口
#[async_trait]
pub trait MessageRouter: Send + Sync {
    /// 路由消息到指定连接
    async fn route_to_connection(
        &self,
        connection_id: Uuid,
        message: WebSocketFrame,
    ) -> Result<(), WebSocketError>;

    /// 路由消息到指定用户的所有连接
    async fn route_to_user(
        &self,
        user_id: Uuid,
        message: WebSocketFrame,
    ) -> Result<(), WebSocketError>;

    /// 路由消息到指定房间的所有连接
    async fn route_to_room(
        &self,
        room_id: &str,
        message: WebSocketFrame,
    ) -> Result<(), WebSocketError>;

    /// 广播消息到所有连接
    async fn broadcast(&self, message: WebSocketFrame) -> Result<(), WebSocketError>;

    /// 路由消息到多个连接
    async fn route_to_connections(
        &self,
        connection_ids: &[Uuid],
        message: WebSocketFrame,
    ) -> Result<(), WebSocketError>;

    /// 获取路由统计信息
    async fn get_stats(&self) -> RouterStats;
}

/// WebSocket消息处理器接口
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// 处理客户端消息
    async fn handle_client_message(
        &self,
        connection_id: Uuid,
        message: ClientMessage,
    ) -> Result<ServerMessage, WebSocketError>;

    /// 处理连接认证
    async fn handle_authentication(
        &self,
        connection_id: Uuid,
        token: &str,
    ) -> Result<AuthenticationResult, WebSocketError>;

    /// 处理连接关闭
    async fn handle_connection_close(&self, connection_id: Uuid) -> Result<(), WebSocketError>;

    /// 验证消息权限
    async fn validate_message_permission(
        &self,
        connection_id: Uuid,
        message: &ClientMessage,
    ) -> Result<bool, WebSocketError>;
}

/// WebSocket房间管理器接口
#[async_trait]
pub trait RoomManager: Send + Sync {
    /// 创建房间
    async fn create_room(
        &self,
        room_id: String,
        name: String,
        owner_id: Uuid,
        password: Option<String>,
    ) -> Result<RoomInfo, WebSocketError>;

    /// 删除房间
    async fn delete_room(&self, room_id: &str) -> Result<(), WebSocketError>;

    /// 加入房间
    async fn join_room(
        &self,
        room_id: &str,
        user_id: Uuid,
        password: Option<String>,
    ) -> Result<RoomInfo, WebSocketError>;

    /// 离开房间
    async fn leave_room(&self, room_id: &str, user_id: Uuid) -> Result<(), WebSocketError>;

    /// 获取房间信息
    async fn get_room(&self, room_id: &str) -> Option<RoomInfo>;

    /// 获取用户所在的房间列表
    async fn get_user_rooms(&self, user_id: Uuid) -> Vec<RoomInfo>;

    /// 获取房间中的用户列表
    async fn get_room_users(&self, room_id: &str) -> Vec<UserInfo>;

    /// 更新房间信息
    async fn update_room(
        &self,
        room_id: &str,
        updates: RoomUpdate,
    ) -> Result<RoomInfo, WebSocketError>;

    /// 检查房间是否存在
    async fn room_exists(&self, room_id: &str) -> bool;

    /// 检查用户是否在房间中
    async fn is_user_in_room(&self, room_id: &str, user_id: Uuid) -> bool;

    /// 获取房间统计信息
    async fn get_room_stats(&self, room_id: &str) -> Option<RoomStats>;
}

/// 房间信息
#[derive(Debug, Clone)]
pub struct RoomInfo {
    /// 房间ID
    pub room_id: String,
    /// 房间名称
    pub name: String,
    /// 房间描述
    pub description: Option<String>,
    /// 房间所有者ID
    pub owner_id: Uuid,
    /// 房间密码（可选）
    pub password: Option<String>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最后活动时间
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// 最大用户数
    pub max_users: Option<u32>,
    /// 当前用户数
    pub current_users: u32,
    /// 房间状态
    pub status: RoomStatus,
}

/// 房间状态
#[derive(Debug, Clone, PartialEq)]
pub enum RoomStatus {
    /// 活跃
    Active,
    /// 已锁定
    Locked,
    /// 已归档
    Archived,
    /// 已删除
    Deleted,
}

/// 房间更新信息
#[derive(Debug, Clone)]
pub struct RoomUpdate {
    /// 房间名称
    pub name: Option<String>,
    /// 房间描述
    pub description: Option<String>,
    /// 房间密码
    pub password: Option<Option<String>>,
    /// 最大用户数
    pub max_users: Option<u32>,
    /// 房间状态
    pub status: Option<RoomStatus>,
}

/// 用户信息（用于房间管理）
#[derive(Debug, Clone)]
pub struct UserInfo {
    /// 用户ID
    pub user_id: Uuid,
    /// 用户名
    pub username: String,
    /// 用户角色
    pub role: String,
    /// 加入时间
    pub joined_at: chrono::DateTime<chrono::Utc>,
    /// 最后活跃时间
    pub last_active: chrono::DateTime<chrono::Utc>,
}

/// 房间统计信息
#[derive(Debug, Clone)]
pub struct RoomStats {
    /// 房间ID
    pub room_id: String,
    /// 消息总数
    pub total_messages: u64,
    /// 活跃用户数
    pub active_users: u32,
    /// 今日消息数
    pub messages_today: u64,
    /// 平均每日消息数
    pub avg_daily_messages: f64,
}

/// 连接统计信息
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    /// 总连接数
    pub total_connections: usize,
    /// 活跃连接数
    pub active_connections: usize,
    /// 认证连接数
    pub authenticated_connections: usize,
    /// 房间连接数
    pub room_connections: usize,
    /// 今日连接数
    pub connections_today: usize,
    /// 峰值连接数
    pub peak_connections: usize,
}

/// 路由统计信息
#[derive(Debug, Clone)]
pub struct RouterStats {
    /// 消息总数
    pub total_messages: u64,
    /// 成功路由消息数
    pub successful_routes: u64,
    /// 失败路由消息数
    pub failed_routes: u64,
    /// 平均路由时间（毫秒）
    pub avg_route_time_ms: f64,
    /// 当前待处理消息数
    pub pending_messages: usize,
}

/// 认证结果
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// 是否成功
    pub success: bool,
    /// 用户ID
    pub user_id: Option<Uuid>,
    /// 用户名
    pub username: Option<String>,
    /// 错误消息
    pub error_message: Option<String>,
}

impl AuthenticationResult {
    /// 创建成功结果
    pub fn success(user_id: Uuid, username: String) -> Self {
        Self {
            success: true,
            user_id: Some(user_id),
            username: Some(username),
            error_message: None,
        }
    }

    /// 创建失败结果
    pub fn failure(error_message: String) -> Self {
        Self {
            success: false,
            user_id: None,
            username: None,
            error_message: Some(error_message),
        }
    }
}

impl RoomInfo {
    /// 创建新房间
    pub fn new(room_id: String, name: String, owner_id: Uuid, password: Option<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            room_id,
            name,
            description: None,
            owner_id,
            password,
            created_at: now,
            last_activity: now,
            max_users: None,
            current_users: 0,
            status: RoomStatus::Active,
        }
    }

    /// 检查房间是否已满
    pub fn is_full(&self) -> bool {
        if let Some(max) = self.max_users {
            self.current_users >= max
        } else {
            false
        }
    }

    /// 检查用户是否可以加入房间
    pub fn can_join(&self, _user_id: Uuid, password: Option<String>) -> bool {
        if self.status != RoomStatus::Active {
            return false;
        }

        if self.is_full() {
            return false;
        }

        // 检查密码
        if let Some(room_password) = &self.password {
            if password.as_ref() != Some(room_password) {
                return false;
            }
        }

        true
    }

    /// 更新最后活动时间
    pub fn update_activity(&mut self) {
        self.last_activity = chrono::Utc::now();
    }

    /// 增加用户数
    pub fn increment_users(&mut self) -> Result<(), WebSocketError> {
        if self.is_full() {
            return Err(WebSocketError::RoomFull(self.room_id.clone()));
        }
        self.current_users += 1;
        Ok(())
    }

    /// 减少用户数
    pub fn decrement_users(&mut self) {
        if self.current_users > 0 {
            self.current_users -= 1;
        }
    }
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            total_connections: 0,
            active_connections: 0,
            authenticated_connections: 0,
            room_connections: 0,
            connections_today: 0,
            peak_connections: 0,
        }
    }
}

impl Default for RouterStats {
    fn default() -> Self {
        Self {
            total_messages: 0,
            successful_routes: 0,
            failed_routes: 0,
            avg_route_time_ms: 0.0,
            pending_messages: 0,
        }
    }
}

impl RoomStats {
    /// 创建新的房间统计
    pub fn new(room_id: String) -> Self {
        Self {
            room_id,
            total_messages: 0,
            active_users: 0,
            messages_today: 0,
            avg_daily_messages: 0.0,
        }
    }

    /// 添加消息
    pub fn add_message(&mut self) {
        self.total_messages += 1;
        self.messages_today += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authentication_result() {
        let user_id = Uuid::new_v4();
        let success_result = AuthenticationResult::success(user_id, "testuser".to_string());

        assert!(success_result.success);
        assert_eq!(success_result.user_id, Some(user_id));
        assert_eq!(success_result.username, Some("testuser".to_string()));
        assert!(success_result.error_message.is_none());

        let failure_result = AuthenticationResult::failure("Invalid token".to_string());

        assert!(!failure_result.success);
        assert!(failure_result.user_id.is_none());
        assert!(failure_result.username.is_none());
        assert_eq!(
            failure_result.error_message,
            Some("Invalid token".to_string())
        );
    }

    #[test]
    fn test_room_info() {
        let owner_id = Uuid::new_v4();
        let mut room = RoomInfo::new(
            "test_room".to_string(),
            "Test Room".to_string(),
            owner_id,
            Some("password123".to_string()),
        );

        assert!(!room.is_full());
        assert!(room.can_join(owner_id, Some("password123".to_string())));
        assert!(!room.can_join(owner_id, Some("wrong_password".to_string())));

        room.max_users = Some(1);
        assert!(room.can_join(owner_id, Some("password123".to_string())));

        // 加入用户后应该满了
        room.increment_users().unwrap();
        assert!(room.is_full());
        assert!(!room.can_join(Uuid::new_v4(), Some("password123".to_string())));
    }

    #[test]
    fn test_connection_stats() {
        let mut stats = ConnectionStats::default();

        assert_eq!(stats.total_connections, 0);
        assert_eq!(stats.active_connections, 0);

        stats.total_connections = 10;
        stats.active_connections = 5;

        assert_eq!(stats.total_connections, 10);
        assert_eq!(stats.active_connections, 5);
    }

    #[test]
    fn test_room_stats() {
        let mut stats = RoomStats::new("test_room".to_string());

        assert_eq!(stats.total_messages, 0);
        assert_eq!(stats.messages_today, 0);

        stats.add_message();

        assert_eq!(stats.total_messages, 1);
        assert_eq!(stats.messages_today, 1);
    }
}
