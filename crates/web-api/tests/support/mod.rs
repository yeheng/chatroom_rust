use std::{collections::HashMap, sync::Arc};

use application::{
    password::PasswordHasher,
    services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies},
    MessageBroadcaster, PasswordHasherError, PresenceManager, SystemClock,
};
use async_trait::async_trait;
use axum::Router;
use domain::{
    ChatRoom, ChatRoomRepository, Message, MessageId, MessageRepository, RoomId, RoomMember,
    RoomMemberRepository, User, UserEmail, UserId, UserRepository,
};
use infrastructure::LocalMessageBroadcaster;
use tokio::sync::RwLock;
use uuid::Uuid;

use web_api::{router as build_router_fn, AppState};

#[derive(Default)]
pub struct InMemoryUserRepository {
    data: Arc<RwLock<HashMap<Uuid, User>>>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl UserRepository for InMemoryUserRepository {
    fn create(&self, user: User) -> domain::RepositoryFuture<User> {
        let repo = self.data.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(user.id);
            if guard.contains_key(&id) {
                return Err(domain::RepositoryError::Conflict);
            }
            let stored = user.clone();
            guard.insert(id, user);
            Ok(stored)
        })
    }

    fn update(&self, user: User) -> domain::RepositoryFuture<User> {
        let repo = self.data.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(user.id);
            if !guard.contains_key(&id) {
                return Err(domain::RepositoryError::NotFound);
            }
            let stored = user.clone();
            guard.insert(id, user);
            Ok(stored)
        })
    }

    fn find_by_id(&self, id: UserId) -> domain::RepositoryFuture<Option<User>> {
        let repo = self.data.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard.get(&Uuid::from(id)).cloned())
        })
    }

    fn find_by_email(&self, email: UserEmail) -> domain::RepositoryFuture<Option<User>> {
        let repo = self.data.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard.values().find(|u| u.email == email).cloned())
        })
    }
}

#[derive(Default)]
pub struct InMemoryChatRoomRepository {
    rooms: Arc<RwLock<HashMap<Uuid, ChatRoom>>>,
}

impl InMemoryChatRoomRepository {
    pub fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ChatRoomRepository for InMemoryChatRoomRepository {
    fn create(&self, room: ChatRoom) -> domain::RepositoryFuture<ChatRoom> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(room.id);
            if guard.contains_key(&id) {
                return Err(domain::RepositoryError::Conflict);
            }
            let stored = room.clone();
            guard.insert(id, room);
            Ok(stored)
        })
    }

    fn update(&self, room: ChatRoom) -> domain::RepositoryFuture<ChatRoom> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(room.id);
            if !guard.contains_key(&id) {
                return Err(domain::RepositoryError::NotFound);
            }
            let stored = room.clone();
            guard.insert(id, room);
            Ok(stored)
        })
    }

    fn delete(&self, id: RoomId) -> domain::RepositoryFuture<()> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            guard.remove(&Uuid::from(id));
            Ok(())
        })
    }

    fn find_by_id(&self, id: RoomId) -> domain::RepositoryFuture<Option<ChatRoom>> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard.get(&Uuid::from(id)).cloned())
        })
    }

    fn list_by_owner(&self, owner: UserId) -> domain::RepositoryFuture<Vec<ChatRoom>> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard
                .values()
                .filter(|room| room.owner_id == owner)
                .cloned()
                .collect())
        })
    }
}

#[derive(Default)]
pub struct InMemoryRoomMemberRepository {
    members: Arc<RwLock<HashMap<(Uuid, Uuid), RoomMember>>>,
}

impl InMemoryRoomMemberRepository {
    pub fn new() -> Self {
        Self {
            members: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl RoomMemberRepository for InMemoryRoomMemberRepository {
    fn upsert(&self, member: RoomMember) -> domain::RepositoryFuture<RoomMember> {
        let repo = self.members.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let key = (Uuid::from(member.room_id), Uuid::from(member.user_id));
            guard.insert(key, member.clone());
            Ok(member)
        })
    }

    fn remove(&self, room_id: RoomId, user_id: UserId) -> domain::RepositoryFuture<()> {
        let repo = self.members.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            guard.remove(&(Uuid::from(room_id), Uuid::from(user_id)));
            Ok(())
        })
    }

    fn find(
        &self,
        room_id: RoomId,
        user_id: UserId,
    ) -> domain::RepositoryFuture<Option<RoomMember>> {
        let repo = self.members.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard
                .get(&(Uuid::from(room_id), Uuid::from(user_id)))
                .cloned())
        })
    }

    fn list_members(&self, room_id: RoomId) -> domain::RepositoryFuture<Vec<RoomMember>> {
        let repo = self.members.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard
                .values()
                .filter(|member| member.room_id == room_id)
                .cloned()
                .collect())
        })
    }
}

#[derive(Default)]
pub struct InMemoryMessageRepository {
    messages: Arc<RwLock<HashMap<Uuid, Vec<Message>>>>,
}

impl InMemoryMessageRepository {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl MessageRepository for InMemoryMessageRepository {
    fn create(&self, message: Message) -> domain::RepositoryFuture<Message> {
        let repo = self.messages.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            guard
                .entry(Uuid::from(message.room_id))
                .or_default()
                .push(message.clone());
            Ok(message)
        })
    }

    fn find_by_id(&self, id: MessageId) -> domain::RepositoryFuture<Option<Message>> {
        let repo = self.messages.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            for list in guard.values() {
                if let Some(found) = list.iter().find(|msg| msg.id == id) {
                    return Ok(Some(found.clone()));
                }
            }
            Ok(None)
        })
    }

    fn list_recent(
        &self,
        room_id: RoomId,
        limit: u32,
        before: Option<MessageId>,
    ) -> domain::RepositoryFuture<Vec<Message>> {
        let repo = self.messages.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            let mut items = guard.get(&Uuid::from(room_id)).cloned().unwrap_or_default();
            if let Some(before_id) = before {
                let cutoff = items
                    .iter()
                    .find(|msg| msg.id == before_id)
                    .map(|msg| msg.created_at);
                if let Some(cutoff) = cutoff {
                    items.retain(|msg| msg.created_at < cutoff);
                }
            }
            items.sort_by_key(|msg| std::cmp::Reverse(msg.created_at));
            items.truncate(limit as usize);
            items.reverse();
            Ok(items)
        })
    }
}

pub struct PlainPasswordHasher;

#[async_trait]
impl PasswordHasher for PlainPasswordHasher {
    async fn hash(&self, plaintext: &str) -> Result<domain::PasswordHash, PasswordHasherError> {
        domain::PasswordHash::new(plaintext.to_owned())
            .map_err(|err| PasswordHasherError::hash_error(err.to_string()))
    }

    async fn verify(
        &self,
        plaintext: &str,
        hashed: &domain::PasswordHash,
    ) -> Result<bool, PasswordHasherError> {
        Ok(hashed.as_str() == plaintext)
    }
}

pub fn build_router() -> Router {
    let user_repo = Arc::new(InMemoryUserRepository::new());
    let room_repo = Arc::new(InMemoryChatRoomRepository::new());
    let member_repo = Arc::new(InMemoryRoomMemberRepository::new());
    let message_repo = Arc::new(InMemoryMessageRepository::new());
    let clock = Arc::new(SystemClock::default());
    let password_hasher = Arc::new(PlainPasswordHasher);
    let broadcaster = Arc::new(LocalMessageBroadcaster::new(128));

    let user_service = Arc::new(UserService::new(UserServiceDependencies {
        user_repository: user_repo.clone(),
        password_hasher: password_hasher.clone(),
        clock: clock.clone(),
    }));

    let chat_service = Arc::new(ChatService::new(ChatServiceDependencies {
        room_repository: room_repo,
        member_repository: member_repo,
        message_repository: message_repo,
        password_hasher,
        clock,
        broadcaster: broadcaster.clone() as Arc<dyn MessageBroadcaster>,
    }));

    // 创建测试用的JWT服务
    let jwt_service = Arc::new(web_api::JwtService::new(web_api::JwtConfig::default()));

    // 创建测试用的PresenceManager（使用内存实现）
    let presence_manager = Arc::new(application::presence::memory::MemoryPresenceManager::new())
        as Arc<dyn PresenceManager>;

    let state = AppState::new(
        user_service,
        chat_service,
        infrastructure::BroadcasterType::Local(broadcaster),
        jwt_service,
        presence_manager,
    );
    build_router_fn(state)
}
