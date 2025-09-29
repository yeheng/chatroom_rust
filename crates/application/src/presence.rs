use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::ApplicationError;
use domain::{RoomId, UserId};

/// 实时在线统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineStats {
    pub room_id: RoomId,
    pub online_count: u64,
    pub timestamp: DateTime<Utc>,
}

/// 用户状态变化事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "presence_event_type", rename_all = "snake_case")
)]
pub enum PresenceEventType {
    Connected,
    Disconnected,
    Heartbeat, // 用于检测僵尸连接
}

impl std::fmt::Display for PresenceEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PresenceEventType::Connected => write!(f, "Connected"),
            PresenceEventType::Disconnected => write!(f, "Disconnected"),
            PresenceEventType::Heartbeat => write!(f, "Heartbeat"),
        }
    }
}

/// 用户状态变化事件（用于历史数据采集）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPresenceEvent {
    pub event_id: Uuid,
    pub user_id: UserId,
    pub room_id: RoomId,
    pub event_type: PresenceEventType,
    pub timestamp: DateTime<Utc>,
    pub session_id: Uuid, // 用于计算在线时长
    pub user_ip: Option<String>,
    pub user_agent: Option<String>,
}

/// 在线状态管理器trait
/// 使用Redis Set来跟踪每个房间的在线用户
#[async_trait::async_trait]
pub trait PresenceManager: Send + Sync {
    /// 用户连接到房间时调用
    async fn user_connected(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<(), ApplicationError>;

    /// 用户从房间断开时调用
    async fn user_disconnected(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<(), ApplicationError>;

    /// 获取房间内所有在线用户
    async fn get_online_users(&self, room_id: RoomId) -> Result<Vec<UserId>, ApplicationError>;

    /// 检查用户是否在线
    async fn is_user_online(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<bool, ApplicationError>;

    /// 获取用户在线的房间列表
    async fn get_user_rooms(&self, user_id: UserId) -> Result<Vec<RoomId>, ApplicationError>;

    /// 清理用户的所有在线状态（用户完全断开时）
    async fn cleanup_user_presence(&self, user_id: UserId) -> Result<(), ApplicationError>;

    // === 在线统计功能扩展 ===

    /// 获取房间在线用户数量
    async fn get_online_count(&self, room_id: RoomId) -> Result<u64, ApplicationError>;

    /// 获取房间实时统计信息
    async fn get_online_stats(&self, room_id: RoomId) -> Result<OnlineStats, ApplicationError>;

    /// 记录用户状态变化事件（用于历史数据采集）
    async fn record_presence_event(&self, event: UserPresenceEvent)
        -> Result<(), ApplicationError> {
        // 默认实现：只记录日志
        tracing::info!(
            event_id = %event.event_id,
            user_id = %event.user_id,
            room_id = %event.room_id,
            event_type = %event.event_type,
            session_id = %event.session_id,
            "记录用户状态变化事件"
        );
        Ok(())
    }
}

/// Redis实现的在线状态管理器
/// 直接查询Redis，确保数据强一致性
pub struct RedisPresenceManager {
    redis_client: Arc<redis::Client>,
}

impl RedisPresenceManager {
    pub fn new(redis_client: Arc<redis::Client>) -> Self {
        Self { redis_client }
    }

    /// 生成房间在线用户集合的Redis键
    fn room_online_key(&self, room_id: RoomId) -> String {
        format!("room:{}:online", room_id)
    }

    /// 生成用户在线房间集合的Redis键
    fn user_rooms_key(&self, user_id: UserId) -> String {
        format!("user:{}:rooms", user_id)
    }

    /// 获取连接
    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, ApplicationError> {
        self.redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                let message = format!("Redis connection failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })
    }

    }

#[async_trait::async_trait]
impl PresenceManager for RedisPresenceManager {
    async fn user_connected(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<(), ApplicationError> {
        let mut conn = self.get_connection().await?;
        let room_key = self.room_online_key(room_id);
        let user_key = self.user_rooms_key(user_id);

        // 使用Redis管道批量执行操作
        let _: () = redis::pipe()
            .sadd(&room_key, user_id.to_string()) // 将用户添加到房间在线用户集合
            .sadd(&user_key, room_id.to_string()) // 将房间添加到用户在线房间集合
            .expire(&room_key, 86400) // 设置过期时间24小时，防止内存泄漏
            .expire(&user_key, 86400)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                let message = format!("Redis operation failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })?;

        tracing::info!(
            room_id = %room_id,
            user_id = %user_id,
            "用户连接到房间"
        );

        Ok(())
    }

    async fn user_disconnected(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<(), ApplicationError> {
        let mut conn = self.get_connection().await?;
        let room_key = self.room_online_key(room_id);
        let user_key = self.user_rooms_key(user_id);

        // 从Redis集合中移除
        let _: () = redis::pipe()
            .srem(&room_key, user_id.to_string()) // 从房间在线用户集合中移除用户
            .srem(&user_key, room_id.to_string()) // 从用户在线房间集合中移除房间
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                let message = format!("Redis operation failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })?;

        tracing::info!(
            room_id = %room_id,
            user_id = %user_id,
            "用户从房间断开"
        );

        Ok(())
    }

    async fn get_online_users(&self, room_id: RoomId) -> Result<Vec<UserId>, ApplicationError> {
        let mut conn = self.get_connection().await?;
        let room_key = self.room_online_key(room_id);

        let members: Vec<String> = redis::cmd("SMEMBERS")
            .arg(&room_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                let message = format!("Redis operation failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })?;

        // 将字符串转换为UserId
        let user_ids: Result<Vec<UserId>, _> = members
            .into_iter()
            .map(|s| s.parse::<Uuid>().map(UserId::from))
            .collect();

        let user_ids = user_ids.map_err(|e| {
            let message = format!("Invalid UUID in Redis: {e}");
            ApplicationError::infrastructure_with_source(message, e)
        })?;

        Ok(user_ids)
    }

    async fn is_user_online(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> Result<bool, ApplicationError> {
        let mut conn = self.get_connection().await?;
        let room_key = self.room_online_key(room_id);

        let is_member: bool = redis::cmd("SISMEMBER")
            .arg(&room_key)
            .arg(user_id.to_string())
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                let message = format!("Redis operation failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })?;

        Ok(is_member)
    }

    async fn get_user_rooms(&self, user_id: UserId) -> Result<Vec<RoomId>, ApplicationError> {
        let mut conn = self.get_connection().await?;
        let user_key = self.user_rooms_key(user_id);

        let members: Vec<String> = redis::cmd("SMEMBERS")
            .arg(&user_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                let message = format!("Redis operation failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })?;

        // 将字符串转换为RoomId
        let room_ids: Result<Vec<RoomId>, _> = members
            .into_iter()
            .map(|s| s.parse::<Uuid>().map(RoomId::from))
            .collect();

        let room_ids = room_ids.map_err(|e| {
            let message = format!("Invalid UUID in Redis: {e}");
            ApplicationError::infrastructure_with_source(message, e)
        })?;

        Ok(room_ids)
    }

    async fn cleanup_user_presence(&self, user_id: UserId) -> Result<(), ApplicationError> {
        // 先获取用户所有在线房间
        let room_ids = self.get_user_rooms(user_id).await?;

        if room_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        let user_key = self.user_rooms_key(user_id);

        // 创建管道批量清理
        let mut pipe = redis::pipe();

        // 从每个房间的在线用户集合中移除这个用户
        for room_id in room_ids {
            let room_key = self.room_online_key(room_id);
            pipe.srem(&room_key, user_id.to_string());
        }

        // 删除用户的在线房间集合
        pipe.del(&user_key);

        let _: () = pipe.query_async(&mut conn).await.map_err(|e| {
            let message = format!("Redis operation failed: {e}");
            ApplicationError::infrastructure_with_source(message, e)
        })?;

        tracing::info!(
            user_id = %user_id,
            "清理用户在线状态"
        );

        Ok(())
    }

    // === 在线统计功能实现 ===

    async fn get_online_count(&self, room_id: RoomId) -> Result<u64, ApplicationError> {
        let mut conn = self.get_connection().await?;
        let room_key = self.room_online_key(room_id);

        let count: u64 = redis::cmd("SCARD")
            .arg(&room_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                let message = format!("Redis operation failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })?;

        Ok(count)
    }

    async fn get_online_stats(&self, room_id: RoomId) -> Result<OnlineStats, ApplicationError> {
        // 直接查询Redis，确保数据强一致性
        let count = self.get_online_count(room_id).await?;
        let stats = OnlineStats {
            room_id,
            online_count: count,
            timestamp: Utc::now(),
        };

        Ok(stats)
    }

    async fn record_presence_event(&self, event: UserPresenceEvent) -> Result<(), ApplicationError> {
        let mut conn = self.get_connection().await?;
        let stream_key = "presence_events_stream";

        // 将事件写入Redis Stream
        let _: String = redis::cmd("XADD")
            .arg(stream_key)
            .arg("*")  // 自动生成的ID
            .arg("event_id")
            .arg(event.event_id.to_string())
            .arg("user_id")
            .arg(event.user_id.to_string())
            .arg("room_id")
            .arg(event.room_id.to_string())
            .arg("event_type")
            .arg(event.event_type.to_string())
            .arg("timestamp")
            .arg(event.timestamp.to_rfc3339())
            .arg("session_id")
            .arg(event.session_id.to_string())
            .arg("user_ip")
            .arg(event.user_ip.clone().unwrap_or_default())
            .arg("user_agent")
            .arg(event.user_agent.clone().unwrap_or_default())
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                let message = format!("Redis Stream operation failed: {e}");
                ApplicationError::infrastructure_with_source(message, e)
            })?;

        tracing::info!(
            event_id = %event.event_id,
            user_id = %event.user_id,
            room_id = %event.room_id,
            event_type = %event.event_type,
            "事件已写入Redis Stream"
        );

        Ok(())
    }

    }

/// 内存实现的在线状态管理器（用于测试）
pub mod memory {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use tokio::sync::RwLock;

    pub struct MemoryPresenceManager {
        room_users: RwLock<HashMap<RoomId, HashSet<UserId>>>,
        user_rooms: RwLock<HashMap<UserId, HashSet<RoomId>>>,
    }

    impl Default for MemoryPresenceManager {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MemoryPresenceManager {
        pub fn new() -> Self {
            Self {
                room_users: RwLock::new(HashMap::new()),
                user_rooms: RwLock::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl PresenceManager for MemoryPresenceManager {
        async fn user_connected(
            &self,
            room_id: RoomId,
            user_id: UserId,
        ) -> Result<(), ApplicationError> {
            let mut room_users = self.room_users.write().await;
            let mut user_rooms = self.user_rooms.write().await;

            room_users
                .entry(room_id)
                .or_insert_with(HashSet::new)
                .insert(user_id);
            user_rooms
                .entry(user_id)
                .or_insert_with(HashSet::new)
                .insert(room_id);

            Ok(())
        }

        async fn user_disconnected(
            &self,
            room_id: RoomId,
            user_id: UserId,
        ) -> Result<(), ApplicationError> {
            let mut room_users = self.room_users.write().await;
            let mut user_rooms = self.user_rooms.write().await;

            if let Some(users) = room_users.get_mut(&room_id) {
                users.remove(&user_id);
                if users.is_empty() {
                    room_users.remove(&room_id);
                }
            }

            if let Some(rooms) = user_rooms.get_mut(&user_id) {
                rooms.remove(&room_id);
                if rooms.is_empty() {
                    user_rooms.remove(&user_id);
                }
            }

            Ok(())
        }

        async fn get_online_users(&self, room_id: RoomId) -> Result<Vec<UserId>, ApplicationError> {
            let room_users = self.room_users.read().await;
            let users = room_users.get(&room_id).cloned().unwrap_or_default();
            Ok(users.into_iter().collect())
        }

        async fn is_user_online(
            &self,
            room_id: RoomId,
            user_id: UserId,
        ) -> Result<bool, ApplicationError> {
            let room_users = self.room_users.read().await;
            let is_online = room_users
                .get(&room_id)
                .map(|users| users.contains(&user_id))
                .unwrap_or(false);
            Ok(is_online)
        }

        async fn get_user_rooms(&self, user_id: UserId) -> Result<Vec<RoomId>, ApplicationError> {
            let user_rooms = self.user_rooms.read().await;
            let rooms = user_rooms.get(&user_id).cloned().unwrap_or_default();
            Ok(rooms.into_iter().collect())
        }

        async fn cleanup_user_presence(&self, user_id: UserId) -> Result<(), ApplicationError> {
            let room_ids = self.get_user_rooms(user_id).await?;

            for room_id in room_ids {
                self.user_disconnected(room_id, user_id).await?;
            }

            Ok(())
        }

        // === 在线统计功能实现 ===

        async fn get_online_count(&self, room_id: RoomId) -> Result<u64, ApplicationError> {
            let room_users = self.room_users.read().await;
            let count = room_users
                .get(&room_id)
                .map(|users| users.len() as u64)
                .unwrap_or(0);
            Ok(count)
        }

        async fn get_online_stats(&self, room_id: RoomId) -> Result<OnlineStats, ApplicationError> {
            let count = self.get_online_count(room_id).await?;
            Ok(OnlineStats {
                room_id,
                online_count: count,
                timestamp: Utc::now(),
            })
        }

        async fn record_presence_event(
            &self,
            event: UserPresenceEvent,
        ) -> Result<(), ApplicationError> {
            // 内存实现只记录日志
            tracing::info!(
                event_id = %event.event_id,
                user_id = %event.user_id,
                room_id = %event.room_id,
                event_type = %event.event_type,
                session_id = %event.session_id,
                "记录用户状态变化事件（内存实现）"
            );
            Ok(())
        }
    }
}
