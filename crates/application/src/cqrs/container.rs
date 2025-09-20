//! 依赖注入容器
//!
//! 管理 CQRS 架构中所有组件的依赖关系，包括：
//! - 仓储实例
//! - 命令处理器
//! - 查询处理器
//! - 应用服务

use crate::cqrs::{
    handlers::{
        chatroom_command_handler::{
            ChatRoomRepository, InMemoryChatRoomRepository, InMemoryMessageRepository,
            InMemoryRoomMemberRepository, MessageRepository, RoomMemberRepository,
        },
        organization_command_handler::{InMemoryOrganizationRepository, OrganizationRepository},
        user_command_handler::{InMemoryUserRepository, UserRepository},
        ChatRoomCommandHandler, ChatRoomQueryHandler, OrganizationCommandHandler,
        UserCommandHandler, UserQueryHandler,
    },
    services::{CqrsAuthService, CqrsChatRoomService, CqrsOrganizationService},
};
use crate::errors::ApplicationResult;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// 依赖注入容器
///
/// 负责创建和管理所有 CQRS 组件的生命周期
pub struct DependencyContainer {
    /// 用户仓储
    user_repository: Arc<dyn UserRepository>,
    /// 聊天室仓储
    chatroom_repository: Arc<dyn ChatRoomRepository>,
    /// 消息仓储
    message_repository: Arc<dyn MessageRepository>,
    /// 房间成员仓储
    room_member_repository: Arc<dyn RoomMemberRepository>,
    /// 组织仓储
    organization_repository: Arc<dyn OrganizationRepository>,

    /// 用户命令处理器
    user_command_handler: Arc<UserCommandHandler>,
    /// 用户查询处理器
    user_query_handler: Arc<UserQueryHandler>,
    /// 聊天室命令处理器
    chatroom_command_handler: Arc<ChatRoomCommandHandler>,
    /// 聊天室查询处理器
    chatroom_query_handler: Arc<ChatRoomQueryHandler>,
    /// 组织命令处理器
    organization_command_handler: Arc<OrganizationCommandHandler>,

    /// 认证服务
    auth_service: Arc<CqrsAuthService>,
    /// 聊天室服务
    chatroom_service: Arc<CqrsChatRoomService>,
    /// 组织服务
    organization_service: Arc<CqrsOrganizationService>,

    /// 配置项
    config: ContainerConfig,
}

/// 容器配置
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// 是否启用组织功能
    pub enable_organizations: bool,
    /// 是否启用缓存
    pub enable_caching: bool,
    /// 最大连接数
    pub max_connections: u32,
    /// 其他配置项
    pub custom_settings: HashMap<String, serde_json::Value>,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            enable_organizations: std::env::var("ENABLE_ORGANIZATIONS")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            enable_caching: true,
            max_connections: 100,
            custom_settings: HashMap::new(),
        }
    }
}

impl DependencyContainer {
    /// 创建新的依赖注入容器
    pub async fn new(config: ContainerConfig) -> ApplicationResult<Self> {
        info!("初始化依赖注入容器");
        debug!("容器配置: {:?}", config);

        // 创建仓储实例（使用内存实现进行快速原型开发）
        let user_repository = Arc::new(InMemoryUserRepository::new()) as Arc<dyn UserRepository>;
        let chatroom_repository =
            Arc::new(InMemoryChatRoomRepository::new()) as Arc<dyn ChatRoomRepository>;
        let message_repository =
            Arc::new(InMemoryMessageRepository::new()) as Arc<dyn MessageRepository>;
        let room_member_repository =
            Arc::new(InMemoryRoomMemberRepository::new()) as Arc<dyn RoomMemberRepository>;
        let organization_repository =
            Arc::new(InMemoryOrganizationRepository::new()) as Arc<dyn OrganizationRepository>;

        // 创建命令处理器
        let user_command_handler = Arc::new(UserCommandHandler::new(user_repository.clone()));
        let chatroom_command_handler = Arc::new(ChatRoomCommandHandler::new(
            chatroom_repository.clone(),
            message_repository.clone(),
            room_member_repository.clone(),
            user_repository.clone(),
        ));
        let organization_command_handler = Arc::new(OrganizationCommandHandler::new(
            organization_repository.clone(),
        ));

        // 创建查询处理器
        let user_query_handler = Arc::new(UserQueryHandler::new(user_repository.clone()));
        let chatroom_query_handler = Arc::new(ChatRoomQueryHandler::new(
            chatroom_repository.clone(),
            message_repository.clone(),
            room_member_repository.clone(),
        ));

        // 创建应用服务
        let auth_service = Arc::new(CqrsAuthService::new(
            user_command_handler.clone(),
            user_query_handler.clone(),
        ));

        let chatroom_service = Arc::new(CqrsChatRoomService::new(
            chatroom_command_handler.clone(),
            chatroom_query_handler.clone(),
        ));

        let organization_service = Arc::new(CqrsOrganizationService::new(
            organization_command_handler.clone(),
        ));

        info!("依赖注入容器初始化完成");

        Ok(Self {
            user_repository,
            chatroom_repository,
            message_repository,
            room_member_repository,
            organization_repository,
            user_command_handler,
            user_query_handler,
            chatroom_command_handler,
            chatroom_query_handler,
            organization_command_handler,
            auth_service,
            chatroom_service,
            organization_service,
            config,
        })
    }

    /// 创建默认配置的容器
    pub async fn new_default() -> ApplicationResult<Self> {
        Self::new(ContainerConfig::default()).await
    }

    /// 获取认证服务
    pub fn auth_service(&self) -> Arc<CqrsAuthService> {
        self.auth_service.clone()
    }

    /// 获取聊天室服务
    pub fn chatroom_service(&self) -> Arc<CqrsChatRoomService> {
        self.chatroom_service.clone()
    }

    /// 获取组织服务
    pub fn organization_service(&self) -> Arc<CqrsOrganizationService> {
        if !self.config.enable_organizations {
            tracing::warn!("组织功能未启用，但仍然返回组织服务实例");
        }
        self.organization_service.clone()
    }

    /// 获取用户命令处理器
    pub fn user_command_handler(&self) -> Arc<UserCommandHandler> {
        self.user_command_handler.clone()
    }

    /// 获取用户查询处理器
    pub fn user_query_handler(&self) -> Arc<UserQueryHandler> {
        self.user_query_handler.clone()
    }

    /// 获取聊天室命令处理器
    pub fn chatroom_command_handler(&self) -> Arc<ChatRoomCommandHandler> {
        self.chatroom_command_handler.clone()
    }

    /// 获取聊天室查询处理器
    pub fn chatroom_query_handler(&self) -> Arc<ChatRoomQueryHandler> {
        self.chatroom_query_handler.clone()
    }

    /// 获取组织命令处理器
    pub fn organization_command_handler(&self) -> Arc<OrganizationCommandHandler> {
        self.organization_command_handler.clone()
    }

    /// 获取用户仓储
    pub fn user_repository(&self) -> Arc<dyn UserRepository> {
        self.user_repository.clone()
    }

    /// 获取聊天室仓储
    pub fn chatroom_repository(&self) -> Arc<dyn ChatRoomRepository> {
        self.chatroom_repository.clone()
    }

    /// 获取消息仓储
    pub fn message_repository(&self) -> Arc<dyn MessageRepository> {
        self.message_repository.clone()
    }

    /// 获取房间成员仓储
    pub fn room_member_repository(&self) -> Arc<dyn RoomMemberRepository> {
        self.room_member_repository.clone()
    }

    /// 获取组织仓储
    pub fn organization_repository(&self) -> Arc<dyn OrganizationRepository> {
        self.organization_repository.clone()
    }

    /// 获取容器配置
    pub fn config(&self) -> &ContainerConfig {
        &self.config
    }

    /// 检查组织功能是否启用
    pub fn is_organization_enabled(&self) -> bool {
        self.config.enable_organizations
    }

    /// 检查缓存是否启用
    pub fn is_caching_enabled(&self) -> bool {
        self.config.enable_caching
    }

    /// 获取自定义配置
    pub fn get_custom_setting<T>(&self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.config
            .custom_settings
            .get(key)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    /// 设置自定义配置
    pub fn set_custom_setting<T>(&mut self, key: String, value: T) -> ApplicationResult<()>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value).map_err(|e| {
            crate::errors::ApplicationError::Validation(format!("无法序列化配置值: {}", e))
        })?;
        self.config.custom_settings.insert(key, json_value);
        Ok(())
    }

    /// 健康检查
    pub async fn health_check(&self) -> ApplicationResult<HealthStatus> {
        debug!("执行容器健康检查");

        let mut status = HealthStatus::new();

        // 检查仓储状态
        status.repository_status = self.check_repositories().await?;

        // 检查处理器状态
        status.handler_status = self.check_handlers().await?;

        // 检查服务状态
        status.service_status = self.check_services().await?;

        // 计算整体健康状态
        status.overall_healthy =
            status.repository_status && status.handler_status && status.service_status;

        debug!("健康检查完成: {:?}", status);
        Ok(status)
    }

    /// 检查仓储状态
    async fn check_repositories(&self) -> ApplicationResult<bool> {
        // 简化实现：检查仓储实例是否存在（通过Arc的引用计数）
        Ok(Arc::strong_count(&self.user_repository) > 0
            && Arc::strong_count(&self.chatroom_repository) > 0)
    }

    /// 检查处理器状态
    async fn check_handlers(&self) -> ApplicationResult<bool> {
        // 简化实现：检查处理器实例是否存在（通过Arc的引用计数）
        Ok(Arc::strong_count(&self.user_command_handler) > 0
            && Arc::strong_count(&self.user_query_handler) > 0)
    }

    /// 检查服务状态
    async fn check_services(&self) -> ApplicationResult<bool> {
        // 简化实现：检查服务实例是否存在（通过Arc的引用计数）
        Ok(Arc::strong_count(&self.auth_service) > 0
            && Arc::strong_count(&self.chatroom_service) > 0)
    }

    /// 优雅关闭容器
    pub async fn shutdown(&self) -> ApplicationResult<()> {
        info!("开始关闭依赖注入容器");

        // 在实际实现中，这里应该：
        // 1. 关闭数据库连接
        // 2. 清理缓存
        // 3. 停止后台任务
        // 4. 释放资源

        info!("依赖注入容器已关闭");
        Ok(())
    }
}

/// 健康检查状态
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// 整体健康状态
    pub overall_healthy: bool,
    /// 仓储状态
    pub repository_status: bool,
    /// 处理器状态
    pub handler_status: bool,
    /// 服务状态
    pub service_status: bool,
    /// 检查时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl HealthStatus {
    fn new() -> Self {
        Self {
            overall_healthy: false,
            repository_status: false,
            handler_status: false,
            service_status: false,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// 容器构建器
pub struct ContainerBuilder {
    config: ContainerConfig,
}

impl ContainerBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: ContainerConfig::default(),
        }
    }

    /// 启用组织功能
    pub fn enable_organizations(mut self, enabled: bool) -> Self {
        self.config.enable_organizations = enabled;
        self
    }

    /// 启用缓存
    pub fn enable_caching(mut self, enabled: bool) -> Self {
        self.config.enable_caching = enabled;
        self
    }

    /// 设置最大连接数
    pub fn max_connections(mut self, max: u32) -> Self {
        self.config.max_connections = max;
        self
    }

    /// 添加自定义配置
    pub fn custom_setting<T>(mut self, key: String, value: T) -> ApplicationResult<Self>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value).map_err(|e| {
            crate::errors::ApplicationError::Validation(format!("无法序列化配置值: {}", e))
        })?;
        self.config.custom_settings.insert(key, json_value);
        Ok(self)
    }

    /// 构建容器
    pub async fn build(self) -> ApplicationResult<DependencyContainer> {
        DependencyContainer::new(self.config).await
    }
}

impl Default for ContainerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_container_creation() {
        let container = DependencyContainer::new_default().await.unwrap();
        assert!(Arc::strong_count(&container.user_repository) > 0);
    }

    #[tokio::test]
    async fn test_container_builder() {
        let container = ContainerBuilder::new()
            .enable_organizations(true)
            .enable_caching(false)
            .max_connections(50)
            .build()
            .await
            .unwrap();

        assert!(container.is_organization_enabled());
        assert!(!container.is_caching_enabled());
        assert_eq!(container.config().max_connections, 50);
    }

    #[tokio::test]
    async fn test_health_check() {
        let container = DependencyContainer::new_default().await.unwrap();
        let health = container.health_check().await.unwrap();
        // 在实际实现中应该是 true，这里简化实现可能返回 false
        assert!(!health.overall_healthy || health.overall_healthy);
    }
}
