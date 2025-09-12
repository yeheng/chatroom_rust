//! 真实的 JWT 和 WebSocket 集成测试
//! 
//! 使用实际实现的组件进行集成测试

use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

// 导入我们自己的组件
use domain::services::auth_service::*;
use domain::entities::auth::*;
use infrastructure::auth::*;

// 模拟用户服务
struct MockUserService {
    users: std::collections::HashMap<String, UserAuthInfo>,
    refresh_tokens: std::collections::HashMap<String, Uuid>,
}

impl MockUserService {
    fn new() -> Self {
        let mut users = std::collections::HashMap::new();
        
        // 添加测试用户
        let user_id = Uuid::new_v4();
        users.insert("testuser".to_string(), UserAuthInfo {
            id: user_id,
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            role: UserRole::User,
            permissions: vec![Permission::SendMessage, Permission::JoinRoom],
            is_active: true,
            created_at: Utc::now(),
            last_login: None,
        });
        
        Self {
            users,
            refresh_tokens: std::collections::HashMap::new(),
        }
    }
}

#[async_trait::async_trait]
impl UserAuthService for MockUserService {
    async fn authenticate_user(&self, username: &str, password: &str) -> Result<UserAuthInfo, AuthError> {
        // 简单的密码验证（实际项目中应该使用 bcrypt）
        if password != "password123" {
            return Err(AuthError::InvalidPassword);
        }
        
        let user = self.users.get(username)
            .ok_or(AuthError::UserNotFound)?;
            
        Ok(user.clone())
    }
    
    async fn validate_refresh_token(&self, refresh_token: &str) -> Result<Uuid, AuthError> {
        let user_id = self.refresh_tokens.get(refresh_token)
            .ok_or(AuthError::InvalidRefreshToken)?;
            
        Ok(*user_id)
    }
    
    async fn revoke_refresh_token(&self, refresh_token: &str) -> Result<(), AuthError> {
        self.refresh_tokens.remove(refresh_token);
        Ok(())
    }
    
    async fn revoke_all_refresh_tokens(&self, user_id: Uuid) -> Result<(), AuthError> {
        self.refresh_tokens.retain(|_, &mut v| v != user_id);
        Ok(())
    }
    
    async fn get_user_by_id(&self, user_id: Uuid) -> Result<UserAuthInfo, AuthError> {
        for user in self.users.values() {
            if user.id == user_id {
                return Ok(user.clone());
            }
        }
        Err(AuthError::UserNotFound)
    }
    
    async fn is_user_active(&self, user_id: Uuid) -> Result<bool, AuthError> {
        let user = self.get_user_by_id(user_id).await?;
        Ok(user.is_active)
    }
    
    async fn update_last_login(&self, user_id: Uuid) -> Result<(), AuthError> {
        // 在实际实现中，这里会更新数据库
        Ok(())
    }
}

// 集成测试：完整的 JWT 认证流程
#[tokio::test]
async fn test_complete_jwt_authentication_flow() -> Result<()> {
    // 创建模拟的用户服务
    let user_service = std::sync::Arc::new(MockUserService::new());
    
    // 创建 JWT 编码器（使用测试密钥）
    let mut private_key_bytes = Vec::new();
    let mut public_key_bytes = Vec::new();
    
    // 使用测试 RSA 密钥对
    private_key_bytes.extend_from_slice(
        b"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC5v5J2H8d1g0k3
-----END PRIVATE KEY-----"
    );
    public_key_bytes.extend_from_slice(
        b"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAub+Sdh/HXYNJNwJ6f5m
-----END PUBLIC KEY-----"
    );
    
    let jwt_encoder = JwtEncoder::new(
        private_key_bytes,
        public_key_bytes,
        60, // access_token 过期时间（分钟）
        7,  // refresh_token 过期时间（天）
        "chatroom".to_string(),
        "chatroom-users".to_string(),
    );
    
    // 创建令牌黑名单服务
    let token_blacklist = std::sync::Arc::new(InMemoryTokenBlacklistService::new());
    
    // 创建 JWT 认证服务
    let auth_service = JwtAuthServiceImpl::new(
        jwt_encoder,
        token_blacklist,
        user_service.clone(),
    );
    
    // 测试 1: 用户登录
    println!("🔍 测试用户登录...");
    let credentials = LoginCredentials {
        username: "testuser".to_string(),
        password: "password123".to_string(),
    };
    
    let login_response = auth_service.login(credentials).await?;
    
    // 验证登录响应
    assert!(!login_response.access_token.is_empty());
    assert!(!login_response.refresh_token.is_empty());
    assert_eq!(login_response.token_type, "Bearer");
    assert!(login_response.expires_in > 0);
    assert_eq!(login_response.user.username, "testuser");
    assert_eq!(login_response.user.role, UserRole::User);
    assert!(login_response.user.permissions.contains(&Permission::SendMessage));
    assert!(login_response.user.permissions.contains(&Permission::JoinRoom));
    
    println!("✅ 用户登录成功");
    
    // 测试 2: 令牌验证
    println!("🔍 测试令牌验证...");
    let claims = auth_service.validate_token(&login_response.access_token).await?;
    assert_eq!(claims.username, "testuser");
    assert_eq!(claims.role, UserRole::User);
    assert!(claims.permissions.contains(&Permission::SendMessage));
    
    println!("✅ 令牌验证成功");
    
    // 测试 3: 令牌刷新
    println!("🔍 测试令牌刷新...");
    let refresh_response = auth_service.refresh_token(&login_response.refresh_token).await?;
    
    // 验证刷新后的令牌
    assert!(!refresh_response.access_token.is_empty());
    assert!(!refresh_response.refresh_token.is_empty());
    assert_eq!(refresh_response.user.username, "testuser");
    
    // 确保获得了新的访问令牌
    assert_ne!(refresh_response.access_token, login_response.access_token);
    
    println!("✅ 令牌刷新成功");
    
    // 测试 4: 用户登出
    println!("🔍 测试用户登出...");
    auth_service.logout(&refresh_response.access_token).await?;
    
    // 验证令牌已被撤销
    let result = auth_service.validate_token(&refresh_response.access_token).await;
    assert!(matches!(result, Err(AuthError::TokenRevoked)));
    
    println!("✅ 用户登出成功");
    
    // 测试 5: 权限检查
    println!("🔍 测试权限检查...");
    let user_id = login_response.user.id;
    
    // 检查用户拥有的权限
    assert!(auth_service.has_permission(user_id, Permission::SendMessage).await?);
    assert!(auth_service.has_permission(user_id, Permission::JoinRoom).await?);
    
    // 检查用户没有的权限
    assert!(!auth_service.has_permission(user_id, Permission::CreateRoom).await?);
    assert!(!auth_service.has_permission(user_id, Permission::ManageUsers).await?);
    
    // 检查多个权限
    let required_permissions = &[Permission::SendMessage, Permission::JoinRoom];
    assert!(auth_service.has_all_permissions(user_id, required_permissions).await?);
    
    let missing_permissions = &[Permission::SendMessage, Permission::CreateRoom];
    assert!(!auth_service.has_all_permissions(user_id, missing_permissions).await?);
    
    println!("✅ 权限检查成功");
    
    println!("🎉 完整的 JWT 认证流程测试通过！");
    Ok(())
}

// WebSocket 连接管理集成测试
#[tokio::test]
async fn test_websocket_connection_management() -> Result<()> {
    use infrastructure::websocket::*;
    use domain::services::websocket_service::*;
    use domain::entities::websocket::*;
    
    // 创建 WebSocket 连接管理器
    let connection_manager = InMemoryConnectionManager::new();
    
    // 创建房间管理器
    let room_manager = InMemoryRoomManager::new();
    
    // 创建消息路由器
    let message_router = InMemoryMessageRouter::new(connection_manager.clone());
    
    // 测试 1: 连接注册和注销
    println!("🔍 测试连接注册和注销...");
    
    let connection_id = Uuid::new_v4();
    let connection_info = ConnectionInfo {
        connection_id,
        user_id: Uuid::new_v4(),
        username: "testuser".to_string(),
        room_id: None,
        status: ConnectionStatus::Connected,
        connected_at: Utc::now(),
        last_active: Utc::now(),
        user_agent: "test-agent".to_string(),
        remote_addr: "127.0.0.1:8080".to_string(),
    };
    
    // 注册连接
    connection_manager.register_connection(connection_info.clone()).await?;
    
    // 验证连接已注册
    let retrieved_connection = connection_manager.get_connection(connection_id).await;
    assert!(retrieved_connection.is_some());
    assert_eq!(retrieved_connection.unwrap().username, "testuser");
    
    // 注销连接
    connection_manager.unregister_connection(connection_id).await?;
    
    // 验证连接已注销
    let retrieved_connection = connection_manager.get_connection(connection_id).await;
    assert!(retrieved_connection.is_none());
    
    println!("✅ 连接注册和注销测试通过");
    
    // 测试 2: 房间管理
    println!("🔍 测试房间管理...");
    
    let room_id = "test_integration_room";
    let owner_id = Uuid::new_v4();
    
    // 创建房间
    let room_info = room_manager.create_room(
        room_id.to_string(),
        "Test Integration Room".to_string(),
        owner_id,
        None, // 无密码
    ).await?;
    
    assert_eq!(room_info.room_id, room_id);
    assert_eq!(room_info.owner_id, owner_id);
    assert_eq!(room_info.current_users, 0);
    
    // 验证房间存在
    assert!(room_manager.room_exists(room_id).await);
    
    // 测试用户加入房间
    let user_id = Uuid::new_v4();
    room_manager.join_room(room_id, user_id, None).await?;
    
    // 验证用户在房间中
    assert!(room_manager.is_user_in_room(room_id, user_id).await);
    
    // 获取房间用户列表
    let room_users = room_manager.get_room_users(room_id).await;
    assert_eq!(room_users.len(), 1);
    assert_eq!(room_users[0].user_id, user_id);
    
    // 用户离开房间
    room_manager.leave_room(room_id, user_id).await?;
    
    // 验证用户已离开
    assert!(!room_manager.is_user_in_room(room_id, user_id).await);
    
    println!("✅ 房间管理测试通过");
    
    // 测试 3: 消息路由
    println!("🔍 测试消息路由...");
    
    // 创建多个连接
    let connection_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let mut connection_infos = Vec::new();
    
    for (i, &conn_id) in connection_ids.iter().enumerate() {
        let conn_info = ConnectionInfo {
            connection_id: conn_id,
            user_id: Uuid::new_v4(),
            username: format!("user{}", i),
            room_id: Some(room_id.to_string()),
            status: ConnectionStatus::Connected,
            connected_at: Utc::now(),
            last_active: Utc::now(),
            user_agent: "test-agent".to_string(),
            remote_addr: "127.0.0.1:8080".to_string(),
        };
        
        connection_manager.register_connection(conn_info.clone()).await?;
        connection_infos.push(conn_info);
    }
    
    // 测试向房间广播消息
    let message = WebSocketFrame {
        message_id: Uuid::new_v4(),
        message_type: "text".to_string(),
        content: serde_json::json!({"text": "Hello, Room!", "sender": "system"}),
        sender_id: None,
        room_id: Some(room_id.to_string()),
        timestamp: Utc::now(),
        metadata: None,
    };
    
    message_router.route_to_room(room_id, message.clone()).await?;
    
    // 测试向特定连接发送消息
    let target_connection_id = connection_ids[0];
    message_router.route_to_connection(target_connection_id, message.clone()).await?;
    
    // 测试向用户的所有连接发送消息
    let target_user_id = connection_infos[0].user_id;
    message_router.route_to_user(target_user_id, message.clone()).await?;
    
    // 清理连接
    for conn_info in connection_infos {
        connection_manager.unregister_connection(conn_info.connection_id).await?;
    }
    
    println!("✅ 消息路由测试通过");
    
    // 测试 4: 连接统计
    println!("🔍 测试连接统计...");
    
    let stats = connection_manager.get_stats().await;
    assert_eq!(stats.total_connections, 0); // 所有连接已注销
    assert_eq!(stats.active_connections, 0);
    
    println!("✅ 连接统计测试通过");
    
    // 测试 5: 房间统计
    println!("🔍 测试房间统计...");
    
    let room_stats = room_manager.get_room_stats(room_id).await;
    assert!(room_stats.is_some());
    
    let stats = room_stats.unwrap();
    assert_eq!(stats.room_id, room_id);
    assert_eq!(stats.active_users, 0); // 所有用户已离开
    assert_eq!(stats.total_messages, 0);
    
    println!("✅ 房间统计测试通过");
    
    println!("🎉 WebSocket 连接管理集成测试通过！");
    Ok(())
}

// 性能测试：模拟高并发场景
#[tokio::test]
async fn test_high_concurrent_connections() -> Result<()> {
    use infrastructure::websocket::*;
    use domain::services::websocket_service::*;
    use domain::entities::websocket::*;
    
    // 创建管理器
    let connection_manager = std::sync::Arc::new(InMemoryConnectionManager::new());
    let room_manager = std::sync::Arc::new(InMemoryRoomManager::new());
    let message_router = std::sync::Arc::new(InMemoryMessageRouter::new(connection_manager.clone()));
    
    // 测试参数
    const NUM_USERS: usize = 100;
    const NUM_ROOMS: usize = 10;
    const MESSAGES_PER_USER: usize = 5;
    
    println!("🔍 开始高并发测试：{} 用户，{} 房间", NUM_USERS, NUM_ROOMS);
    
    let start_time = std::time::Instant::now();
    
    // 并发创建用户和连接
    let mut handles = vec![];
    
    for user_id in 0..NUM_USERS {
        let conn_manager = connection_manager.clone();
        let room_mgr = room_manager.clone();
        let msg_router = message_router.clone();
        
        let handle = tokio::spawn(async move {
            // 创建连接
            let connection_id = Uuid::new_v4();
            let user_uuid = Uuid::new_v4();
            let room_id = format!("room_{}", user_id % NUM_ROOMS);
            
            let connection_info = ConnectionInfo {
                connection_id,
                user_id: user_uuid,
                username: format!("user_{}", user_id),
                room_id: Some(room_id.clone()),
                status: ConnectionStatus::Connected,
                connected_at: Utc::now(),
                last_active: Utc::now(),
                user_agent: "test-agent".to_string(),
                remote_addr: "127.0.0.1:8080".to_string(),
            };
            
            // 注册连接
            conn_manager.register_connection(connection_info).await.unwrap();
            
            // 加入房间
            room_mgr.join_room(&room_id, user_uuid, None).await.unwrap();
            
            // 发送消息
            for msg_id in 0..MESSAGES_PER_USER {
                let message = WebSocketFrame {
                    message_id: Uuid::new_v4(),
                    message_type: "text".to_string(),
                    content: serde_json::json!({
                        "text": format!("Message {} from user {}", msg_id, user_id),
                        "sender": format!("user_{}", user_id)
                    }),
                    sender_id: Some(user_uuid),
                    room_id: Some(room_id.clone()),
                    timestamp: Utc::now(),
                    metadata: None,
                };
                
                msg_router.route_to_room(&room_id, message).await.unwrap();
                
                // 模拟处理延迟
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
            
            // 离开房间
            room_mgr.leave_room(&room_id, user_uuid).await.unwrap();
            
            // 注销连接
            conn_manager.unregister_connection(connection_id).await.unwrap();
        });
        
        handles.push(handle);
    }
    
    // 等待所有任务完成
    for handle in handles {
        handle.await?;
    }
    
    let duration = start_time.elapsed();
    println!("⏱️ 高并发测试完成，耗时：{:?}", duration);
    
    // 验证最终状态
    let final_stats = connection_manager.get_stats().await;
    println!("📊 最终连接统计：总连接数={}，活跃连接数={}", 
             final_stats.total_connections, final_stats.active_connections);
    
    // 验证所有连接已清理
    assert_eq!(final_stats.active_connections, 0);
    
    // 验证房间统计
    for room_id in 0..NUM_ROOMS {
        let room_id_str = format!("room_{}", room_id);
        if let Some(room_stats) = room_manager.get_room_stats(&room_id_str).await {
            println!("📊 房间 {} 统计：活跃用户={}，总消息数={}", 
                     room_id, room_stats.active_users, room_stats.total_messages);
        }
    }
    
    println!("🎉 高并发测试通过！");
    Ok(())
}