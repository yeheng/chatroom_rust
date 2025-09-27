mod support;

use std::time::Duration;

use application::PresenceManager;
use domain::{RoomId, UserId};
use reqwest::Client;
use serde_json::json;
use tokio::{net::TcpListener, sync::oneshot, time::sleep};
use uuid::Uuid;

use support::setup_test_app;

#[tokio::test]
#[ignore = "requires local postgres"]
async fn logout_cleans_presence_state() {
    let test_app = setup_test_app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, test_app.router.into_make_service())
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    // 等待服务器启动
    sleep(Duration::from_millis(50)).await;

    let base_http = format!("http://{}", addr);
    let client = Client::new();

    // 注册并登录用户，获取 token
    let register_resp = client
        .post(format!("{}/api/v1/auth/register", base_http))
        .json(&json!({
            "username": "logout-user",
            "email": "logout@example.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("register user")
        .json::<serde_json::Value>()
        .await
        .expect("register json");

    let user_id = register_resp["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();

    let login_resp = client
        .post(format!("{}/api/v1/auth/login", base_http))
        .json(&json!({
            "email": "logout@example.com",
            "password": "secret"
        }))
        .send()
        .await
        .expect("login user")
        .json::<serde_json::Value>()
        .await
        .expect("login json");

    let token = login_resp["token"].as_str().unwrap();

    // 模拟用户在多个房间在线
    let presence = test_app.presence_manager.clone();
    let user_id_domain = UserId::from(user_id);
    let room_ids: Vec<RoomId> = (0..3).map(|_| RoomId::from(Uuid::new_v4())).collect();

    for room_id in &room_ids {
        presence
            .user_connected(*room_id, user_id_domain)
            .await
            .expect("user connected");
    }

    // 确认用户在线房间已记录
    let rooms_before = presence
        .get_user_rooms(user_id_domain)
        .await
        .expect("user rooms before");
    assert_eq!(rooms_before.len(), room_ids.len());

    // 调用登出接口
    let logout_status = client
        .post(format!("{}/api/v1/auth/logout", base_http))
        .header("authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("logout request")
        .status();

    assert!(logout_status.is_success(), "logout should succeed");

    // 检查用户在线状态被清理
    let rooms_after = presence
        .get_user_rooms(user_id_domain)
        .await
        .expect("user rooms after");
    assert!(rooms_after.is_empty(), "user rooms should be cleared");

    for room_id in room_ids {
        let users = presence
            .get_online_users(room_id)
            .await
            .expect("room users after logout");
        assert!(users.is_empty(), "room {:?} should be empty", room_id);
    }

    let _ = shutdown_tx.send(());
}
