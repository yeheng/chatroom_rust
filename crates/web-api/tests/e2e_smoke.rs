use reqwest::StatusCode;
use std::net::TcpListener;
use web_api::ChatRoomApp;

#[tokio::test]
async fn health_and_auth_smoke() {
    // choose a random free port and set env for config loader
    let sock = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = sock.local_addr().unwrap().port();
    drop(sock);
    std::env::set_var("APP_SERVER__PORT", port.to_string());
    let app = ChatRoomApp::new().await.unwrap();
    // spawn server
    let shutdown = app.get_shutdown_signal();
    let handle = tokio::spawn(async move {
        let _ = app.run().await;
    });
    // wait a bit
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{}", port);
    let health = client.get(format!("{}/health", base)).send().await.unwrap();
    assert_eq!(health.status(), StatusCode::OK);

    // register
    let reg = client.post(format!("{}/api/auth/register", base))
        .json(&serde_json::json!({"username":"e2euser","email":"e2e@example.com","password":"Abc12345"}))
        .send().await.unwrap();
    assert_eq!(reg.status(), StatusCode::CREATED);

    // login
    let login = client
        .post(format!("{}/api/auth/login", base))
        .json(&serde_json::json!({"username":"e2euser","password":"Abc12345"}))
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);

    // shutdown
    let _ = shutdown.send(());
    let _ = handle.abort();
}
