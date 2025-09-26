mod support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use support::build_router;

async fn send_request(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
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
    let app = build_router().await;

    let (status, _owner_body) = send_request(
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

    // Owner登录获取token
    let (status, owner_login_body) = send_request(
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
    let owner_token = owner_login_body["token"].as_str().unwrap();

    // Member登录获取token
    let (status, member_login_body) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "member@example.com",
                    "password": "secret"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let member_token = member_login_body["token"].as_str().unwrap();

    let (status, room_body) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/v1/rooms")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", owner_token))
            .body(Body::from(
                json!({
                    "name": "general",
                    "visibility": "Public"
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let room_id = room_body["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // Owner邀请member加入房间
    let (status, _) = send_request(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/api/v1/rooms/{room_id}/members"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", owner_token))
            .body(Body::from(
                json!({
                    "invitee_id": member_id
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
            .header("authorization", format!("Bearer {}", member_token))
            .body(Body::from(
                json!({
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
            .header("authorization", format!("Bearer {}", member_token))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let messages = history_body.as_array().expect("array");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["content"], "hello");
}
