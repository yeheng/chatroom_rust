use std::{collections::HashMap, sync::Arc};

use application::{
    services::{ChatService, ChatServiceDependencies, UserService, UserServiceDependencies},
    MessageBroadcaster, MessageBroadcast, PasswordHasher, PasswordHasherError, SystemClock,
};
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use domain::{
    ChatRoom, ChatRoomRepository, Message, MessageId, MessageRepository, PasswordHash, RepositoryError, RepositoryFuture,
    RoomId, RoomMember, RoomMemberRepository, User, UserEmail, UserId, UserRepository,
};
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tower::ServiceExt;
use uuid::Uuid;

use web_api::{router, AppState};

#[derive(Default)]
struct InMemoryUserRepository {
    data: Arc<RwLock<HashMap<Uuid, User>>>,
}

impl InMemoryUserRepository {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl UserRepository for InMemoryUserRepository {
    fn create(&self, user: User) -> RepositoryFuture<User> {
        let repo = self.data.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(user.id);
            if guard.contains_key(&id) {
                return Err(RepositoryError::Conflict);
            }
            let stored = user.clone();
            guard.insert(id, user);
            Ok(stored)
        })
    }

    fn update(&self, user: User) -> RepositoryFuture<User> {
        let repo = self.data.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(user.id);
            if !guard.contains_key(&id) {
                return Err(RepositoryError::NotFound);
            }
            let stored = user.clone();
            guard.insert(id, user);
            Ok(stored)
        })
    }

    fn find_by_id(&self, id: UserId) -> RepositoryFuture<Option<User>> {
        let repo = self.data.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard.get(&Uuid::from(id)).cloned())
        })
    }

    fn find_by_email(&self, email: UserEmail) -> RepositoryFuture<Option<User>> {
        let repo = self.data.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard
                .values()
                .find(|u| u.email.as_str() == email.as_str())
                .cloned())
        })
    }
}

#[derive(Default)]
struct InMemoryChatRoomRepository {
    rooms: Arc<RwLock<HashMap<Uuid, ChatRoom>>>,
}

impl InMemoryChatRoomRepository {
    fn new() -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ChatRoomRepository for InMemoryChatRoomRepository {
    fn create(&self, room: ChatRoom) -> RepositoryFuture<ChatRoom> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(room.id);
            if guard.contains_key(&id) {
                return Err(RepositoryError::Conflict);
            }
            let stored = room.clone();
            guard.insert(id, room);
            Ok(stored)
        })
    }

    fn update(&self, room: ChatRoom) -> RepositoryFuture<ChatRoom> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let id = Uuid::from(room.id);
            if !guard.contains_key(&id) {
                return Err(RepositoryError::NotFound);
            }
            let stored = room.clone();
            guard.insert(id, room);
            Ok(stored)
        })
    }

    fn find_by_id(&self, id: RoomId) -> RepositoryFuture<Option<ChatRoom>> {
        let repo = self.rooms.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard.get(&Uuid::from(id)).cloned())
        })
    }

    fn list_by_owner(&self, owner: UserId) -> RepositoryFuture<Vec<ChatRoom>> {
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
struct InMemoryRoomMemberRepository {
    members: Arc<RwLock<HashMap<(Uuid, Uuid), RoomMember>>>,
}

impl InMemoryRoomMemberRepository {
    fn new() -> Self {
        Self {
            members: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl RoomMemberRepository for InMemoryRoomMemberRepository {
    fn upsert(&self, member: RoomMember) -> RepositoryFuture<RoomMember> {
        let repo = self.members.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let key = (Uuid::from(member.room_id), Uuid::from(member.user_id));
            guard.insert(key, member.clone());
            Ok(member)
        })
    }

    fn remove(&self, room_id: RoomId, user_id: UserId) -> RepositoryFuture<()> {
        let repo = self.members.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            guard.remove(&(Uuid::from(room_id), Uuid::from(user_id)));
            Ok(())
        })
    }

    fn find(&self, room_id: RoomId, user_id: UserId) -> RepositoryFuture<Option<RoomMember>> {
        let repo = self.members.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            Ok(guard
                .get(&(Uuid::from(room_id), Uuid::from(user_id)))
                .cloned())
        })
    }

    fn list_members(&self, room_id: RoomId) -> RepositoryFuture<Vec<RoomMember>> {
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
struct InMemoryMessageRepository {
    messages: Arc<RwLock<HashMap<Uuid, Vec<Message>>>>,
}

impl InMemoryMessageRepository {
    fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl MessageRepository for InMemoryMessageRepository {
    fn create(&self, message: Message) -> RepositoryFuture<Message> {
        let repo = self.messages.clone();
        Box::pin(async move {
            let mut guard = repo.write().await;
            let room_id = Uuid::from(message.room_id);
            let entry = guard.entry(room_id).or_default();
            entry.push(message.clone());
            Ok(message)
        })
    }

    fn find_by_id(&self, id: MessageId) -> RepositoryFuture<Option<Message>> {
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
    ) -> RepositoryFuture<Vec<Message>> {
        let repo = self.messages.clone();
        Box::pin(async move {
            let guard = repo.read().await;
            let mut items = guard
                .get(&Uuid::from(room_id))
                .cloned()
                .unwrap_or_default();
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

struct PlainPasswordHasher;

#[async_trait]
impl PasswordHasher for PlainPasswordHasher {
    async fn hash(&self, plaintext: &str) -> Result<PasswordHash, PasswordHasherError> {
        PasswordHash::new(plaintext.to_owned()).map_err(|err| PasswordHasherError::hash_error(err.to_string()))
    }

    async fn verify(&self, plaintext: &str, hashed: &PasswordHash) -> Result<bool, PasswordHasherError> {
        Ok(hashed.as_str() == plaintext)
    }
}

struct NoopBroadcaster;

#[async_trait]
impl MessageBroadcaster for NoopBroadcaster {
    async fn broadcast(&self, _payload: MessageBroadcast) -> Result<(), application::broadcaster::BroadcastError> {
        Ok(())
    }
}

fn test_router() -> Router {
    let user_repo = Arc::new(InMemoryUserRepository::new());
    let room_repo = Arc::new(InMemoryChatRoomRepository::new());
    let member_repo = Arc::new(InMemoryRoomMemberRepository::new());
    let message_repo = Arc::new(InMemoryMessageRepository::new());
    let clock = Arc::new(SystemClock::default());
    let password_hasher = Arc::new(PlainPasswordHasher);
    let broadcaster = Arc::new(NoopBroadcaster);

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
        broadcaster,
    }));

    let state = AppState::new(user_service, chat_service);
    router(state)
}

async fn send_request(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("request");
    let status = response.status();
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    (status, body)
}

#[tokio::test]
async fn user_to_message_flow() {
    let app = test_router();

    let (status, owner_body) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "username": "owner",
                    "email": "owner@example.com",
                    "password": "secret"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let owner_id = owner_body["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    let (status, member_body) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "username": "member",
                    "email": "member@example.com",
                    "password": "secret"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let member_id = member_body["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    let (status, _) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "owner@example.com",
                    "password": "secret"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, room_body) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/rooms")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "name": "general",
                    "owner_id": owner_id,
                    "visibility": "Public"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let room_id = room_body["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    let (status, _) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/rooms/{room_id}/join"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "user_id": member_id
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, message_body) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/rooms/{room_id}/messages"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "sender_id": member_id,
                    "content": "hello",
                    "message_type": "Text"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(message_body["content"], "hello");

    let (status, history_body) = send_request(
        &app,
        Request::builder()
            .method("GET")
            .uri(format!("/api/v1/rooms/{room_id}/messages"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let messages = history_body.as_array().expect("array");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["content"], "hello");
}
