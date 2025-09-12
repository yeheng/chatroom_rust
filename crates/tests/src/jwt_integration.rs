//! JWT 认证集成测试
//! 
//! 测试 JWT 认证流程的核心功能

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
    refresh_tokens: std::sync::RwLock<std::collections::HashMap<String, Uuid>>,
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
            refresh_tokens: std::sync::RwLock::new(std::collections::HashMap::new()),
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
        let tokens = self.refresh_tokens.read().map_err(|e| AuthError::InternalError(e.to_string()))?;
        let user_id = tokens.get(refresh_token)
            .ok_or(AuthError::InvalidRefreshToken)?;
            
        Ok(*user_id)
    }
    
    async fn revoke_refresh_token(&self, refresh_token: &str) -> Result<(), AuthError> {
        let mut tokens = self.refresh_tokens.write().map_err(|e| AuthError::InternalError(e.to_string()))?;
        tokens.remove(refresh_token);
        Ok(())
    }
    
    async fn revoke_all_refresh_tokens(&self, user_id: Uuid) -> Result<(), AuthError> {
        let mut tokens = self.refresh_tokens.write().map_err(|e| AuthError::InternalError(e.to_string()))?;
        tokens.retain(|_, v| *v != user_id);
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
    
    async fn update_last_login(&self, _user_id: Uuid) -> Result<(), AuthError> {
        // 在实际实现中，这里会更新数据库
        Ok(())
    }
}

// JWT 认证流程集成测试
#[tokio::test]
async fn test_jwt_authentication_flow() -> Result<()> {
    println!("🔍 开始 JWT 认证流程集成测试");
    
    // 创建模拟的用户服务
    let user_service = std::sync::Arc::new(MockUserService::new());
    
    // 创建令牌黑名单服务
    let token_blacklist = std::sync::Arc::new(InMemoryTokenBlacklistService::new());
    
    // 测试 1: 用户认证（不依赖 JWT 编码器）
    println!("  📋 测试用户认证...");
    let credentials = LoginCredentials {
        username: "testuser".to_string(),
        password: "password123".to_string(),
    };
    
    let user_info = user_service.authenticate_user(&credentials.username, &credentials.password).await?;
    assert_eq!(user_info.username, "testuser");
    assert_eq!(user_info.role, UserRole::User);
    
    println!("  ✅ 用户认证成功");
    
    // 测试 2: 权限检查
    println!("  📋 测试权限检查...");
    let user_id = user_info.id;
    
    // 检查用户权限
    assert!(user_info.permissions.contains(&Permission::SendMessage));
    assert!(user_info.permissions.contains(&Permission::JoinRoom));
    assert!(!user_info.permissions.contains(&Permission::CreateRoom));
    assert!(!user_info.permissions.contains(&Permission::ManageUsers));
    
    println!("  ✅ 权限检查成功");
    
    // 测试 3: 用户信息获取
    println!("  📋 测试用户信息获取...");
    let retrieved_user = user_service.get_user_by_id(user_id).await?;
    assert_eq!(retrieved_user.username, "testuser");
    assert_eq!(retrieved_user.role, UserRole::User);
    
    println!("  ✅ 用户信息获取成功");
    
    // 测试 4: 用户活跃状态检查
    println!("  📋 测试用户活跃状态检查...");
    let is_active = user_service.is_user_active(user_id).await?;
    assert!(is_active);
    
    println!("  ✅ 用户活跃状态检查成功");
    
    // 测试 5: 刷新令牌管理
    println!("  📋 测试刷新令牌管理...");
    
    // 模拟添加刷新令牌
    let refresh_token = Uuid::new_v4().to_string();
    {
        let mut tokens = user_service.refresh_tokens.write().map_err(|e| AuthError::InternalError(e.to_string()))?;
        tokens.insert(refresh_token.clone(), user_id);
    }
    
    // 验证刷新令牌
    let validated_user_id = user_service.validate_refresh_token(&refresh_token).await?;
    assert_eq!(validated_user_id, user_id);
    
    // 撤销刷新令牌
    user_service.revoke_refresh_token(&refresh_token).await?;
    
    // 验证令牌已撤销
    let result = user_service.validate_refresh_token(&refresh_token).await;
    assert!(matches!(result, Err(AuthError::InvalidRefreshToken)));
    
    println!("  ✅ 刷新令牌管理测试成功");
    
    // 测试 6: 批量撤销刷新令牌
    println!("  📋 测试批量撤销刷新令牌...");
    
    // 添加多个刷新令牌
    let refresh_tokens: Vec<String> = (0..5).map(|_| Uuid::new_v4().to_string()).collect();
    for token in &refresh_tokens {
        let mut tokens = user_service.refresh_tokens.write().map_err(|e| AuthError::InternalError(e.to_string()))?;
        tokens.insert(token.clone(), user_id);
    }
    
    // 批量撤销
    user_service.revoke_all_refresh_tokens(user_id).await?;
    
    // 验证所有令牌已撤销
    for token in &refresh_tokens {
        let result = user_service.validate_refresh_token(token).await;
        assert!(matches!(result, Err(AuthError::InvalidRefreshToken)));
    }
    
    println!("  ✅ 批量撤销刷新令牌测试成功");
    
    // 测试 7: 令牌黑名单功能
    println!("  📋 测试令牌黑名单功能...");
    
    let jti = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::hours(1);
    
    // 添加令牌到黑名单
    token_blacklist.add_to_blacklist(&jti, expires_at).await?;
    
    // 检查令牌在黑名单中
    assert!(token_blacklist.is_blacklisted(&jti).await?);
    
    // 从黑名单移除
    token_blacklist.remove_from_blacklist(&jti).await?;
    
    // 检查令牌已移除
    assert!(!token_blacklist.is_blacklisted(&jti).await?);
    
    println!("  ✅ 令牌黑名单功能测试成功");
    
    // 测试 8: 更新最后登录时间
    println!("  📋 测试更新最后登录时间...");
    user_service.update_last_login(user_id).await?;
    println!("  ✅ 更新最后登录时间成功");
    
    println!("🎉 JWT 认证流程集成测试全部通过！");
    Ok(())
}

// JWT 编码器测试
#[tokio::test]
async fn test_jwt_encoder_functionality() -> Result<()> {
    println!("🔍 开始 JWT 编码器功能测试");
    
    // 创建 JWT 编码器
    let mut private_key_bytes = Vec::new();
    let mut public_key_bytes = Vec::new();
    
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
    
    // 测试 Claims 生成
    let user_id = Uuid::new_v4();
    let username = "testuser".to_string();
    let email = Some("test@example.com".to_string());
    let role = UserRole::User;
    let permissions = vec![Permission::SendMessage, Permission::JoinRoom];
    
    let claims = jwt_encoder.generate_claims(user_id, username, email, role, permissions);
    
    // 验证 Claims 内容
    assert_eq!(claims.sub, user_id.to_string());
    assert_eq!(claims.username, "testuser");
    assert_eq!(claims.email, Some("test@example.com".to_string()));
    assert_eq!(claims.role, UserRole::User);
    assert!(claims.permissions.contains(&Permission::SendMessage));
    assert!(claims.permissions.contains(&Permission::JoinRoom));
    assert_eq!(claims.iss, "chatroom");
    assert_eq!(claims.aud, "chatroom-users");
    
    println!("  ✅ Claims 生成成功");
    
    // 测试刷新令牌生成
    let refresh_token = jwt_encoder.generate_refresh_token(user_id)?;
    assert!(!refresh_token.is_empty());
    
    println!("  ✅ 刷新令牌生成成功");
    
    // 测试过期时间设置
    assert!(jwt_encoder.access_token_expire_minutes() == 60);
    assert!(jwt_encoder.refresh_token_expire_days() == 7);
    
    println!("  ✅ 过期时间设置正确");
    
    println!("🎉 JWT 编码器功能测试全部通过！");
    Ok(())
}

// 令牌黑名单服务测试
#[tokio::test]
async fn test_token_blacklist_service() -> Result<()> {
    println!("🔍 开始令牌黑名单服务测试");
    
    // 创建内存令牌黑名单服务
    let blacklist_service = InMemoryTokenBlacklistService::new();
    
    let jti = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::hours(1);
    
    // 测试添加令牌到黑名单
    blacklist_service.add_to_blacklist(&jti, expires_at).await?;
    
    // 检查令牌是否在黑名单中
    assert!(blacklist_service.is_blacklisted(&jti).await?);
    
    // 测试从黑名单移除令牌
    blacklist_service.remove_from_blacklist(&jti).await?;
    
    // 检查令牌是否已移除
    assert!(!blacklist_service.is_blacklisted(&jti).await?);
    
    println!("  ✅ 令牌黑名单基本功能测试通过");
    
    // 测试过期令牌自动清理（通过现有的 cleanup_expired_tokens 方法）
    // 这个方法在 InMemoryTokenBlacklistService 中已经实现
    let cleaned_count = blacklist_service.cleanup_expired_tokens().await?;
    assert_eq!(cleaned_count, 0); // 没有过期令牌
    
    println!("  ✅ 过期令牌自动清理测试通过");
    
    println!("🎉 令牌黑名单服务测试全部通过！");
    Ok(())
}

// 性能测试：验证认证组件性能
#[tokio::test]
async fn test_authentication_performance() -> Result<()> {
    println!("🔍 开始认证性能测试");
    
    // 创建测试用户服务
    let user_service = std::sync::Arc::new(MockUserService::new());
    
    // 创建令牌黑名单服务
    let token_blacklist = std::sync::Arc::new(InMemoryTokenBlacklistService::new());
    
    const NUM_ITERATIONS: usize = 100;
    
    // 测试用户认证性能
    let start_time = std::time::Instant::now();
    
    for _ in 0..NUM_ITERATIONS {
        let user_info = user_service.authenticate_user("testuser", "password123").await?;
        assert_eq!(user_info.username, "testuser");
    }
    
    let auth_duration = start_time.elapsed();
    println!("  📊 用户认证性能：{} 次认证耗时 {:?}", NUM_ITERATIONS, auth_duration);
    
    // 测试用户信息获取性能
    let user_id = user_service.authenticate_user("testuser", "password123").await?.id;
    
    let start_time = std::time::Instant::now();
    
    for _ in 0..NUM_ITERATIONS {
        let user_info = user_service.get_user_by_id(user_id).await?;
        assert_eq!(user_info.username, "testuser");
    }
    
    let retrieval_duration = start_time.elapsed();
    println!("  📊 用户信息获取性能：{} 次获取耗时 {:?}", NUM_ITERATIONS, retrieval_duration);
    
    // 测试权限检查性能
    let start_time = std::time::Instant::now();
    
    for _ in 0..NUM_ITERATIONS {
        let user_info = user_service.get_user_by_id(user_id).await?;
        let has_permission = user_info.permissions.contains(&Permission::SendMessage);
        assert!(has_permission);
    }
    
    let permission_duration = start_time.elapsed();
    println!("  📊 权限检查性能：{} 次检查耗时 {:?}", NUM_ITERATIONS, permission_duration);
    
    // 测试令牌黑名单操作性能
    let start_time = std::time::Instant::now();
    
    for i in 0..NUM_ITERATIONS {
        let jti = format!("token_{}", i);
        let expires_at = Utc::now() + chrono::Duration::hours(1);
        
        token_blacklist.add_to_blacklist(&jti, expires_at).await?;
        assert!(token_blacklist.is_blacklisted(&jti).await?);
        
        token_blacklist.remove_from_blacklist(&jti).await?;
        assert!(!token_blacklist.is_blacklisted(&jti).await?);
    }
    
    let blacklist_duration = start_time.elapsed();
    println!("  📊 令牌黑名单操作性能：{} 次添加/删除/检查耗时 {:?}", NUM_ITERATIONS, blacklist_duration);
    
    // 测试刷新令牌管理性能
    let start_time = std::time::Instant::now();
    
    for _ in 0..NUM_ITERATIONS {
        let refresh_token = Uuid::new_v4().to_string();
        
        // 添加刷新令牌
        {
            let mut tokens = user_service.refresh_tokens.write().map_err(|e| AuthError::InternalError(e.to_string()))?;
            tokens.insert(refresh_token.clone(), user_id);
        }
        
        // 验证刷新令牌
        let validated_user_id = user_service.validate_refresh_token(&refresh_token).await?;
        assert_eq!(validated_user_id, user_id);
        
        // 撤销刷新令牌
        user_service.revoke_refresh_token(&refresh_token).await?;
        
        // 验证令牌已撤销
        let result = user_service.validate_refresh_token(&refresh_token).await;
        assert!(matches!(result, Err(AuthError::InvalidRefreshToken)));
    }
    
    let refresh_token_duration = start_time.elapsed();
    println!("  📊 刷新令牌管理性能：{} 次添加/验证/撤销耗时 {:?}", NUM_ITERATIONS, refresh_token_duration);
    
    // 性能断言（确保每次操作不超过 1ms）
    assert!(auth_duration / NUM_ITERATIONS as u32 <= std::time::Duration::from_millis(1));
    assert!(retrieval_duration / NUM_ITERATIONS as u32 <= std::time::Duration::from_millis(1));
    assert!(permission_duration / NUM_ITERATIONS as u32 <= std::time::Duration::from_millis(1));
    assert!(blacklist_duration / NUM_ITERATIONS as u32 <= std::time::Duration::from_millis(1));
    assert!(refresh_token_duration / NUM_ITERATIONS as u32 <= std::time::Duration::from_millis(1));
    
    println!("🎉 认证性能测试全部通过！");
    Ok(())
}