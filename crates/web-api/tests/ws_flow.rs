mod support;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::json;
use tokio::{net::TcpListener, sync::oneshot, time::sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message as TungsteniteMessage};
use uuid::Uuid;

use support::build_router;

#[tokio::test]
async fn websocket_broadcast_flow() {
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

    // allow server to start
    sleep(Duration::from_millis(100)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // Register owner and member, create room, join, send message
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let _owner = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": format!("owner_{}", timestamp),
            "email": format!("owner_{}@example.com", timestamp),
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
            "username": format!("member_{}", timestamp),
            "email": format!("member_{}@example.com", timestamp),
            "password": "secret"
        }))
        .send()
        .await
        .expect("register member")
        .json::<serde_json::Value>()
        .await
        .expect("member json");
    let member_id = member["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("ERROR: Member registration failed. Response: {:?}", member);
            panic!("Member registration failed - no id field in response")
        })
        .parse::<Uuid>()
        .unwrap();

    // Owner登录获取token
    let owner_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": format!("owner_{}@example.com", timestamp), "password": "secret"}))
        .send()
        .await
        .expect("login owner")
        .json::<serde_json::Value>()
        .await
        .expect("owner login json");
    let owner_token = owner_login["token"].as_str().unwrap_or_else(|| {
        eprintln!("ERROR: Owner login failed. Response: {:?}", owner_login);
        panic!("Owner login failed - no token field in response")
    });

    // Member登录获取token
    let member_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": format!("member_{}@example.com", timestamp), "password": "secret"}))
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

#[tokio::test]
async fn websocket_ping_pong_flow() {
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

    // allow server to start
    sleep(Duration::from_millis(100)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // 注册用户并获取token
    let user = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register user")
        .json::<serde_json::Value>()
        .await
        .expect("user json");

    let user_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "test@example.com", "password": "secret"}))
        .send()
        .await
        .expect("login user")
        .json::<serde_json::Value>()
        .await
        .expect("user login json");
    let user_token = user_login["token"].as_str().unwrap();

    // 创建房间
    let room = client
        .post(format!("{}/api/v1/rooms", base_http))
        .header("authorization", format!("Bearer {}", user_token))
        .json(&json!({
            "name": "ping_pong_test",
            "visibility": "Public"
        }))
        .send()
        .await
        .expect("create room")
        .json::<serde_json::Value>()
        .await
        .expect("room json");
    let room_id = room["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 连接WebSocket
    let ws_url = format!(
        "ws://{}/api/v1/ws?room_id={}&token={}",
        addr, room_id, user_token
    );
    let (mut ws, _) = connect_async(ws_url).await.expect("ws connect");

    // 发送ping消息
    let ping_data = b"test ping data";
    ws.send(TungsteniteMessage::Ping(ping_data.to_vec().into()))
        .await
        .expect("send ping");

    // 等待pong回应
    let timeout = tokio::time::timeout(Duration::from_secs(5), ws.next()).await;

    match timeout {
        Ok(Some(Ok(msg))) => {
            match msg {
                TungsteniteMessage::Pong(data) => {
                    assert_eq!(data.as_ref(), ping_data, "Pong data should match ping data");
                    println!("✅ Ping/Pong test passed: received correct pong response");
                }
                other => panic!("Expected Pong message, got: {:?}", other),
            }
        }
        Ok(Some(Err(e))) => panic!("WebSocket error: {:?}", e),
        Ok(None) => panic!("WebSocket closed unexpectedly"),
        Err(_) => panic!("Timeout waiting for pong response"),
    }

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn websocket_multiple_users_broadcast() {
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

    sleep(Duration::from_millis(100)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // 注册三个用户
    let owner = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "owner",
            "email": "owner@test.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register owner")
        .json::<serde_json::Value>()
        .await
        .expect("owner json");

    let user1 = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "user1",
            "email": "user1@test.com",
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
            "email": "user2@test.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register user2")
        .json::<serde_json::Value>()
        .await
        .expect("user2 json");
    let user2_id = user2["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 登录所有用户获取token
    let owner_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "owner@test.com", "password": "secret"}))
        .send()
        .await
        .expect("login owner")
        .json::<serde_json::Value>()
        .await
        .expect("owner login json");
    let owner_token = owner_login["token"].as_str().unwrap();

    let user1_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "user1@test.com", "password": "secret"}))
        .send()
        .await
        .expect("login user1")
        .json::<serde_json::Value>()
        .await
        .expect("user1 login json");
    let user1_token = user1_login["token"].as_str().unwrap();

    let user2_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "user2@test.com", "password": "secret"}))
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
        .header("authorization", format!("Bearer {}", owner_token))
        .json(&json!({
            "name": "multi-user-room",
            "visibility": "Public"
        }))
        .send()
        .await
        .expect("create room")
        .json::<serde_json::Value>()
        .await
        .expect("room json");
    let room_id = room["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 邀请用户加入房间
    client
        .post(format!("{}/api/v1/rooms/{}/members", base_http, room_id))
        .header("authorization", format!("Bearer {}", owner_token))
        .json(&json!({
            "invitee_id": user1_id
        }))
        .send()
        .await
        .expect("invite user1");

    client
        .post(format!("{}/api/v1/rooms/{}/members", base_http, room_id))
        .header("authorization", format!("Bearer {}", owner_token))
        .json(&json!({
            "invitee_id": user2_id
        }))
        .send()
        .await
        .expect("invite user2");

    // 建立WebSocket连接
    let ws_url1 = format!(
        "ws://{}/api/v1/ws?room_id={}&token={}",
        addr, room_id, user1_token
    );
    let (mut ws1, _) = connect_async(ws_url1).await.expect("ws1 connect");

    let ws_url2 = format!(
        "ws://{}/api/v1/ws?room_id={}&token={}",
        addr, room_id, user2_token
    );
    let (mut ws2, _) = connect_async(ws_url2).await.expect("ws2 connect");

    // user1发送消息
    println!("DEBUG: user1 准备发送消息 'Hello from user1'");
    client
        .post(format!("{}/api/v1/rooms/{}/messages", base_http, room_id))
        .header("authorization", format!("Bearer {}", user1_token))
        .json(&json!({
            "content": "Hello from user1",
            "message_type": "Text"
        }))
        .send()
        .await
        .expect("send message from user1");
    println!("DEBUG: user1 消息发送完成");

    // 等待一下确保消息被处理
    sleep(Duration::from_millis(100)).await;

    // user2应该收到消息
    println!("DEBUG: 等待 user2 接收消息...");
    let msg2 = ws2.next().await.expect("ws2 message").expect("ws2 text");
    match msg2 {
        TungsteniteMessage::Text(payload) => {
            let json: serde_json::Value = serde_json::from_str(&payload).expect("json");
            println!("DEBUG: user2 收到消息: {}", payload);
            assert_eq!(json["content"], "Hello from user1");
            assert_eq!(json["sender_id"], user1_id.to_string());
        }
        other => panic!("unexpected message {other:?}"),
    }

    // 等待一下再发送下一条消息
    sleep(Duration::from_millis(100)).await;

    // user2发送消息
    client
        .post(format!("{}/api/v1/rooms/{}/messages", base_http, room_id))
        .header("authorization", format!("Bearer {}", user2_token))
        .json(&json!({
            "content": "Hello from user2",
            "message_type": "Text"
        }))
        .send()
        .await
        .expect("send message from user2");

    // user1应该收到消息 - 需要跳过自己发送的第一条消息
    let msg1 = ws1.next().await.expect("ws1 message").expect("ws1 text");
    match msg1 {
        TungsteniteMessage::Text(payload) => {
            let json: serde_json::Value = serde_json::from_str(&payload).expect("json");
            // 如果这是user1自己发送的消息，跳过它，读取下一条
            if json["content"] == "Hello from user1" && json["sender_id"] == user1_id.to_string() {
                let msg2 = ws1.next().await.expect("ws1 message2").expect("ws1 text2");
                match msg2 {
                    TungsteniteMessage::Text(payload2) => {
                        let json2: serde_json::Value = serde_json::from_str(&payload2).expect("json2");
                        assert_eq!(json2["content"], "Hello from user2");
                        assert_eq!(json2["sender_id"], user2_id.to_string());
                    }
                    other => panic!("unexpected second message {other:?}"),
                }
            } else {
                // 如果第一条不是user1自己发送的消息，直接检查它
                assert_eq!(json["content"], "Hello from user2");
                assert_eq!(json["sender_id"], user2_id.to_string());
            }
        }
        other => panic!("unexpected message {other:?}"),
    }

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn websocket_authentication_failure() {
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

    sleep(Duration::from_millis(100)).await;

    let room_id = Uuid::new_v4();

    // 尝试使用无效的token连接WebSocket
    let ws_url = format!(
        "ws://{}/api/v1/ws?room_id={}&token=invalid-token",
        addr, room_id
    );

    let result = connect_async(ws_url).await;
    assert!(result.is_err(), "WebSocket connection should fail with invalid token");

    // 尝试连接不带token的WebSocket
    let ws_url_no_token = format!(
        "ws://{}/api/v1/ws?room_id={}",
        addr, room_id
    );

    let result = connect_async(ws_url_no_token).await;
    assert!(result.is_err(), "WebSocket connection should fail without token");

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn websocket_private_room_flow() {
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

    sleep(Duration::from_millis(100)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // 注册用户 - 使用唯一用户名避免测试间冲突
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let owner = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": format!("private_owner_{}", timestamp),
            "email": format!("private_owner_{}@example.com", timestamp),
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
            "username": format!("private_member_{}", timestamp),
            "email": format!("private_member_{}@example.com", timestamp),
            "password": "secret"
        }))
        .send()
        .await
        .expect("register member")
        .json::<serde_json::Value>()
        .await
        .expect("member json");
    let member_id = member["id"]
        .as_str()
        .unwrap_or_else(|| {
            eprintln!("ERROR: Private room member registration failed. Response: {:?}", member);
            panic!("Member registration failed - no id field in response")
        })
        .parse::<Uuid>()
        .unwrap();

    // 登录
    let owner_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": format!("private_owner_{}@example.com", timestamp), "password": "secret"}))
        .send()
        .await
        .expect("login owner")
        .json::<serde_json::Value>()
        .await
        .expect("owner login json");
    let owner_token = owner_login["token"].as_str().unwrap_or_else(|| {
        eprintln!("ERROR: Private room owner login failed. Response: {:?}", owner_login);
        panic!("Private room owner login failed - no token field in response")
    });

    let member_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": format!("private_member_{}@example.com", timestamp), "password": "secret"}))
        .send()
        .await
        .expect("login member")
        .json::<serde_json::Value>()
        .await
        .expect("member login json");
    let member_token = member_login["token"].as_str().unwrap();

    // 创建私有房间
    let room = client
        .post(format!("{}/api/v1/rooms", base_http))
        .header("authorization", format!("Bearer {}", owner_token))
        .json(&json!({
            "name": "private-room",
            "visibility": "Private",
            "password": "room-secret"
        }))
        .send()
        .await
        .expect("create private room")
        .json::<serde_json::Value>()
        .await
        .expect("room json");
    let room_id = room["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 邀请成员加入私有房间（需要密码）
    client
        .post(format!("{}/api/v1/rooms/{}/members", base_http, room_id))
        .header("authorization", format!("Bearer {}", owner_token))
        .json(&json!({
            "invitee_id": member_id,
            "password": "room-secret"
        }))
        .send()
        .await
        .expect("invite member to private room");

    // 建立WebSocket连接
    let ws_url = format!(
        "ws://{}/api/v1/ws?room_id={}&token={}",
        addr, room_id, member_token
    );
    let (mut ws, _) = connect_async(ws_url).await.expect("ws connect to private room");

    // 发送消息
    client
        .post(format!("{}/api/v1/rooms/{}/messages", base_http, room_id))
        .header("authorization", format!("Bearer {}", member_token))
        .json(&json!({
            "content": "Secret message",
            "message_type": "Text"
        }))
        .send()
        .await
        .expect("send message to private room");

    // 验证消息接收
    let msg = ws.next().await.expect("ws message").expect("ws text");
    match msg {
        TungsteniteMessage::Text(payload) => {
            let json: serde_json::Value = serde_json::from_str(&payload).expect("json");
            assert_eq!(json["content"], "Secret message");
        }
        other => panic!("unexpected message {other:?}"),
    }

    let _ = shutdown_tx.send(());
}

#[tokio::test]
async fn websocket_message_history_flow() {
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

    sleep(Duration::from_millis(100)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // 注册用户
    let user = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "historyuser",
            "email": "history@test.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register user")
        .json::<serde_json::Value>()
        .await
        .expect("user json");

    // 登录
    let user_login = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({"email": "history@test.com", "password": "secret"}))
        .send()
        .await
        .expect("login user")
        .json::<serde_json::Value>()
        .await
        .expect("user login json");
    let user_token = user_login["token"].as_str().unwrap();

    // 创建房间
    let room = client
        .post(format!("{}/api/v1/rooms", base_http))
        .header("authorization", format!("Bearer {}", user_token))
        .json(&json!({
            "name": "history-test",
            "visibility": "Public"
        }))
        .send()
        .await
        .expect("create room")
        .json::<serde_json::Value>()
        .await
        .expect("room json");
    let room_id = room["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 发送多条消息测试持久化存储
    for i in 1..=5 {
        client
            .post(format!("{}/api/v1/rooms/{}/messages", base_http, room_id))
            .header("authorization", format!("Bearer {}", user_token))
            .json(&json!({
                "content": format!("Message {}", i),
                "message_type": "Text"
            }))
            .send()
            .await
            .expect(&format!("send message {}", i));
    }

    // 获取消息历史（验证持久化存储）
    let history_response = client
        .get(format!("{}/api/v1/rooms/{}/messages", base_http, room_id))
        .header("authorization", format!("Bearer {}", user_token))
        .send()
        .await
        .expect("get history");

    assert_eq!(history_response.status(), 200);
    let history: Vec<serde_json::Value> = history_response
        .json()
        .await
        .expect("parse history json");

    // 验证历史消息
    assert_eq!(history.len(), 5);

    // 消息应该按时间倒序返回（最新的在前面）
    for (index, message) in history.iter().enumerate() {
        let expected_content = format!("Message {}", 5 - index);
        assert_eq!(message["content"], expected_content);
    }

    // 测试分页获取历史
    let first_message_id = history[0]["id"].as_str().unwrap();
    let paginated_response = client
        .get(format!(
            "{}/api/v1/rooms/{}/messages?before={}&limit=2",
            base_http, room_id, first_message_id
        ))
        .header("authorization", format!("Bearer {}", user_token))
        .send()
        .await
        .expect("get paginated history");

    let paginated_history: Vec<serde_json::Value> = paginated_response
        .json()
        .await
        .expect("parse paginated history json");

    assert_eq!(paginated_history.len(), 2);

    let _ = shutdown_tx.send(());
}
