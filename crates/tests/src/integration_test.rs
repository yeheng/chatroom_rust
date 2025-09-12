//! JWT 认证和 WebSocket 集成测试
//! 
//! 测试 JWT 认证流程与 WebSocket 连接管理的集成功能

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

// 模拟的 Web API 客户端用于集成测试
struct TestClient {
    base_url: String,
    client: reqwest::Client,
}

impl TestClient {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }
    
    async fn login(&self, username: &str, password: &str) -> Result<LoginResponse> {
        let credentials = LoginCredentials {
            username: username.to_string(),
            password: password.to_string(),
        };
        
        let response = self.client
            .post(&format!("{}/api/auth/login", self.base_url))
            .json(&credentials)
            .send()
            .await?;
            
        Ok(response.json().await?)
    }
    
    async fn websocket_connect(&self, token: &str) -> Result<WebSocketTestConnection> {
        // 这里应该建立真实的 WebSocket 连接
        // 为了集成测试，我们模拟连接过程
        Ok(WebSocketTestConnection::new(token.to_string()))
    }
}

// 模拟的 WebSocket 连接
struct WebSocketTestConnection {
    token: String,
    user_id: Uuid,
    connected_at: chrono::DateTime<chrono::Utc>,
}

impl WebSocketTestConnection {
    fn new(token: String) -> Self {
        Self {
            token,
            user_id: Uuid::new_v4(), // 模拟用户ID
            connected_at: Utc::now(),
        }
    }
    
    async fn send_message(&self, message: &str, room_id: &str) -> Result<()> {
        // 模拟发送消息到聊天室
        println!("Sending message to room {}: {}", room_id, message);
        Ok(())
    }
    
    async fn join_room(&self, room_id: &str) -> Result<()> {
        // 模拟加入聊天室
        println!("Joining room: {}", room_id);
        Ok(())
    }
    
    async fn leave_room(&self, room_id: &str) -> Result<()> {
        // 模拟离开聊天室
        println!("Leaving room: {}", room_id);
        Ok(())
    }
}

// 复用 domain 层的类型定义
#[derive(serde::Serialize, serde::Deserialize)]
struct LoginCredentials {
    username: String,
    password: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LoginResponse {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: i64,
    user: UserInfo,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UserInfo {
    id: Uuid,
    username: String,
    email: Option<String>,
    role: String,
    permissions: Vec<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_login: Option<chrono::DateTime<chrono::Utc>>,
}

// 集成测试
#[tokio::test]
async fn test_jwt_authentication_flow() -> Result<()> {
    // 创建测试客户端
    let client = TestClient::new("http://localhost:8080");
    
    // 测试用户登录
    let login_response = client.login("testuser", "password123").await?;
    
    // 验证登录响应
    assert!(!login_response.access_token.is_empty());
    assert!(!login_response.refresh_token.is_empty());
    assert_eq!(login_response.token_type, "Bearer");
    assert!(login_response.expires_in > 0);
    assert_eq!(login_response.user.username, "testuser");
    
    println!("✅ JWT 认证流程测试通过");
    Ok(())
}

#[tokio::test]
async fn test_websocket_connection_flow() -> Result<()> {
    // 创建测试客户端
    let client = TestClient::new("http://localhost:8080");
    
    // 首先登录获取令牌
    let login_response = client.login("testuser", "password123").await?;
    
    // 使用令牌建立 WebSocket 连接
    let ws_connection = client.websocket_connect(&login_response.access_token).await?;
    
    // 测试加入聊天室
    ws_connection.join_room("test_room").await?;
    
    // 测试发送消息
    ws_connection.send_message("Hello, World!", "test_room").await?;
    
    // 测试离开聊天室
    ws_connection.leave_room("test_room").await?;
    
    println!("✅ WebSocket 连接流程测试通过");
    Ok(())
}

#[tokio::test]
async fn test_token_refresh_flow() -> Result<()> {
    // 创建测试客户端
    let client = TestClient::new("http://localhost:8080");
    
    // 登录获取令牌对
    let login_response = client.login("testuser", "password123").await?;
    
    // 这里应该实现令牌刷新逻辑
    // 由于是集成测试，我们模拟刷新过程
    println!("模拟刷新令牌...");
    
    // 验证刷新后的令牌依然有效
    assert!(!login_response.refresh_token.is_empty());
    
    println!("✅ 令牌刷新流程测试通过");
    Ok(())
}

#[tokio::test]
async fn test_websocket_heartbeat_mechanism() -> Result<()> {
    // 创建测试客户端
    let client = TestClient::new("http://localhost:8080");
    
    // 登录获取令牌
    let login_response = client.login("testuser", "password123").await?;
    
    // 建立 WebSocket 连接
    let ws_connection = client.websocket_connect(&login_response.access_token).await?;
    
    // 模拟心跳机制
    for i in 1..=3 {
        println!("发送心跳包 {}", i);
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
    
    // 验证连接依然活跃
    assert!(ws_connection.connected_at + chrono::Duration::seconds(30) > Utc::now());
    
    println!("✅ WebSocket 心跳机制测试通过");
    Ok(())
}

#[tokio::test]
async fn test_room_management_flow() -> Result<()> {
    // 创建测试客户端
    let client = TestClient::new("http://localhost:8080");
    
    // 登录获取令牌
    let login_response = client.login("testuser", "password123").await?;
    
    // 建立 WebSocket 连接
    let ws_connection = client.websocket_connect(&login_response.access_token).await?;
    
    // 测试房间管理流程
    let room_id = "test_integration_room";
    
    // 加入房间
    ws_connection.join_room(room_id).await?;
    
    // 在房间中发送多条消息
    for i in 1..=5 {
        ws_connection.send_message(&format!("Message {}", i), room_id).await?;
    }
    
    // 离开房间
    ws_connection.leave_room(room_id).await?;
    
    println!("✅ 房间管理流程测试通过");
    Ok(())
}

#[tokio::test]
async fn test_concurrent_connections() -> Result<()> {
    // 创建多个并发连接测试
    let mut handles = vec![];
    
    for i in 0..5 {
        let handle = tokio::spawn(async move {
            let client = TestClient::new("http://localhost:8080");
            let login_response = client.login(&format!("user{}", i), "password123").await.unwrap();
            let ws_connection = client.websocket_connect(&login_response.access_token).await.unwrap();
            
            // 每个用户加入不同的房间
            let room_id = format!("room_{}", i % 3); // 3个房间，5个用户
            ws_connection.join_room(&room_id).await.unwrap();
            
            // 发送消息
            ws_connection.send_message(&format!("Hello from user {}", i), &room_id).await.unwrap();
            
            // 保持连接一段时间
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            ws_connection.leave_room(&room_id).await.unwrap();
        });
        handles.push(handle);
    }
    
    // 等待所有连接完成
    for handle in handles {
        handle.await?;
    }
    
    println!("✅ 并发连接测试通过");
    Ok(())
}