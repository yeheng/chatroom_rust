//! 组织管理服务
//!
//! 提供组织层级管理的业务逻辑处理

use async_trait::async_trait;
use uuid::Uuid;

use domain::{
    organization::{Organization, OrganizationPath, OrganizationType, Position, UserOrganization},
    FeatureFlag, FeatureFlagService,
};

use crate::{ApplicationError, ApplicationResult};

/// 组织管理服务接口
#[async_trait]
pub trait OrganizationService {
    /// 创建组织
    async fn create_organization(
        &self,
        name: String,
        description: Option<String>,
        parent_id: Option<Uuid>,
        organization_type: OrganizationType,
        created_by: Uuid,
    ) -> ApplicationResult<Organization>;

    /// 获取组织信息
    async fn get_organization(&self, org_id: Uuid) -> ApplicationResult<Option<Organization>>;

    /// 更新组织信息
    async fn update_organization(
        &self,
        org_id: Uuid,
        name: Option<String>,
        description: Option<String>,
    ) -> ApplicationResult<Organization>;

    /// 删除组织（软删除）
    async fn delete_organization(&self, org_id: Uuid) -> ApplicationResult<()>;

    /// 获取子组织列表
    async fn get_child_organizations(
        &self,
        parent_id: Uuid,
    ) -> ApplicationResult<Vec<Organization>>;

    /// 获取组织层级路径
    async fn get_organization_path(&self, org_id: Uuid) -> ApplicationResult<OrganizationPath>;

    /// 创建职位
    async fn create_position(
        &self,
        name: String,
        description: Option<String>,
        organization_id: Uuid,
        level: u32,
    ) -> ApplicationResult<Position>;

    /// 获取组织的职位列表
    async fn get_organization_positions(&self, org_id: Uuid) -> ApplicationResult<Vec<Position>>;

    /// 将用户添加到组织
    async fn add_user_to_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        position_id: Option<Uuid>,
    ) -> ApplicationResult<UserOrganization>;

    /// 从组织中移除用户
    async fn remove_user_from_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> ApplicationResult<()>;

    /// 获取用户的组织列表
    async fn get_user_organizations(
        &self,
        user_id: Uuid,
    ) -> ApplicationResult<Vec<(Organization, Option<Position>)>>;

    /// 获取组织的用户列表
    async fn get_organization_users(
        &self,
        org_id: Uuid,
    ) -> ApplicationResult<Vec<(Uuid, Option<Position>)>>;
}

/// 组织管理服务实现
pub struct OrganizationServiceImpl<F, R>
where
    F: FeatureFlagService,
    R: OrganizationRepository,
{
    feature_flag_service: F,
    repository: R,
}

impl<F, R> OrganizationServiceImpl<F, R>
where
    F: FeatureFlagService,
    R: OrganizationRepository,
{
    pub fn new(feature_flag_service: F, repository: R) -> Self {
        Self {
            feature_flag_service,
            repository,
        }
    }

    /// 验证组织功能是否启用
    fn require_organizations_feature(&self) -> ApplicationResult<()> {
        if !self
            .feature_flag_service
            .is_feature_enabled(&FeatureFlag::EnableOrganizations)
        {
            return Err(ApplicationError::Validation(
                "Organizations feature is not enabled".to_string(),
            ));
        }
        Ok(())
    }

    /// 验证组织层级深度
    async fn validate_organization_depth(&self, parent_id: Option<Uuid>) -> ApplicationResult<()> {
        if let Some(parent_id) = parent_id {
            let path = self.repository.get_organization_path(parent_id).await?;
            if path.depth >= 5 {
                // 最大5层深度
                return Err(ApplicationError::Validation(
                    "Organization hierarchy too deep (max 5 levels)".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// 验证循环依赖
    async fn validate_no_circular_dependency(
        &self,
        org_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> ApplicationResult<()> {
        if let Some(parent_id) = parent_id {
            let path = self.repository.get_organization_path(parent_id).await?;
            if path.is_descendant_of(org_id) {
                return Err(ApplicationError::Validation(
                    "Circular dependency detected".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl<F, R> OrganizationService for OrganizationServiceImpl<F, R>
where
    F: FeatureFlagService + Send + Sync,
    R: OrganizationRepository + Send + Sync,
{
    async fn create_organization(
        &self,
        name: String,
        description: Option<String>,
        parent_id: Option<Uuid>,
        organization_type: OrganizationType,
        created_by: Uuid,
    ) -> ApplicationResult<Organization> {
        self.require_organizations_feature()?;

        // 验证层级深度
        self.validate_organization_depth(parent_id).await?;

        // 验证名称唯一性（在同一父级下）
        if self
            .repository
            .exists_organization_name(&name, parent_id)
            .await?
        {
            return Err(ApplicationError::Validation(
                "Organization name already exists in this level".to_string(),
            ));
        }

        let organization = Organization::new(name, description, parent_id, organization_type);

        let org_id = organization.id;
        self.repository
            .create_organization(organization.clone())
            .await?;

        // 创建组织路径记录
        let parent_path = if let Some(parent_id) = parent_id {
            self.repository.get_organization_path(parent_id).await?
        } else {
            // 根组织
            OrganizationPath::new(org_id, vec![org_id])
        };

        let org_path = parent_path.append_child(org_id);
        self.repository.create_organization_path(org_path).await?;

        Ok(organization)
    }

    async fn get_organization(&self, org_id: Uuid) -> ApplicationResult<Option<Organization>> {
        self.require_organizations_feature()?;
        self.repository.get_organization(org_id).await
    }

    async fn update_organization(
        &self,
        org_id: Uuid,
        name: Option<String>,
        description: Option<String>,
    ) -> ApplicationResult<Organization> {
        self.require_organizations_feature()?;

        let mut organization = self
            .repository
            .get_organization(org_id)
            .await?
            .ok_or_else(|| ApplicationError::Validation("^1".to_string()))?;

        // 如果更新名称，验证唯一性
        if let Some(ref new_name) = name {
            if new_name != &organization.name
                && self
                    .repository
                    .exists_organization_name(new_name, organization.parent_id)
                    .await?
            {
                return Err(ApplicationError::Validation("^1".to_string()));
            }
        }

        organization.update(name, description);
        self.repository
            .update_organization(organization.clone())
            .await?;

        Ok(organization)
    }

    async fn delete_organization(&self, org_id: Uuid) -> ApplicationResult<()> {
        self.require_organizations_feature()?;

        // 检查是否有子组织
        let children = self.repository.get_child_organizations(org_id).await?;
        if !children.is_empty() {
            return Err(ApplicationError::Validation("^1".to_string()));
        }

        // 检查是否有关联用户
        let users = self.repository.get_organization_users(org_id).await?;
        if !users.is_empty() {
            return Err(ApplicationError::Validation(
                "Cannot delete organization with associated users".to_string(),
            ));
        }

        self.repository.delete_organization(org_id).await
    }

    async fn get_child_organizations(
        &self,
        parent_id: Uuid,
    ) -> ApplicationResult<Vec<Organization>> {
        self.require_organizations_feature()?;
        self.repository.get_child_organizations(parent_id).await
    }

    async fn get_organization_path(&self, org_id: Uuid) -> ApplicationResult<OrganizationPath> {
        self.require_organizations_feature()?;
        self.repository.get_organization_path(org_id).await
    }

    async fn create_position(
        &self,
        name: String,
        description: Option<String>,
        organization_id: Uuid,
        level: u32,
    ) -> ApplicationResult<Position> {
        self.require_organizations_feature()?;

        // 验证组织存在
        self.repository
            .get_organization(organization_id)
            .await?
            .ok_or_else(|| ApplicationError::Validation("Organization not found".to_string()))?;

        // 验证职位名称唯一性（在同一组织内）
        if self
            .repository
            .exists_position_name(&name, organization_id)
            .await?
        {
            return Err(ApplicationError::Validation(
                "Position name already exists in this organization".to_string(),
            ));
        }

        let position = Position::new(name, description, organization_id, level);
        self.repository.create_position(position.clone()).await?;

        Ok(position)
    }

    async fn get_organization_positions(&self, org_id: Uuid) -> ApplicationResult<Vec<Position>> {
        self.require_organizations_feature()?;
        self.repository.get_organization_positions(org_id).await
    }

    async fn add_user_to_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        position_id: Option<Uuid>,
    ) -> ApplicationResult<UserOrganization> {
        self.require_organizations_feature()?;

        // 验证组织存在
        self.repository
            .get_organization(organization_id)
            .await?
            .ok_or_else(|| ApplicationError::Validation("Organization not found".to_string()))?;

        // 验证职位存在（如果提供）
        if let Some(pos_id) = position_id {
            let position =
                self.repository.get_position(pos_id).await?.ok_or_else(|| {
                    ApplicationError::Validation("Position not found".to_string())
                })?;

            if position.organization_id != organization_id {
                return Err(ApplicationError::Validation(
                    "Position does not belong to this organization".to_string(),
                ));
            }
        }

        // 检查用户是否已在组织中
        if self
            .repository
            .is_user_in_organization(user_id, organization_id)
            .await?
        {
            return Err(ApplicationError::Validation(
                "User is already in this organization".to_string(),
            ));
        }

        let user_org = UserOrganization::new(user_id, organization_id, position_id);
        self.repository
            .create_user_organization(user_org.clone())
            .await?;

        Ok(user_org)
    }

    async fn remove_user_from_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> ApplicationResult<()> {
        self.require_organizations_feature()?;

        self.repository
            .remove_user_from_organization(user_id, organization_id)
            .await
    }

    async fn get_user_organizations(
        &self,
        user_id: Uuid,
    ) -> ApplicationResult<Vec<(Organization, Option<Position>)>> {
        self.require_organizations_feature()?;
        self.repository.get_user_organizations(user_id).await
    }

    async fn get_organization_users(
        &self,
        org_id: Uuid,
    ) -> ApplicationResult<Vec<(Uuid, Option<Position>)>> {
        self.require_organizations_feature()?;
        self.repository.get_organization_users(org_id).await
    }
}

/// 组织存储库接口
#[async_trait]
pub trait OrganizationRepository {
    /// 创建组织
    async fn create_organization(&self, organization: Organization) -> ApplicationResult<()>;

    /// 获取组织
    async fn get_organization(&self, org_id: Uuid) -> ApplicationResult<Option<Organization>>;

    /// 更新组织
    async fn update_organization(&self, organization: Organization) -> ApplicationResult<()>;

    /// 删除组织
    async fn delete_organization(&self, org_id: Uuid) -> ApplicationResult<()>;

    /// 检查组织名称是否存在
    async fn exists_organization_name(
        &self,
        name: &str,
        parent_id: Option<Uuid>,
    ) -> ApplicationResult<bool>;

    /// 获取子组织
    async fn get_child_organizations(
        &self,
        parent_id: Uuid,
    ) -> ApplicationResult<Vec<Organization>>;

    /// 创建组织路径
    async fn create_organization_path(&self, path: OrganizationPath) -> ApplicationResult<()>;

    /// 获取组织路径
    async fn get_organization_path(&self, org_id: Uuid) -> ApplicationResult<OrganizationPath>;

    /// 创建职位
    async fn create_position(&self, position: Position) -> ApplicationResult<()>;

    /// 获取职位
    async fn get_position(&self, position_id: Uuid) -> ApplicationResult<Option<Position>>;

    /// 获取组织职位
    async fn get_organization_positions(&self, org_id: Uuid) -> ApplicationResult<Vec<Position>>;

    /// 检查职位名称是否存在
    async fn exists_position_name(
        &self,
        name: &str,
        organization_id: Uuid,
    ) -> ApplicationResult<bool>;

    /// 创建用户组织关联
    async fn create_user_organization(&self, user_org: UserOrganization) -> ApplicationResult<()>;

    /// 检查用户是否在组织中
    async fn is_user_in_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> ApplicationResult<bool>;

    /// 从组织移除用户
    async fn remove_user_from_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> ApplicationResult<()>;

    /// 获取用户组织
    async fn get_user_organizations(
        &self,
        user_id: Uuid,
    ) -> ApplicationResult<Vec<(Organization, Option<Position>)>>;

    /// 获取组织用户
    async fn get_organization_users(
        &self,
        org_id: Uuid,
    ) -> ApplicationResult<Vec<(Uuid, Option<Position>)>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{FeatureFlagService as _, FeatureFlags};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // Mock implementations for testing
    struct MockFeatureFlagService {
        flags: Arc<Mutex<FeatureFlags>>,
    }

    impl MockFeatureFlagService {
        fn new(flags: FeatureFlags) -> Self {
            Self {
                flags: Arc::new(Mutex::new(flags)),
            }
        }
    }

    impl FeatureFlagService for MockFeatureFlagService {
        fn is_feature_enabled(&self, flag: &FeatureFlag) -> bool {
            self.flags.lock().unwrap().is_enabled(flag)
        }

        fn get_feature_flags(&self) -> FeatureFlags {
            self.flags.lock().unwrap().clone()
        }

        fn update_feature_flag(&mut self, flag: FeatureFlag, enabled: bool) {
            let mut flags = self.flags.lock().unwrap();
            if enabled {
                flags.enable(flag);
            } else {
                flags.disable(flag);
            }
        }

        fn update_feature_flags(&mut self, new_flags: HashMap<FeatureFlag, bool>) {
            let mut flags = self.flags.lock().unwrap();
            for (flag, enabled) in new_flags {
                if enabled {
                    flags.enable(flag);
                } else {
                    flags.disable(flag);
                }
            }
        }
    }

    struct MockOrganizationRepository {
        organizations: Arc<Mutex<HashMap<Uuid, Organization>>>,
        paths: Arc<Mutex<HashMap<Uuid, OrganizationPath>>>,
    }

    impl MockOrganizationRepository {
        fn new() -> Self {
            Self {
                organizations: Arc::new(Mutex::new(HashMap::new())),
                paths: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl OrganizationRepository for MockOrganizationRepository {
        async fn create_organization(&self, organization: Organization) -> ApplicationResult<()> {
            self.organizations
                .lock()
                .unwrap()
                .insert(organization.id, organization);
            Ok(())
        }

        async fn get_organization(&self, org_id: Uuid) -> ApplicationResult<Option<Organization>> {
            Ok(self.organizations.lock().unwrap().get(&org_id).cloned())
        }

        async fn update_organization(&self, organization: Organization) -> ApplicationResult<()> {
            self.organizations
                .lock()
                .unwrap()
                .insert(organization.id, organization);
            Ok(())
        }

        async fn delete_organization(&self, org_id: Uuid) -> ApplicationResult<()> {
            self.organizations.lock().unwrap().remove(&org_id);
            Ok(())
        }

        async fn exists_organization_name(
            &self,
            name: &str,
            _parent_id: Option<Uuid>,
        ) -> ApplicationResult<bool> {
            Ok(self
                .organizations
                .lock()
                .unwrap()
                .values()
                .any(|org| org.name == name))
        }

        async fn get_child_organizations(
            &self,
            _parent_id: Uuid,
        ) -> ApplicationResult<Vec<Organization>> {
            Ok(Vec::new())
        }

        async fn create_organization_path(&self, path: OrganizationPath) -> ApplicationResult<()> {
            self.paths
                .lock()
                .unwrap()
                .insert(path.organization_id, path);
            Ok(())
        }

        async fn get_organization_path(&self, org_id: Uuid) -> ApplicationResult<OrganizationPath> {
            self.paths
                .lock()
                .unwrap()
                .get(&org_id)
                .cloned()
                .ok_or_else(|| {
                    ApplicationError::Validation("Organization path not found".to_string())
                })
        }

        // 其他方法的简单实现...
        async fn create_position(&self, _position: Position) -> ApplicationResult<()> {
            Ok(())
        }
        async fn get_position(&self, _position_id: Uuid) -> ApplicationResult<Option<Position>> {
            Ok(None)
        }
        async fn get_organization_positions(
            &self,
            _org_id: Uuid,
        ) -> ApplicationResult<Vec<Position>> {
            Ok(Vec::new())
        }
        async fn exists_position_name(
            &self,
            _name: &str,
            _organization_id: Uuid,
        ) -> ApplicationResult<bool> {
            Ok(false)
        }
        async fn create_user_organization(
            &self,
            _user_org: UserOrganization,
        ) -> ApplicationResult<()> {
            Ok(())
        }
        async fn is_user_in_organization(
            &self,
            _user_id: Uuid,
            _organization_id: Uuid,
        ) -> ApplicationResult<bool> {
            Ok(false)
        }
        async fn remove_user_from_organization(
            &self,
            _user_id: Uuid,
            _organization_id: Uuid,
        ) -> ApplicationResult<()> {
            Ok(())
        }
        async fn get_user_organizations(
            &self,
            _user_id: Uuid,
        ) -> ApplicationResult<Vec<(Organization, Option<Position>)>> {
            Ok(Vec::new())
        }
        async fn get_organization_users(
            &self,
            _org_id: Uuid,
        ) -> ApplicationResult<Vec<(Uuid, Option<Position>)>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn test_create_organization_with_feature_disabled() {
        let flags = FeatureFlags::new(); // 默认所有功能都关闭
        let feature_service = MockFeatureFlagService::new(flags);
        let repo = MockOrganizationRepository::new();
        let service = OrganizationServiceImpl::new(feature_service, repo);

        let result = service
            .create_organization(
                "Test Org".to_string(),
                None,
                None,
                OrganizationType::Company,
                Uuid::new_v4(),
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not enabled"));
    }

    #[tokio::test]
    async fn test_create_organization_with_feature_enabled() {
        let mut flags = FeatureFlags::new();
        flags.enable(FeatureFlag::EnableOrganizations);
        let feature_service = MockFeatureFlagService::new(flags);
        let repo = MockOrganizationRepository::new();
        let service = OrganizationServiceImpl::new(feature_service, repo);

        let result = service
            .create_organization(
                "Test Org".to_string(),
                Some("Test Description".to_string()),
                None,
                OrganizationType::Company,
                Uuid::new_v4(),
            )
            .await;

        assert!(result.is_ok());
        let org = result.unwrap();
        assert_eq!(org.name, "Test Org");
        assert_eq!(org.organization_type, OrganizationType::Company);
    }
}
