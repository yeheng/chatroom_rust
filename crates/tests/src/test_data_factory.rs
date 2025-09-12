//! 测试数据工厂
//! 
//! 提供快速创建测试用户、房间、消息的工厂方法

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

use domain::{MessageType, UserStatus};
use crate::TestEnvironment;

/// 测试数据工厂
/// 
/// 提供创建各种测试数据的便捷方法
pub struct TestDataFactory {
    env: Arc<TestEnvironment>,
}

impl TestDataFactory {
    pub fn new(env: Arc<TestEnvironment>) -> Self {
        Self { env }
    }

    /// 创建测试用户
    pub async fn create_test_user(&self, username: &str) -> Result<TestUser> {
        let user = TestUser {
            id: Uuid::new_v4(),
            username: username.to_string(),
            email: format!("{}@example.com", username),
            avatar_url: None,
            status: UserStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 保存到数据库
        self.save_user_to_database(&user).await?;
        
        Ok(user)
    }

    /// 创建多个测试用户
    pub async fn create_multiple_users(&self, count: usize) -> Result<Vec<TestUser>> {
        let mut users = Vec::with_capacity(count);
        
        for i in 0..count {
            let username = format!("user_{}", i);
            let user = self.create_test_user(&username).await?;
            users.push(user);
        }
        
        Ok(users)
    }

    /// 创建测试聊天室
    pub async fn create_test_room(&self, name: &str) -> Result<TestChatRoom> {
        // 创建一个默认的房主用户
        let owner = self.create_test_user("room_owner").await?;
        
        let room = TestChatRoom {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: Some(format!("Test room: {}", name)),
            owner_id: owner.id,
            is_private: false,
            password_hash: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // 保存到数据库
        self.save_room_to_database(&room).await?;
        
        Ok(room)
    }

    /// 创建私密聊天室
    pub async fn create_private_room(&self, name: &str, password: &str) -> Result<TestChatRoom> {
        let owner = self.create_test_user("private_room_owner").await?;
        
        let room = TestChatRoom {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: Some(format!("Private test room: {}", name)),
            owner_id: owner.id,
            is_private: true,
            password_hash: Some(self.hash_password(password).await?),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.save_room_to_database(&room).await?;
        
        Ok(room)
    }

    /// 创建测试消息
    pub async fn create_test_message(
        &self, 
        room_id: Uuid, 
        user_id: Uuid, 
        content: &str
    ) -> Result<TestMessage> {
        let message = TestMessage {
            id: Uuid::new_v4(),
            room_id,
            user_id,
            content: content.to_string(),
            message_type: MessageType::Text,
            created_at: Utc::now(),
        };

        // 保存到数据库
        self.save_message_to_database(&message).await?;
        
        Ok(message)
    }

    /// 让用户加入聊天室
    pub async fn join_room(
        &self, 
        user: &TestUser, 
        room: &TestChatRoom, 
        password: Option<&str>
    ) -> Result<()> {
        // 验证密码（如果是私密房间）
        if room.is_private {
            if let Some(ref hash) = room.password_hash {
                let password = password.ok_or_else(|| {
                    anyhow::anyhow!("私密房间需要密码")
                })?;
                
                if !self.verify_password(password, hash).await? {
                    return Err(anyhow::anyhow!("密码错误"));
                }
            }
        }

        // 将用户添加到房间成员表
        self.add_room_member(user.id, room.id).await?;
        
        Ok(())
    }

    /// 用户登录并获取token
    pub async fn login_user(&self, user: &TestUser) -> Result<String> {
        // 这里应该调用实际的认证服务来生成JWT token
        // 为了测试，我们生成一个简单的token
        let token = format!("test_token_for_user_{}", user.id);
        Ok(token)
    }

    /// 创建批量测试数据
    pub async fn create_chat_scenario(
        &self,
        room_name: &str,
        user_count: usize,
        message_count: usize,
    ) -> Result<ChatScenario> {
        // 创建聊天室
        let room = self.create_test_room(room_name).await?;
        
        // 创建用户
        let users = self.create_multiple_users(user_count).await?;
        
        // 用户加入房间
        for user in &users {
            self.join_room(user, &room, None).await?;
        }
        
        // 创建消息
        let mut messages = Vec::new();
        for i in 0..message_count {
            let user = &users[i % users.len()];
            let content = format!("Test message {} from {}", i + 1, user.username);
            let message = self.create_test_message(room.id, user.id, &content).await?;
            messages.push(message);
        }
        
        Ok(ChatScenario {
            room,
            users,
            messages,
        })
    }

    // 私有辅助方法
    
    async fn save_user_to_database(&self, user: &TestUser) -> Result<()> {
        let pool = self.env.get_database_pool().await?;
        
        sqlx::query!(
            r#"
            INSERT INTO users (id, username, email, avatar_url, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            user.id,
            user.username,
            user.email,
            user.avatar_url,
            user.status.to_string(),
            user.created_at,
            user.updated_at
        )
        .execute(&pool.0)
        .await?;
        
        Ok(())
    }
    
    async fn save_room_to_database(&self, room: &TestChatRoom) -> Result<()> {
        let pool = self.env.get_database_pool().await?;
        
        sqlx::query!(
            r#"
            INSERT INTO chat_rooms (id, name, description, owner_id, is_private, password_hash, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            room.id,
            room.name,
            room.description,
            room.owner_id,
            room.is_private,
            room.password_hash,
            room.created_at,
            room.updated_at
        )
        .execute(&pool.0)
        .await?;
        
        Ok(())
    }
    
    async fn save_message_to_database(&self, message: &TestMessage) -> Result<()> {
        let pool = self.env.get_database_pool().await?;
        
        sqlx::query!(
            r#"
            INSERT INTO messages (id, room_id, user_id, content, message_type, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            message.id,
            message.room_id,
            message.user_id,
            message.content,
            message.message_type.to_string(),
            message.created_at
        )
        .execute(&pool.0)
        .await?;
        
        Ok(())
    }
    
    async fn add_room_member(&self, user_id: Uuid, room_id: Uuid) -> Result<()> {
        let pool = self.env.get_database_pool().await?;
        
        sqlx::query!(
            r#"
            INSERT INTO room_members (room_id, user_id, joined_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (room_id, user_id) DO NOTHING
            "#,
            room_id,
            user_id
        )
        .execute(&pool.0)
        .await?;
        
        Ok(())
    }
    
    async fn hash_password(&self, password: &str) -> Result<String> {
        // 使用bcrypt哈希密码
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)?;
        Ok(hash)
    }
    
    async fn verify_password(&self, password: &str, hash: &str) -> Result<bool> {
        let valid = bcrypt::verify(password, hash)?;
        Ok(valid)
    }
}

/// 测试用户数据
#[derive(Debug, Clone)]
pub struct TestUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 测试聊天室数据
#[derive(Debug, Clone)]
pub struct TestChatRoom {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_private: bool,
    pub password_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 测试消息数据
#[derive(Debug, Clone)]
pub struct TestMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub message_type: MessageType,
    pub created_at: DateTime<Utc>,
}

/// 聊天场景数据
/// 
/// 包含一个完整的聊天场景：房间、用户和消息
#[derive(Debug)]
pub struct ChatScenario {
    pub room: TestChatRoom,
    pub users: Vec<TestUser>,
    pub messages: Vec<TestMessage>,
}