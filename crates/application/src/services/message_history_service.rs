//! 消息历史查询与缓存服务（Task 6）
//!
//! 提供基于游标分页的历史消息查询、基础权限校验以及热点房间的内存 LRU 缓存（TTL）。

use crate::errors::{ApplicationError, ApplicationResult, ChatRoomError};
use chrono::DateTime;
use domain::chatroom::*;
use domain::message::{Message, MessageType};
use domain::user::{User, UserStatus};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

/// 历史查询请求（基于游标分页）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRoomHistoryQuery {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub page_size: u32,
    pub cursor: Option<String>,
    /// 可选的消息类型过滤
    pub message_type: Option<MessageType>,
    /// 是否包含已删除/撤回的消息
    pub include_deleted: bool,
}

/// 搜索请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSearchQuery {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub keyword: String,
    pub page_size: u32,
    pub cursor: Option<String>,
    pub message_type: Option<MessageType>,
}

/// 历史分页响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistoryPage {
    pub messages: Vec<Message>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[async_trait::async_trait]
pub trait MessageHistoryService: Send + Sync {
    async fn get_room_history(
        &self,
        query: GetRoomHistoryQuery,
    ) -> ApplicationResult<MessageHistoryPage>;
    async fn search_messages(
        &self,
        query: MessageSearchQuery,
    ) -> ApplicationResult<MessageHistoryPage>;
}

/// 简单的内存 LRU 缓存条目
struct CacheEntry {
    value: MessageHistoryPage,
    stored_at: std::time::Instant,
    last_access: std::time::Instant,
}

/// 内存 LRU 缓存
struct LruCache {
    map: HashMap<String, CacheEntry>,
    order: VecDeque<String>,
    capacity: usize,
    ttl: std::time::Duration,
    hits: u64,
    misses: u64,
}

impl LruCache {
    fn new(capacity: usize, ttl_secs: u64) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
            ttl: std::time::Duration::from_secs(ttl_secs),
            hits: 0,
            misses: 0,
        }
    }

    fn key(
        room_id: Uuid,
        cursor: &Option<String>,
        page_size: u32,
        filter: &Option<MessageType>,
        keyword: &Option<String>,
    ) -> String {
        format!(
            "{}|{}|{}|{:?}|{}",
            room_id,
            cursor.clone().unwrap_or_else(|| "_".into()),
            page_size,
            filter,
            keyword.clone().unwrap_or_else(|| "_".into())
        )
    }

    fn get(&mut self, key: &str) -> Option<MessageHistoryPage> {
        let now = std::time::Instant::now();
        let value_opt = {
            if let Some(entry) = self.map.get_mut(key) {
                if now.duration_since(entry.stored_at) <= self.ttl {
                    entry.last_access = now;
                    Some(entry.value.clone())
                } else {
                    // 过期，移除
                    None
                }
            } else {
                None
            }
        };
        match value_opt {
            Some(v) => {
                // 若未过期，更新 LRU 顺序
                self.touch(key);
                self.hits += 1;
                Some(v)
            }
            None => {
                // 如果存在但过期，需要清理
                if self.map.contains_key(key) {
                    self.map.remove(key);
                    self.remove_from_order(key);
                }
                self.misses += 1;
                None
            }
        }
    }

    fn put(&mut self, key: String, value: MessageHistoryPage) {
        self.evict_if_needed();
        let now = std::time::Instant::now();
        self.order.push_back(key.clone());
        self.map.insert(
            key,
            CacheEntry {
                value,
                stored_at: now,
                last_access: now,
            },
        );
    }

    fn evict_if_needed(&mut self) {
        // 清理过期
        let now = std::time::Instant::now();
        let expired_keys: Vec<String> = self
            .map
            .iter()
            .filter_map(|(k, v)| {
                if now.duration_since(v.stored_at) > self.ttl {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        for k in expired_keys {
            self.map.remove(&k);
            self.remove_from_order(&k);
        }

        // 容量淘汰（LRU）
        while self.map.len() >= self.capacity {
            // 找到最久未访问
            if let Some(lru_key) = self
                .order
                .iter()
                .min_by_key(|k| self.map.get(&***k).map(|e| e.last_access))
                .cloned()
            {
                self.map.remove(&lru_key);
                self.remove_from_order(&lru_key);
            } else {
                break;
            }
        }
    }

    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos).unwrap();
            self.order.push_back(k);
        }
    }

    fn remove_from_order(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
    }
}

/// 消息历史服务实现（内存版，易于替换为数据库实现）
pub struct MessageHistoryServiceImpl {
    messages_by_room: Arc<RwLock<HashMap<Uuid, Vec<Message>>>>,
    rooms: Arc<RwLock<HashMap<Uuid, ChatRoom>>>,
    room_members: Arc<RwLock<HashMap<Uuid, HashSet<Uuid>>>>,
    users: Arc<RwLock<HashMap<Uuid, User>>>,

    cache: Arc<Mutex<LruCache>>,
}

impl Default for MessageHistoryServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageHistoryServiceImpl {
    pub fn new() -> Self {
        Self {
            messages_by_room: Arc::new(RwLock::new(HashMap::new())),
            rooms: Arc::new(RwLock::new(HashMap::new())),
            room_members: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(Mutex::new(LruCache::new(1024, 3600))), // 1h TTL, 1024 条缓存项
        }
    }

    /// 测试辅助：创建公开房间
    pub async fn create_public_room(&self, name: &str, owner_id: Uuid) -> Uuid {
        let room = ChatRoom::new_public(name.to_string(), owner_id, None, None)
            .map_err(ApplicationError::from)
            .unwrap();
        let id = room.id;
        self.rooms.write().await.insert(id, room);
        id
    }

    /// 测试辅助：创建私密房间
    pub async fn create_private_room(&self, name: &str, owner_id: Uuid, password: &str) -> Uuid {
        // 重要：将明文密码传入 Domain，由 Domain 统一负责哈希，避免双重哈希
        let room = ChatRoom::new_private(name.to_string(), None, owner_id, password)
            .map_err(ApplicationError::from)
            .unwrap();
        let id = room.id;
        self.rooms.write().await.insert(id, room);
        id
    }

    /// 测试辅助：加入房间（若为私密房间需提供密码）
    pub async fn join_room(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        password: Option<String>,
    ) -> ApplicationResult<()> {
        // 确保用户存在且活跃
        self.ensure_user_exists(user_id).await?;
        let users = self.users.read().await;
        let user = users
            .get(&user_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("用户不存在: {}", user_id)))?;
        if user.status != UserStatus::Active {
            return Err(ApplicationError::Unauthorized("用户状态不活跃".into()));
        }
        drop(users);

        let rooms = self.rooms.read().await;
        let room = rooms
            .get(&room_id)
            .ok_or(ChatRoomError::RoomNotFound(room_id))?;
        if room.status != ChatRoomStatus::Active {
            return Err(ChatRoomError::RoomDeleted(room_id).into());
        }
        if room.is_private {
            let provided = password.ok_or(ChatRoomError::InvalidPassword)?;
            let ok =
                bcrypt::verify(&provided, room.password_hash.as_ref().unwrap()).unwrap_or(false);
            if !ok {
                return Err(ChatRoomError::InvalidPassword.into());
            }
        }
        drop(rooms);

        let mut members = self.room_members.write().await;
        members.entry(room_id).or_default().insert(user_id);
        Ok(())
    }

    /// 测试辅助：插入消息
    pub async fn add_message(&self, message: Message) {
        let mut map = self.messages_by_room.write().await;
        map.entry(message.room_id).or_default().push(message);
    }

    /// 测试/统计：获取缓存命中信息
    pub async fn get_cache_stats(&self) -> (u64, u64, usize) {
        let cache = self.cache.lock().await;
        (cache.hits, cache.misses, cache.map.len())
    }

    async fn ensure_user_exists(&self, user_id: Uuid) -> ApplicationResult<()> {
        if self.users.read().await.contains_key(&user_id) {
            return Ok(());
        }
        let username = format!("user-{}", &user_id.to_string()[..8]);
        let email = format!("{}@example.com", username);
        let now = chrono::Utc::now();
        let user = User::with_id(
            user_id,
            username,
            email,
            None,
            None,
            UserStatus::Active,
            now,
            now,
            Some(now),
        )
        .map_err(ApplicationError::from)?;
        self.users.write().await.insert(user_id, user);
        Ok(())
    }

    async fn validate_permissions(&self, room_id: Uuid, user_id: Uuid) -> ApplicationResult<()> {
        // 确保用户存在（测试友好）
        let _ = self.ensure_user_exists(user_id).await;
        // 用户
        let users = self.users.read().await;
        let user = users
            .get(&user_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("用户不存在: {}", user_id)))?;
        if user.status != UserStatus::Active {
            return Err(ApplicationError::Unauthorized("用户状态不活跃".into()));
        }
        drop(users);

        // 房间
        let rooms = self.rooms.read().await;
        let room = rooms
            .get(&room_id)
            .ok_or(ChatRoomError::RoomNotFound(room_id))?;
        if room.status != ChatRoomStatus::Active {
            return Err(ChatRoomError::RoomDeleted(room_id).into());
        }
        drop(rooms);

        // 成员
        let members = self.room_members.read().await;
        if !members
            .get(&room_id)
            .map(|s| s.contains(&user_id))
            .unwrap_or(false)
        {
            return Err(ApplicationError::Unauthorized("用户不在房间中".into()));
        }
        Ok(())
    }

    fn encode_cursor(message: &Message) -> String {
        #[derive(Serialize)]
        struct C {
            t: i64,
            n: u32,
            id: String,
        }
        let ts = message.created_at.timestamp();
        let ns = message.created_at.timestamp_subsec_nanos();
        let c = C {
            t: ts,
            n: ns,
            id: message.id.to_string(),
        };
        data_encoding::BASE64.encode(serde_json::to_string(&c).unwrap().as_bytes())
    }

    fn decode_cursor(cursor: &str) -> Option<(chrono::DateTime<chrono::Utc>, Uuid)> {
        #[derive(Deserialize)]
        struct C {
            t: i64,
            n: u32,
            id: String,
        }
        let data = data_encoding::BASE64.decode(cursor.as_bytes()).ok()?;
        let c: C = serde_json::from_slice(&data).ok()?;

        let dt = DateTime::from_timestamp(c.t, c.n)?.naive_utc();
        let dt = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc);
        let id = Uuid::parse_str(&c.id).ok()?;
        Some((dt, id))
    }

    fn filter_and_sort(
        mut list: Vec<Message>,
        include_deleted: bool,
        message_type: &Option<MessageType>,
    ) -> Vec<Message> {
        if !include_deleted {
            list.retain(|m| m.is_visible());
        }
        if let Some(t) = message_type {
            list.retain(|m| &m.message_type == t);
        }
        list.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        list
    }
}

#[async_trait::async_trait]
impl MessageHistoryService for MessageHistoryServiceImpl {
    async fn get_room_history(
        &self,
        query: GetRoomHistoryQuery,
    ) -> ApplicationResult<MessageHistoryPage> {
        // 权限校验
        self.validate_permissions(query.room_id, query.user_id)
            .await?;

        // 缓存键
        let key = {
            let kw: Option<String> = None;
            let mut cache = self.cache.lock().await;
            let key = LruCache::key(
                query.room_id,
                &query.cursor,
                query.page_size,
                &query.message_type,
                &kw,
            );
            if let Some(page) = cache.get(&key) {
                return Ok(page);
            }
            key
        };

        // 读取消息
        let room_msgs = {
            let map = self.messages_by_room.read().await;
            map.get(&query.room_id).cloned().unwrap_or_default()
        };

        let list = Self::filter_and_sort(room_msgs, query.include_deleted, &query.message_type);

        // 游标定位
        let start_index = if let Some(ref c) = query.cursor {
            if let Some((dt, id)) = Self::decode_cursor(c) {
                list.iter()
                    .position(|m| m.created_at == dt && m.id == id)
                    .map(|i| i + 1)
                    .unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        let size = query.page_size as usize;
        let end = std::cmp::min(start_index + size, list.len());
        let page_msgs = if start_index < list.len() {
            list[start_index..end].to_vec()
        } else {
            vec![]
        };
        let has_more = end < list.len();
        let next_cursor = page_msgs.last().map(Self::encode_cursor);

        let page = MessageHistoryPage {
            messages: page_msgs,
            has_more,
            next_cursor,
        };

        let mut cache = self.cache.lock().await;
        cache.put(key, page.clone());
        Ok(page)
    }

    async fn search_messages(
        &self,
        query: MessageSearchQuery,
    ) -> ApplicationResult<MessageHistoryPage> {
        // 权限校验
        self.validate_permissions(query.room_id, query.user_id)
            .await?;

        // 缓存键
        let key = {
            let kw = Some(query.keyword.clone());
            let mut cache = self.cache.lock().await;
            let key = LruCache::key(
                query.room_id,
                &query.cursor,
                query.page_size,
                &query.message_type,
                &kw,
            );
            if let Some(page) = cache.get(&key) {
                return Ok(page);
            }
            key
        };

        // 读取消息
        let room_msgs = {
            let map = self.messages_by_room.read().await;
            map.get(&query.room_id).cloned().unwrap_or_default()
        };

        // 过滤
        let mut list = room_msgs;
        list.retain(|m| m.is_visible());
        if let Some(t) = &query.message_type {
            list.retain(|m| &m.message_type == t);
        }
        let kw_lower = query.keyword.to_lowercase();
        list.retain(|m| m.content.to_lowercase().contains(&kw_lower));
        list.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });

        // 游标
        let start_index = if let Some(ref c) = query.cursor {
            if let Some((dt, id)) = Self::decode_cursor(c) {
                list.iter()
                    .position(|m| m.created_at == dt && m.id == id)
                    .map(|i| i + 1)
                    .unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        let size = query.page_size as usize;
        let end = std::cmp::min(start_index + size, list.len());
        let page_msgs = if start_index < list.len() {
            list[start_index..end].to_vec()
        } else {
            vec![]
        };
        let has_more = end < list.len();
        let next_cursor = page_msgs.last().map(Self::encode_cursor);

        let page = MessageHistoryPage {
            messages: page_msgs,
            has_more,
            next_cursor,
        };
        let mut cache = self.cache.lock().await;
        cache.put(key, page.clone());
        Ok(page)
    }
}

#[cfg(test)]
mod tests_helpers {
    use super::*;

    pub async fn create_test_message_history_service() -> MessageHistoryServiceImpl {
        MessageHistoryServiceImpl::new()
    }

    pub async fn create_test_message(room_id: Uuid, user_id: Uuid, content: String) -> Message {
        Message::new_text(room_id, user_id, content).unwrap()
    }
}
