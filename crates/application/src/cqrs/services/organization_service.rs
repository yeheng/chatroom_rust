//! 基于 CQRS 的组织应用服务
//!
//! 提供组织管理相关的高级业务逻辑（企业级功能）

use crate::cqrs::{
    commands::{
        AddUserToOrganizationCommand, CreateOrganizationCommand, DeleteOrganizationCommand,
        UpdateOrganizationCommand,
    },
    dtos::OrganizationDto,
    handlers::OrganizationCommandHandler,
    CommandHandler,
};
use crate::errors::ApplicationResult;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// 基于 CQRS 的组织应用服务
pub struct CqrsOrganizationService {
    org_command_handler: Arc<OrganizationCommandHandler>,
}

impl CqrsOrganizationService {
    /// 创建新的组织服务实例
    pub fn new(org_command_handler: Arc<OrganizationCommandHandler>) -> Self {
        Self {
            org_command_handler,
        }
    }

    /// 创建组织
    pub async fn create_organization(
        &self,
        name: String,
        description: Option<String>,
        owner_id: Uuid,
        settings: Option<serde_json::Value>,
    ) -> ApplicationResult<OrganizationDto> {
        info!("创建组织: {} (所有者: {})", name, owner_id);

        let command = CreateOrganizationCommand {
            name,
            description,
            owner_id,
            settings,
        };

        let organization = self.org_command_handler.handle(command).await?;

        Ok(OrganizationDto {
            id: organization.id,
            name: organization.name,
            description: organization.description,
            owner_id,                          // 使用传入的 owner_id，因为实体没有这个字段
            settings: serde_json::Value::Null, // 实体中没有 settings 字段
            member_count: 1,                   // 创建时只有所有者一个成员
            created_at: organization.created_at,
            updated_at: organization.updated_at,
        })
    }

    /// 更新组织信息
    pub async fn update_organization(
        &self,
        organization_id: Uuid,
        owner_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        settings: Option<serde_json::Value>,
    ) -> ApplicationResult<OrganizationDto> {
        info!("更新组织: {}", organization_id);

        let command = UpdateOrganizationCommand {
            organization_id,
            owner_id,
            name,
            description,
            settings,
        };

        let organization = self.org_command_handler.handle(command).await?;

        Ok(OrganizationDto {
            id: organization.id,
            name: organization.name,
            description: organization.description,
            owner_id,
            settings: serde_json::Value::Null,
            member_count: 1,
            created_at: organization.created_at,
            updated_at: organization.updated_at,
        })
    }

    /// 删除组织
    pub async fn delete_organization(
        &self,
        organization_id: Uuid,
        owner_id: Uuid,
    ) -> ApplicationResult<()> {
        info!("删除组织: {}", organization_id);

        let command = DeleteOrganizationCommand {
            organization_id,
            owner_id,
        };

        self.org_command_handler.handle(command).await
    }

    /// 添加用户到组织
    pub async fn add_user_to_organization(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
        role_id: Uuid,
        department_id: Option<Uuid>,
        position_id: Option<Uuid>,
    ) -> ApplicationResult<()> {
        info!("添加用户 {} 到组织 {}", user_id, organization_id);

        let command = AddUserToOrganizationCommand {
            organization_id,
            user_id,
            role_id,
            department_id,
            position_id,
        };

        self.org_command_handler.handle(command).await
    }

    /// 验证组织功能是否可用（简化版本的 feature flag 检查）
    pub fn is_organization_feature_enabled(&self) -> bool {
        // 简化实现：总是返回 true
        // 实际应该检查环境变量或配置：ENABLE_ORGANIZATIONS
        warn!("使用简化的组织功能检查，实际应该实现 feature flag");
        true
    }

    /// 验证用户是否有组织管理权限
    pub async fn verify_organization_admin(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> ApplicationResult<bool> {
        // 简化实现：暂时返回 true
        // 实际应该检查用户在组织中的角色权限
        warn!("使用简化的权限检查，实际应该查询用户在组织中的角色");
        let _organization_id = organization_id;
        let _user_id = user_id;
        Ok(true)
    }

    /// 获取组织成员数量（简化实现）
    pub async fn get_organization_member_count(
        &self,
        organization_id: Uuid,
    ) -> ApplicationResult<u32> {
        // 简化实现：返回固定值
        // 实际应该查询组织成员表统计数量
        warn!("使用简化的成员统计，实际应该查询组织成员表");
        let _organization_id = organization_id;
        Ok(1)
    }

    /// 验证组织层级深度（防止过深的组织结构）
    pub async fn validate_organization_depth(
        &self,
        parent_id: Option<Uuid>,
    ) -> ApplicationResult<bool> {
        // 简化实现：总是返回 true
        // 实际应该递归检查父组织链的深度，确保不超过配置的最大深度
        warn!("使用简化的深度检查，实际应该检查组织层级深度");
        let _parent_id = parent_id;
        Ok(true)
    }

    /// 检查组织名称是否重复
    pub async fn is_organization_name_unique(
        &self,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> ApplicationResult<bool> {
        // 简化实现：总是返回 true
        // 实际应该查询数据库检查名称唯一性
        warn!("使用简化的名称唯一性检查，实际应该查询数据库");
        let _name = name;
        let _exclude_id = exclude_id;
        Ok(true)
    }
}
