mod support;

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde_json::json;
use tokio::{net::TcpListener, sync::oneshot, time::sleep};
use tokio_tungstenite::connect_async;
use uuid::Uuid;

use support::build_router;

#[tokio::test]
async fn websocket_broadcast_flow() {
    let router = build_router();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    // allow server to start
    sleep(Duration::from_millis(100)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // Register owner and member, create room, join, send message
    let _owner = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "owner",
            "email": "owner@example.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register owner")
        .json::<serde_json::Value>()
        .await
        .expect("owner json");

    let member = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "member",
            "email": "member@example.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register member")
        .json::<serde_json::Value>()
        .await
        .expect("member json");
    let member_id = member["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // Owner登录获取token
    let owner_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "owner@example.com", "password": "secret"}))
        .send()
        .await
        .expect("login owner")
        .json::<serde_json::Value>()
        .await
        .expect("owner login json");
    let owner_token = owner_login["token"].as_str().unwrap();

    // Member登录获取token
    let member_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "member@example.com", "password": "secret"}))
        .send()
        .await
        .expect("login member")
        .json::<serde_json::Value>()
        .await
        .expect("member login json");
    let member_token = member_login["token"].as_str().unwrap();

    let room = client
        .post(format!("{}/api/v1/rooms", base_http))
        .header("authorization", format!("Bearer {}", owner_token))
        .json(&json!({
            "name": "general",
            "visibility": "Public"
        }))
        .send()
        .await
        .expect("create room")
        .json::<serde_json::Value>()
        .await
        .expect("room json");
    let room_id = room["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // Owner邀请member加入房间
    client
        .post(format!("{}/api/v1/rooms/{}/members", base_http, room_id))
        .header("authorization", format!("Bearer {}", owner_token))
        .json(&json!({
            "invitee_id": member_id
        }))
        .send()
        .await
        .expect("invite member");

    // Connect WebSocket as member
    let ws_url = format!(
        "ws://{}/api/v1/ws?room_id={}&token={}",
        addr, room_id, member_token
    );
    let (mut ws, _) = connect_async(ws_url).await.expect("ws connect");

    // Send message via HTTP and expect it via WS
    client
        .post(format!("{}/api/v1/rooms/{}/messages", base_http, room_id))
        .header("authorization", format!("Bearer {}", member_token))
        .json(&json!({
            "content": "hello",
            "message_type": "Text"
        }))
        .send()
        .await
        .expect("send message");

    let msg = ws.next().await.expect("ws message").expect("ws text");

    match msg {
        tokio_tungstenite::tungstenite::Message::Text(payload) => {
            let json: serde_json::Value = serde_json::from_str(&payload).expect("json");
            assert_eq!(json["content"], "hello");
        }
        other => panic!("unexpected message {other:?}"),
    }

    let _ = shutdown_tx.send(());
}
