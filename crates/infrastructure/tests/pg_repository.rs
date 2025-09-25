use domain::{
    ChatRoom, ChatRoomRepository, Message, MessageContent, MessageId, MessageRepository, MessageType,
    RoomId, RoomMember, RoomMemberRepository, RoomRole, User, UserEmail, UserId, UserRepository, Username,
};
use application::password::PasswordHasher;
use infrastructure::password::BcryptPasswordHasher;
use infrastructure::repository::{create_pg_pool, PgStorage};
use sqlx::migrate::Migrator;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use time::OffsetDateTime;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_repository_round_trip() {
    let node = Postgres::default().start().await.expect("start postgres");
    let port = node
        .get_host_port_ipv4(5432u16)
        .await
        .expect("port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

    let pool = create_pg_pool(&database_url, 5).await.expect("pool");
    MIGRATOR.run(&pool).await.expect("migrations");

    let storage = PgStorage::new(pool.clone());
    let hasher = BcryptPasswordHasher::default();
    let now = OffsetDateTime::now_utc();

    let password_hash = hasher
        .hash("secret-password")
        .await
        .expect("password hash");

    let mut user = User::register(
        UserId::from(Uuid::new_v4()),
        Username::parse("tester").expect("username"),
        UserEmail::parse("tester@example.com").expect("email"),
        password_hash,
        now,
    );
    user.activate(now);

    let stored_user = storage
        .user_repository
        .create(user.clone())
        .await
        .expect("store user");

    let fetched_user = storage
        .user_repository
        .find_by_email(user.email.clone())
        .await
        .expect("fetch user")
        .expect("user exists");
    assert_eq!(fetched_user.username.as_str(), "tester");

    let room = ChatRoom::new_public(
        RoomId::from(Uuid::new_v4()),
        "general",
        stored_user.id,
        now,
    )
    .expect("create room");
    let stored_room = storage
        .room_repository
        .create(room)
        .await
        .expect("store room");

    let member = RoomMember::new(stored_room.id, stored_user.id, RoomRole::Owner, now);
    storage
        .member_repository
        .upsert(member)
        .await
        .expect("upsert member");

    let message = Message::new(
        MessageId::from(Uuid::new_v4()),
        stored_room.id,
        stored_user.id,
        MessageContent::new("hello world").expect("content"),
        MessageType::Text,
        None,
        now,
    )
    .expect("message");

    storage
        .message_repository
        .create(message)
        .await
        .expect("store message");

    let history = storage
        .message_repository
        .list_recent(stored_room.id, 10, None)
        .await
        .expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].content.as_str(), "hello world");
}
