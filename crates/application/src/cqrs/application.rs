//! CQRS 应用程序示例
//!
//! 展示如何使用依赖注入容器来构建完整的 CQRS 应用

use crate::cqrs::{ContainerBuilder, DependencyContainer};
use crate::errors::ApplicationResult;
use domain::entities::message::MessageType;
use tracing::{debug, info};
use uuid::Uuid;

/// CQRS 应用程序
///
/// 封装了整个应用的 CQRS 架构，提供统一的访问入口
pub struct CqrsApplication {
    /// 依赖注入容器
    container: DependencyContainer,
}

impl CqrsApplication {
    /// 使用默认配置创建应用
    pub async fn new_default() -> ApplicationResult<Self> {
        let container = DependencyContainer::new_default().await?;
        Ok(Self { container })
    }

    /// 使用自定义配置创建应用
    pub async fn new_with_config() -> ApplicationResult<Self> {
        let container = ContainerBuilder::new()
            .enable_organizations(true)
            .enable_caching(true)
            .max_connections(100)
            .build()
            .await?;
        Ok(Self { container })
    }

    /// 获取依赖容器
    pub fn container(&self) -> &DependencyContainer {
        &self.container
    }

    /// 应用初始化
    pub async fn initialize(&self) -> ApplicationResult<()> {
        info!("初始化 CQRS 应用程序");

        // 执行健康检查
        let health = self.container.health_check().await?;
        if !health.overall_healthy {
            tracing::warn!("应用健康检查未完全通过: {:?}", health);
        }

        info!("CQRS 应用程序初始化完成");
        Ok(())
    }

    /// 演示完整的用户注册和聊天流程
    pub async fn demo_workflow(&self) -> ApplicationResult<()> {
        info!("开始演示 CQRS 工作流程");

        // 1. 注册用户
        let user_id = self.demo_user_registration().await?;
        debug!("用户注册完成: {}", user_id);

        // 2. 创建聊天室
        let room_id = self.demo_chatroom_creation(user_id).await?;
        debug!("聊天室创建完成: {}", room_id);

        // 3. 发送消息
        let message_id = self.demo_message_sending(user_id, room_id).await?;
        debug!("消息发送完成: {}", message_id);

        // 4. 查询验证
        self.demo_query_operations(user_id, room_id).await?;

        info!("CQRS 工作流程演示完成");
        Ok(())
    }

    /// 演示用户注册
    async fn demo_user_registration(&self) -> ApplicationResult<Uuid> {
        let auth_service = self.container.auth_service();

        let auth_response = auth_service
            .register_user(
                "demo_user".to_string(),
                "demo@example.com".to_string(),
                "password123".to_string(),
                Some("演示用户".to_string()),
                None,
            )
            .await?;

        info!("用户注册成功: {}", auth_response.user.username);
        Ok(auth_response.user.id)
    }

    /// 演示聊天室创建
    async fn demo_chatroom_creation(&self, owner_id: Uuid) -> ApplicationResult<Uuid> {
        let chatroom_service = self.container.chatroom_service();

        let room = chatroom_service
            .create_room(
                "演示聊天室".to_string(),
                Some("这是一个CQRS演示聊天室".to_string()),
                owner_id,
                false,     // 公开房间
                None,      // 无密码
                Some(100), // 最多100人
            )
            .await?;

        info!("聊天室创建成功: {}", room.name);
        Ok(room.id)
    }

    /// 演示消息发送
    async fn demo_message_sending(&self, user_id: Uuid, room_id: Uuid) -> ApplicationResult<Uuid> {
        let chatroom_service = self.container.chatroom_service();

        let message = chatroom_service
            .send_message(
                room_id,
                user_id,
                "Hello, CQRS World!".to_string(),
                MessageType::Text,
                None,
            )
            .await?;

        info!("消息发送成功: {}", message.content);
        Ok(message.id)
    }

    /// 演示查询操作
    async fn demo_query_operations(&self, user_id: Uuid, room_id: Uuid) -> ApplicationResult<()> {
        let auth_service = self.container.auth_service();
        let chatroom_service = self.container.chatroom_service();

        // 查询用户信息
        if let Some(user) = auth_service.get_user(user_id).await? {
            info!("查询到用户: {} ({})", user.username, user.email);
        }

        // 查询聊天室信息
        if let Some(room) = chatroom_service.get_room(room_id).await? {
            info!(
                "查询到聊天室: {} (成员数: {})",
                room.name, room.member_count
            );
        }

        // 查询房间消息
        let messages = chatroom_service
            .get_room_messages(room_id, Some(10))
            .await?;
        info!("查询到 {} 条消息", messages.len());

        // 查询房间成员
        let members = chatroom_service.get_room_members(room_id).await?;
        info!("查询到 {} 个房间成员", members.len());

        Ok(())
    }

    /// 应用关闭
    pub async fn shutdown(&self) -> ApplicationResult<()> {
        info!("关闭 CQRS 应用程序");
        self.container.shutdown().await?;
        info!("CQRS 应用程序已关闭");
        Ok(())
    }
}

/// 应用程序工厂
pub struct ApplicationFactory;

impl ApplicationFactory {
    /// 创建开发环境应用
    pub async fn create_development_app() -> ApplicationResult<CqrsApplication> {
        let container = ContainerBuilder::new()
            .enable_organizations(false) // 开发环境关闭企业功能
            .enable_caching(true)
            .max_connections(50)
            .build()
            .await?;

        Ok(CqrsApplication { container })
    }

    /// 创建生产环境应用
    pub async fn create_production_app() -> ApplicationResult<CqrsApplication> {
        let container = ContainerBuilder::new()
            .enable_organizations(true) // 生产环境启用企业功能
            .enable_caching(true)
            .max_connections(200)
            .build()
            .await?;

        Ok(CqrsApplication { container })
    }

    /// 创建测试环境应用
    pub async fn create_test_app() -> ApplicationResult<CqrsApplication> {
        let container = ContainerBuilder::new()
            .enable_organizations(false)
            .enable_caching(false) // 测试环境关闭缓存
            .max_connections(10)
            .build()
            .await?;

        Ok(CqrsApplication { container })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[tokio::test]
    async fn test_application_creation() {
        let app = CqrsApplication::new_default().await.unwrap();
        assert!(Arc::strong_count(&app.container.user_repository()) > 0);
    }

    #[tokio::test]
    async fn test_application_initialization() {
        let app = CqrsApplication::new_default().await.unwrap();
        app.initialize().await.unwrap();
    }

    #[tokio::test]
    async fn test_demo_workflow() {
        let app = CqrsApplication::new_default().await.unwrap();
        app.initialize().await.unwrap();

        // 在测试环境中运行演示工作流
        app.demo_workflow().await.unwrap();

        app.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_factory_patterns() {
        let dev_app = ApplicationFactory::create_development_app().await.unwrap();
        assert!(!dev_app.container().is_organization_enabled());

        let prod_app = ApplicationFactory::create_production_app().await.unwrap();
        assert!(prod_app.container().is_organization_enabled());

        let test_app = ApplicationFactory::create_test_app().await.unwrap();
        assert!(!test_app.container().is_caching_enabled());
    }
}
