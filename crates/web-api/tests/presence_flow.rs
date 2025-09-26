mod support;

use std::time::Duration;

use reqwest::Client;
use serde_json::json;
use tokio::{net::TcpListener, sync::oneshot, time::sleep};
use tokio_tungstenite::connect_async;
use uuid::Uuid;

use support::build_router;

#[tokio::test]
async fn presence_management_flow() {
    let router = build_router().await;
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

    // 等待服务器启动
    sleep(Duration::from_millis(100)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // 注册两个用户
    let user1 = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "user1",
            "email": "user1@example.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register user1")
        .json::<serde_json::Value>()
        .await
        .expect("user1 json");
    let user1_id = user1["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    let user2 = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "user2",
            "email": "user2@example.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register user2")
        .json::<serde_json::Value>()
        .await
        .expect("user2 json");
    let user2_id = user2["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 用户登录获取token
    let user1_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "user1@example.com", "password": "secret"}))
        .send()
        .await
        .expect("login user1")
        .json::<serde_json::Value>()
        .await
        .expect("user1 login json");
    let user1_token = user1_login["token"].as_str().unwrap();

    let user2_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "user2@example.com", "password": "secret"}))
        .send()
        .await
        .expect("login user2")
        .json::<serde_json::Value>()
        .await
        .expect("user2 login json");
    let user2_token = user2_login["token"].as_str().unwrap();

    // 创建房间
    let room = client
        .post(format!("{}/api/v1/rooms", base_http))
        .header("authorization", format!("Bearer {}", user1_token))
        .json(&json!({
            "name": "test-room",
            "visibility": "Public"
        }))
        .send()
        .await
        .expect("create room")
        .json::<serde_json::Value>()
        .await
        .expect("room json");
    let room_id = room["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 邀请user2加入房间
    client
        .post(format!("{}/api/v1/rooms/{}/members", base_http, room_id))
        .header("authorization", format!("Bearer {}", user1_token))
        .json(&json!({
            "invitee_id": user2_id
        }))
        .send()
        .await
        .expect("invite user2");

    // 检查初始状态：房间内没有在线用户
    let online_users = client
        .get(format!("{}/api/v1/rooms/{}/online", base_http, room_id))
        .header("authorization", format!("Bearer {}", user1_token))
        .send()
        .await
        .expect("get online users")
        .json::<Vec<Uuid>>()
        .await
        .expect("online users json");

    assert_eq!(online_users.len(), 0, "初始状态下房间应该没有在线用户");

    // User1 连接WebSocket
    let ws_url1 = format!(
        "ws://{}/api/v1/ws?room_id={}&token={}",
        addr, room_id, user1_token
    );
    let (mut ws1, _) = connect_async(ws_url1).await.expect("user1 ws connect");

    // 短暂等待，让在线状态更新
    sleep(Duration::from_millis(50)).await;

    // 检查user1在线
    let online_users = client
        .get(format!("{}/api/v1/rooms/{}/online", base_http, room_id))
        .header("authorization", format!("Bearer {}", user1_token))
        .send()
        .await
        .expect("get online users")
        .json::<Vec<Uuid>>()
        .await
        .expect("online users json");

    assert_eq!(online_users.len(), 1, "user1连接后应该有1个在线用户");
    assert!(online_users.contains(&user1_id), "在线用户应该包含user1");

    // User2 也连接WebSocket
    let ws_url2 = format!(
        "ws://{}/api/v1/ws?room_id={}&token={}",
        addr, room_id, user2_token
    );
    let (mut ws2, _) = connect_async(ws_url2).await.expect("user2 ws connect");

    // 短暂等待，让在线状态更新
    sleep(Duration::from_millis(50)).await;

    // 检查两个用户都在线
    let online_users = client
        .get(format!("{}/api/v1/rooms/{}/online", base_http, room_id))
        .header("authorization", format!("Bearer {}", user1_token))
        .send()
        .await
        .expect("get online users")
        .json::<Vec<Uuid>>()
        .await
        .expect("online users json");

    assert_eq!(online_users.len(), 2, "两个用户连接后应该有2个在线用户");
    assert!(online_users.contains(&user1_id), "在线用户应该包含user1");
    assert!(online_users.contains(&user2_id), "在线用户应该包含user2");

    // 关闭user1的WebSocket连接
    ws1.close(None).await.expect("close ws1");

    // 等待连接关闭和状态清理
    sleep(Duration::from_millis(100)).await;

    // 检查只有user2在线
    let online_users = client
        .get(format!("{}/api/v1/rooms/{}/online", base_http, room_id))
        .header("authorization", format!("Bearer {}", user2_token))
        .send()
        .await
        .expect("get online users")
        .json::<Vec<Uuid>>()
        .await
        .expect("online users json");

    assert_eq!(online_users.len(), 1, "user1断开后应该只有1个在线用户");
    assert!(online_users.contains(&user2_id), "在线用户应该只包含user2");
    assert!(
        !online_users.contains(&user1_id),
        "user1应该不在在线用户列表中"
    );

    // 关闭user2的WebSocket连接
    ws2.close(None).await.expect("close ws2");

    // 等待连接关闭和状态清理
    sleep(Duration::from_millis(100)).await;

    // 检查没有在线用户
    let online_users = client
        .get(format!("{}/api/v1/rooms/{}/online", base_http, room_id))
        .header("authorization", format!("Bearer {}", user2_token))
        .send()
        .await
        .expect("get online users")
        .json::<Vec<Uuid>>()
        .await
        .expect("online users json");

    assert_eq!(online_users.len(), 0, "所有用户断开后应该没有在线用户");

    let _ = shutdown_tx.send(());
}
