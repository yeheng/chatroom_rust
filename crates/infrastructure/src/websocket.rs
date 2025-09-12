//! WebSocket处理器基础设施实现
//!
//! 实现WebSocket连接管理、消息路由和房间管理功能。

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::entities::websocket::*;
use domain::services::websocket_service::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// 内存中的连接管理器
pub struct InMemoryConnectionManager {
    /// 连接存储
    connections: Arc<RwLock<HashMap<Uuid, ConnectionInfo>>>,
    /// 用户到连接的映射
    user_connections: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    /// 房间到连接的映射
    room_connections: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
    /// 统计信息
    stats: Arc<RwLock<ConnectionStats>>,
}

impl InMemoryConnectionManager {
    /// 创建新的内存连接管理器
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            user_connections: Arc::new(RwLock::new(HashMap::new())),
            room_connections: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
        }
    }

    /// 更新统计信息
    async fn update_stats(&self, f: impl FnOnce(&mut ConnectionStats)) {
        let mut stats = self.stats.write().await;
        f(&mut stats);
    }

    /// 获取当前时间戳
    fn now() -> DateTime<Utc> {
        Utc::now()
    }
}

#[async_trait]
impl ConnectionManager for InMemoryConnectionManager {
    async fn register_connection(&self, connection: ConnectionInfo) -> Result<(), WebSocketError> {
        let connection_id = connection.connection_id;
        let user_id = connection.user_id;

        // 检查是否超过最大连接数限制
        let connections = self.connections.read().await;
        if connections.len() >= 1000 {
            return Err(WebSocketError::InternalError(
                "Maximum connections reached".to_string(),
            ));
        }
        drop(connections);

        // 注册连接
        {
            let mut connections = self.connections.write().await;
            connections.insert(connection_id, connection);
        }

        // 更新用户连接映射
        {
            let mut user_connections = self.user_connections.write().await;
            user_connections
                .entry(user_id)
                .or_insert_with(Vec::new)
                .push(connection_id);
        }

        // 更新统计信息
        self.update_stats(|stats| {
            stats.total_connections += 1;
            stats.active_connections += 1;
            stats.connections_today += 1;
            stats.peak_connections = stats.peak_connections.max(stats.active_connections);
        })
        .await;

        info!(
            "Connection {} registered for user {}",
            connection_id, user_id
        );
        Ok(())
    }

    async fn unregister_connection(&self, connection_id: Uuid) -> Result<(), WebSocketError> {
        let connection = {
            let mut connections = self.connections.write().await;
            connections.remove(&connection_id).ok_or_else(|| {
                WebSocketError::ConnectionClosed("Connection not found".to_string())
            })?
        };

        // 从用户连接映射中移除
        {
            let mut user_connections = self.user_connections.write().await;
            if let Some(connections) = user_connections.get_mut(&connection.user_id) {
                connections.retain(|&id| id != connection_id);
                if connections.is_empty() {
                    user_connections.remove(&connection.user_id);
                }
            }
        }

        // 从房间连接映射中移除
        {
            let mut room_connections = self.room_connections.write().await;
            for room_connections_list in room_connections.values_mut() {
                room_connections_list.retain(|&id| id != connection_id);
            }
        }

        // 更新统计信息
        self.update_stats(|stats| {
            stats.active_connections = stats.active_connections.saturating_sub(1);
            if connection.status == ConnectionStatus::Authenticated {
                stats.authenticated_connections = stats.authenticated_connections.saturating_sub(1);
            }
            stats.room_connections = stats
                .room_connections
                .saturating_sub(connection.room_count() as usize);
        })
        .await;

        info!(
            "Connection {} unregistered for user {}",
            connection_id, connection.user_id
        );
        Ok(())
    }

    async fn get_connection(&self, connection_id: Uuid) -> Option<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections.get(&connection_id).cloned()
    }

    async fn get_user_connections(&self, user_id: Uuid) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        let user_connections = self.user_connections.read().await;

        if let Some(connection_ids) = user_connections.get(&user_id) {
            connection_ids
                .iter()
                .filter_map(|&id| connections.get(&id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    async fn get_room_connections(&self, room_id: &str) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        let room_connections = self.room_connections.read().await;

        if let Some(connection_ids) = room_connections.get(room_id) {
            connection_ids
                .iter()
                .filter_map(|&id| connections.get(&id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    async fn update_connection_status(
        &self,
        connection_id: Uuid,
        status: ConnectionStatus,
    ) -> Result<(), WebSocketError> {
        let mut connections = self.connections.write().await;
        let connection = connections
            .get_mut(&connection_id)
            .ok_or_else(|| WebSocketError::ConnectionClosed("Connection not found".to_string()))?;

        let is_authenticated = connection.status == ConnectionStatus::Authenticated;
        let is_becoming_authenticated = status == ConnectionStatus::Authenticated;
        connection.status = status;

        // 更新统计信息
        if !is_authenticated && is_becoming_authenticated {
            let mut stats = self.stats.write().await;
            stats.authenticated_connections += 1;
        } else if is_authenticated && !is_becoming_authenticated {
            let mut stats = self.stats.write().await;
            stats.authenticated_connections = stats.authenticated_connections.saturating_sub(1);
        }

        debug!(
            "Connection {} status changed to {:?}",
            connection_id, connection.status
        );
        Ok(())
    }

    async fn update_connection_activity(&self, connection_id: Uuid) -> Result<(), WebSocketError> {
        let mut connections = self.connections.write().await;
        let connection = connections
            .get_mut(&connection_id)
            .ok_or_else(|| WebSocketError::ConnectionClosed("Connection not found".to_string()))?;

        connection.update_activity();
        debug!("Connection {} activity updated", connection_id);
        Ok(())
    }

    async fn join_room(&self, connection_id: Uuid, room_id: String) -> Result<(), WebSocketError> {
        let mut connections = self.connections.write().await;
        let connection = connections
            .get_mut(&connection_id)
            .ok_or_else(|| WebSocketError::ConnectionClosed("Connection not found".to_string()))?;

        connection.join_room(room_id.clone());
        drop(connections);

        // 更新房间连接映射
        {
            let mut room_connections = self.room_connections.write().await;
            room_connections
                .entry(room_id.clone())
                .or_insert_with(Vec::new)
                .push(connection_id);
        }

        // 更新统计信息
        self.update_stats(|stats| {
            stats.room_connections += 1;
        })
        .await;

        info!("Connection {} joined room {}", connection_id, room_id);
        Ok(())
    }

    async fn leave_room(&self, connection_id: Uuid, room_id: &str) -> Result<(), WebSocketError> {
        let mut connections = self.connections.write().await;
        let connection = connections
            .get_mut(&connection_id)
            .ok_or_else(|| WebSocketError::ConnectionClosed("Connection not found".to_string()))?;

        connection.leave_room(room_id);
        drop(connections);

        // 从房间连接映射中移除
        {
            let mut room_connections = self.room_connections.write().await;
            if let Some(connections) = room_connections.get_mut(room_id) {
                connections.retain(|&id| id != connection_id);
                if connections.is_empty() {
                    room_connections.remove(room_id);
                }
            }
        }

        // 更新统计信息
        self.update_stats(|stats| {
            stats.room_connections = stats.room_connections.saturating_sub(1);
        })
        .await;

        info!("Connection {} left room {}", connection_id, room_id);
        Ok(())
    }

    async fn is_connection_in_room(&self, connection_id: Uuid, room_id: &str) -> bool {
        let connections = self.connections.read().await;
        if let Some(connection) = connections.get(&connection_id) {
            connection.is_in_room(room_id)
        } else {
            false
        }
    }

    async fn get_stats(&self) -> ConnectionStats {
        self.stats.read().await.clone()
    }

    async fn cleanup_inactive_connections(
        &self,
        timeout_seconds: u64,
    ) -> Result<usize, WebSocketError> {
        let now = Self::now();
        let timeout = chrono::Duration::seconds(timeout_seconds as i64);

        let inactive_connections: Vec<Uuid> = {
            let connections = self.connections.read().await;
            connections
                .iter()
                .filter(|(_, conn)| now.signed_duration_since(conn.last_active) > timeout)
                .map(|(&id, _)| id)
                .collect()
        };

        let count = inactive_connections.len();

        for connection_id in inactive_connections {
            if let Err(e) = self.unregister_connection(connection_id).await {
                warn!(
                    "Failed to cleanup inactive connection {}: {}",
                    connection_id, e
                );
            }
        }

        if count > 0 {
            info!("Cleaned up {} inactive connections", count);
        }

        Ok(count)
    }
}

/// 内存中的消息路由器
pub struct InMemoryMessageRouter {
    /// 连接发送器映射
    connection_senders: Arc<RwLock<HashMap<Uuid, mpsc::UnboundedSender<WebSocketFrame>>>>,
    /// 统计信息
    stats: Arc<RwLock<RouterStats>>,
}

impl InMemoryMessageRouter {
    /// 创建新的内存消息路由器
    pub fn new() -> Self {
        Self {
            connection_senders: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RouterStats::default())),
        }
    }

    /// 注册连接发送器
    pub async fn register_sender(
        &self,
        connection_id: Uuid,
        sender: mpsc::UnboundedSender<WebSocketFrame>,
    ) {
        let mut senders = self.connection_senders.write().await;
        senders.insert(connection_id, sender);
    }

    /// 注销连接发送器
    pub async fn unregister_sender(&self, connection_id: Uuid) {
        let mut senders = self.connection_senders.write().await;
        senders.remove(&connection_id);
    }

    /// 发送消息到连接
    async fn send_to_connection(
        &self,
        connection_id: Uuid,
        message: WebSocketFrame,
    ) -> Result<(), WebSocketError> {
        let senders = self.connection_senders.read().await;
        let sender = senders
            .get(&connection_id)
            .ok_or_else(|| WebSocketError::ConnectionClosed("Sender not found".to_string()))?;

        sender
            .send(message)
            .map_err(|_| WebSocketError::SendFailed("Failed to send message".to_string()))?;

        Ok(())
    }

    /// 更新统计信息
    async fn update_stats(&self, f: impl FnOnce(&mut RouterStats)) {
        let mut stats = self.stats.write().await;
        f(&mut stats);
    }
}

#[async_trait]
impl MessageRouter for InMemoryMessageRouter {
    async fn route_to_connection(
        &self,
        connection_id: Uuid,
        message: WebSocketFrame,
    ) -> Result<(), WebSocketError> {
        let start_time = std::time::Instant::now();

        match self.send_to_connection(connection_id, message).await {
            Ok(_) => {
                let duration = start_time.elapsed();
                self.update_stats(|stats| {
                    stats.total_messages += 1;
                    stats.successful_routes += 1;
                    stats.avg_route_time_ms = (stats.avg_route_time_ms
                        * (stats.total_messages - 1) as f64
                        + duration.as_millis() as f64)
                        / stats.total_messages as f64;
                })
                .await;
                debug!("Message routed to connection {}", connection_id);
                Ok(())
            }
            Err(e) => {
                self.update_stats(|stats| {
                    stats.total_messages += 1;
                    stats.failed_routes += 1;
                })
                .await;
                error!(
                    "Failed to route message to connection {}: {}",
                    connection_id, e
                );
                Err(e)
            }
        }
    }

    async fn route_to_user(
        &self,
        _user_id: Uuid,
        _message: WebSocketFrame,
    ) -> Result<(), WebSocketError> {
        // 这个实现需要ConnectionManager的支持
        // 实际使用时需要注入ConnectionManager依赖
        Ok(())
    }

    async fn route_to_room(
        &self,
        _room_id: &str,
        _message: WebSocketFrame,
    ) -> Result<(), WebSocketError> {
        // 这个实现需要ConnectionManager的支持
        // 实际使用时需要注入ConnectionManager依赖
        Ok(())
    }

    async fn broadcast(&self, message: WebSocketFrame) -> Result<(), WebSocketError> {
        let senders = self.connection_senders.read().await;
        let mut failed_count = 0;

        for (connection_id, sender) in senders.iter() {
            if sender.send(message.clone()).is_err() {
                failed_count += 1;
                warn!("Failed to broadcast to connection {}", connection_id);
            }
        }

        if failed_count > 0 {
            warn!("Broadcast failed for {} connections", failed_count);
        }

        Ok(())
    }

    async fn route_to_connections(
        &self,
        connection_ids: &[Uuid],
        message: WebSocketFrame,
    ) -> Result<(), WebSocketError> {
        let senders = self.connection_senders.read().await;
        let mut failed_count = 0;

        for &connection_id in connection_ids {
            if let Some(sender) = senders.get(&connection_id) {
                if sender.send(message.clone()).is_err() {
                    failed_count += 1;
                    warn!("Failed to route to connection {}", connection_id);
                }
            }
        }

        if failed_count > 0 {
            warn!(
                "Route to connections failed for {} connections",
                failed_count
            );
        }

        Ok(())
    }

    async fn get_stats(&self) -> RouterStats {
        self.stats.read().await.clone()
    }
}

/// 内存中的房间管理器
pub struct InMemoryRoomManager {
    /// 房间存储
    rooms: Arc<RwLock<HashMap<String, RoomInfo>>>,
    /// 用户房间映射
    user_rooms: Arc<RwLock<HashMap<Uuid, Vec<String>>>>,
    /// 房间统计
    room_stats: Arc<RwLock<HashMap<String, RoomStats>>>,
}

impl InMemoryRoomManager {
    /// 创建新的内存房间管理器
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            user_rooms: Arc::new(RwLock::new(HashMap::new())),
            room_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 更新房间的最后活动时间
    async fn update_room_activity(&self, room_id: &str) {
        let mut rooms = self.rooms.write().await;
        if let Some(room) = rooms.get_mut(room_id) {
            room.update_activity();
        }
    }

    /// 获取或创建房间统计
    async fn get_or_create_room_stats(&self, room_id: &str) -> RoomStats {
        let mut stats = self.room_stats.write().await;
        stats
            .entry(room_id.to_string())
            .or_insert_with(|| RoomStats::new(room_id.to_string()))
            .clone()
    }
}

#[async_trait]
impl RoomManager for InMemoryRoomManager {
    async fn create_room(
        &self,
        room_id: String,
        name: String,
        owner_id: Uuid,
        password: Option<String>,
    ) -> Result<RoomInfo, WebSocketError> {
        let mut rooms = self.rooms.write().await;

        if rooms.contains_key(&room_id) {
            return Err(WebSocketError::RoomNotFound(format!(
                "Room {} already exists",
                room_id
            )));
        }

        let room = RoomInfo::new(room_id.clone(), name, owner_id, password);
        rooms.insert(room_id.clone(), room.clone());

        // 初始化房间统计
        let mut stats = self.room_stats.write().await;
        stats.insert(room_id.clone(), RoomStats::new(room_id.clone()));

        info!("Room {} created by user {}", room_id, owner_id);
        Ok(room)
    }

    async fn delete_room(&self, room_id: &str) -> Result<(), WebSocketError> {
        let mut rooms = self.rooms.write().await;
        if rooms.remove(room_id).is_none() {
            return Err(WebSocketError::RoomNotFound(format!(
                "Room {} not found",
                room_id
            )));
        }

        // 清理相关统计
        let mut stats = self.room_stats.write().await;
        stats.remove(room_id);

        info!("Room {} deleted", room_id);
        Ok(())
    }

    async fn join_room(
        &self,
        room_id: &str,
        user_id: Uuid,
        password: Option<String>,
    ) -> Result<RoomInfo, WebSocketError> {
        let mut rooms = self.rooms.write().await;
        let room = rooms
            .get_mut(room_id)
            .ok_or_else(|| WebSocketError::RoomNotFound(format!("Room {} not found", room_id)))?;

        if !room.can_join(user_id, password) {
            return Err(WebSocketError::InvalidPassword(format!(
                "Cannot join room {}",
                room_id
            )));
        }

        room.increment_users()?;
        room.update_activity();

        // 更新用户房间映射
        let mut user_rooms = self.user_rooms.write().await;
        user_rooms
            .entry(user_id)
            .or_insert_with(Vec::new)
            .push(room_id.to_string());

        info!("User {} joined room {}", user_id, room_id);
        Ok(room.clone())
    }

    async fn leave_room(&self, room_id: &str, user_id: Uuid) -> Result<(), WebSocketError> {
        let mut rooms = self.rooms.write().await;
        let room = rooms
            .get_mut(room_id)
            .ok_or_else(|| WebSocketError::RoomNotFound(format!("Room {} not found", room_id)))?;

        room.decrement_users();
        room.update_activity();

        // 从用户房间映射中移除
        let mut user_rooms = self.user_rooms.write().await;
        if let Some(rooms) = user_rooms.get_mut(&user_id) {
            rooms.retain(|r| r != room_id);
            if rooms.is_empty() {
                user_rooms.remove(&user_id);
            }
        }

        info!("User {} left room {}", user_id, room_id);
        Ok(())
    }

    async fn get_room(&self, room_id: &str) -> Option<RoomInfo> {
        let rooms = self.rooms.read().await;
        rooms.get(room_id).cloned()
    }

    async fn get_user_rooms(&self, user_id: Uuid) -> Vec<RoomInfo> {
        let rooms = self.rooms.read().await;
        let user_rooms = self.user_rooms.read().await;

        if let Some(room_ids) = user_rooms.get(&user_id) {
            room_ids
                .iter()
                .filter_map(|room_id| rooms.get(room_id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    async fn get_room_users(&self, _room_id: &str) -> Vec<UserInfo> {
        // 这个实现需要用户服务的支持
        Vec::new()
    }

    async fn update_room(
        &self,
        room_id: &str,
        updates: RoomUpdate,
    ) -> Result<RoomInfo, WebSocketError> {
        let mut rooms = self.rooms.write().await;
        let room = rooms
            .get_mut(room_id)
            .ok_or_else(|| WebSocketError::RoomNotFound(format!("Room {} not found", room_id)))?;

        if let Some(name) = updates.name {
            room.name = name;
        }

        if let Some(description) = updates.description {
            room.description = Some(description);
        }

        if let Some(password) = updates.password {
            room.password = password;
        }

        if let Some(max_users) = updates.max_users {
            room.max_users = Some(max_users);
        }

        if let Some(status) = updates.status {
            room.status = status;
        }

        room.update_activity();

        info!("Room {} updated", room_id);
        Ok(room.clone())
    }

    async fn room_exists(&self, room_id: &str) -> bool {
        let rooms = self.rooms.read().await;
        rooms.contains_key(room_id)
    }

    async fn is_user_in_room(&self, room_id: &str, user_id: Uuid) -> bool {
        let user_rooms = self.user_rooms.read().await;
        if let Some(rooms) = user_rooms.get(&user_id) {
            rooms.contains(&room_id.to_string())
        } else {
            false
        }
    }

    async fn get_room_stats(&self, room_id: &str) -> Option<RoomStats> {
        let stats = self.room_stats.read().await;
        stats.get(room_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_manager() {
        let manager = InMemoryConnectionManager::new();
        let user_id = Uuid::new_v4();
        let connection =
            ConnectionInfo::new(user_id, "testuser".to_string(), ClientInfo::new(None, None));

        // 测试注册连接
        let connection_id = connection.connection_id;
        manager.register_connection(connection).await.unwrap();

        // 测试获取连接
        let retrieved_connection = manager.get_connection(connection_id).await.unwrap();
        assert_eq!(retrieved_connection.user_id, user_id);

        // 测试获取用户连接
        let user_connections = manager.get_user_connections(user_id).await;
        assert_eq!(user_connections.len(), 1);

        // 测试加入房间
        manager
            .join_room(connection_id, "test_room".to_string())
            .await
            .unwrap();
        assert!(
            manager
                .is_connection_in_room(connection_id, "test_room")
                .await
        );

        // 测试离开房间
        manager
            .leave_room(connection_id, "test_room")
            .await
            .unwrap();
        assert!(
            !manager
                .is_connection_in_room(connection_id, "test_room")
                .await
        );

        // 测试统计信息
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_connections, 1);
        assert_eq!(stats.active_connections, 1);

        // 测试注销连接
        manager.unregister_connection(connection_id).await.unwrap();
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_connections, 0);
    }

    #[tokio::test]
    async fn test_room_manager() {
        let manager = InMemoryRoomManager::new();
        let owner_id = Uuid::new_v4();
        let room_id = "test_room";

        // 测试创建房间
        let room = manager
            .create_room(
                room_id.to_string(),
                "Test Room".to_string(),
                owner_id,
                Some("password123".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(room.room_id, room_id);
        assert_eq!(room.owner_id, owner_id);
        assert!(manager.room_exists(room_id).await);

        // 测试加入房间
        let user_id = Uuid::new_v4();
        manager
            .join_room(room_id, user_id, Some("password123".to_string()))
            .await
            .unwrap();
        assert!(manager.is_user_in_room(room_id, user_id).await);

        // 测试获取用户房间
        let user_rooms = manager.get_user_rooms(user_id).await;
        assert_eq!(user_rooms.len(), 1);

        // 测试离开房间
        manager.leave_room(room_id, user_id).await.unwrap();
        assert!(!manager.is_user_in_room(room_id, user_id).await);

        // 测试获取房间统计
        let stats = manager.get_room_stats(room_id).await;
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().room_id, room_id);

        // 测试删除房间
        manager.delete_room(room_id).await.unwrap();
        assert!(!manager.room_exists(room_id).await);
    }

    #[tokio::test]
    async fn test_message_router() {
        let router = InMemoryMessageRouter::new();
        let connection_id = Uuid::new_v4();

        // 创建测试发送器
        let (tx, mut rx) = mpsc::unbounded_channel();
        router.register_sender(connection_id, tx).await;

        // 测试发送消息
        let message = WebSocketFrame::ping();
        router
            .route_to_connection(connection_id, message.clone())
            .await
            .unwrap();

        // 验证消息已发送
        let received_message = rx.try_recv().unwrap();
        assert_eq!(received_message.message_type, message.message_type);

        // 测试统计信息
        let stats = router.get_stats().await;
        assert_eq!(stats.total_messages, 1);
        assert_eq!(stats.successful_routes, 1);

        // 清理
        router.unregister_sender(connection_id).await;
    }
}
