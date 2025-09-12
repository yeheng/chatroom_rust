//! Feature Flag服务实现
//!
//! 提供Feature Flag的动态配置和查询功能

use anyhow::Result;
use std::collections::HashMap;
use std::sync::RwLock;

use domain::{FeatureFlag, FeatureFlagError, FeatureFlagService, FeatureFlags};

/// Feature Flag服务实现
pub struct FeatureFlagServiceImpl {
    /// 内存中的Feature Flags配置
    flags: RwLock<FeatureFlags>,
}

impl FeatureFlagServiceImpl {
    /// 创建新的Feature Flag服务
    pub fn new(initial_flags: FeatureFlags) -> Self {
        Self {
            flags: RwLock::new(initial_flags),
        }
    }

    /// 从环境变量加载Feature Flags
    pub fn from_env() -> Self {
        let mut flags = FeatureFlags::new();

        // 从环境变量读取配置
        if let Ok(enable_orgs) = std::env::var("ENABLE_ORGANIZATIONS") {
            if enable_orgs.to_lowercase() == "true" {
                flags.enable(FeatureFlag::EnableOrganizations);
            }
        }

        if let Ok(enable_roles) = std::env::var("ENABLE_USER_ROLES") {
            if enable_roles.to_lowercase() == "true" {
                flags.enable(FeatureFlag::EnableUserRoles);
            }
        }

        if let Ok(enable_proxy) = std::env::var("ENABLE_PROXY_SYSTEM") {
            if enable_proxy.to_lowercase() == "true" {
                flags.enable(FeatureFlag::EnableProxySystem);
            }
        }

        if let Ok(enable_bots) = std::env::var("ENABLE_BOT_MESSAGES") {
            if enable_bots.to_lowercase() == "true" {
                flags.enable(FeatureFlag::EnableBotMessages);
            }
        }

        if let Ok(enable_stats) = std::env::var("ENABLE_ONLINE_STATISTICS") {
            if enable_stats.to_lowercase() == "true" {
                flags.enable(FeatureFlag::EnableOnlineStatistics);
            }
        }

        Self::new(flags)
    }

    /// 重新加载配置
    pub fn reload_from_config(&self, config: HashMap<String, bool>) -> Result<()> {
        let new_flags = FeatureFlags::from_config(config);
        let mut flags = self
            .flags
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        *flags = new_flags;
        Ok(())
    }

    /// 启用所有Enterprise功能
    pub fn enable_all_enterprise_features(&self) -> Result<()> {
        let mut flags = self
            .flags
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        flags.enable(FeatureFlag::EnableOrganizations);
        flags.enable(FeatureFlag::EnableUserRoles);
        flags.enable(FeatureFlag::EnableProxySystem);
        flags.enable(FeatureFlag::EnableBotMessages);
        flags.enable(FeatureFlag::EnableOnlineStatistics);

        Ok(())
    }

    /// 禁用所有Enterprise功能
    pub fn disable_all_enterprise_features(&self) -> Result<()> {
        let mut flags = self
            .flags
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire write lock: {}", e))?;

        flags.disable(FeatureFlag::EnableOrganizations);
        flags.disable(FeatureFlag::EnableUserRoles);
        flags.disable(FeatureFlag::EnableProxySystem);
        flags.disable(FeatureFlag::EnableBotMessages);
        flags.disable(FeatureFlag::EnableOnlineStatistics);

        Ok(())
    }

    /// 验证功能是否可用
    pub fn require_feature(&self, flag: &FeatureFlag) -> Result<(), FeatureFlagError> {
        if !self.is_feature_enabled(flag) {
            return Err(FeatureFlagError::FeatureDisabled {
                feature_name: flag.as_str().to_string(),
            });
        }
        Ok(())
    }

    /// 检查多个功能是否都启用
    pub fn are_features_enabled(&self, flags: &[FeatureFlag]) -> bool {
        flags.iter().all(|flag| self.is_feature_enabled(flag))
    }

    /// 获取所有启用的Enterprise功能
    pub fn get_enabled_enterprise_features(&self) -> Vec<FeatureFlag> {
        let enterprise_flags = vec![
            FeatureFlag::EnableOrganizations,
            FeatureFlag::EnableUserRoles,
            FeatureFlag::EnableProxySystem,
            FeatureFlag::EnableBotMessages,
            FeatureFlag::EnableOnlineStatistics,
        ];

        enterprise_flags
            .into_iter()
            .filter(|flag| self.is_feature_enabled(flag))
            .collect()
    }
}

impl FeatureFlagService for FeatureFlagServiceImpl {
    fn is_feature_enabled(&self, flag: &FeatureFlag) -> bool {
        self.flags
            .read()
            .map(|flags| flags.is_enabled(flag))
            .unwrap_or(false)
    }

    fn get_feature_flags(&self) -> FeatureFlags {
        self.flags
            .read()
            .map(|flags| flags.clone())
            .unwrap_or_else(|_| FeatureFlags::new())
    }

    fn update_feature_flag(&mut self, flag: FeatureFlag, enabled: bool) {
        if let Ok(mut flags) = self.flags.write() {
            if enabled {
                flags.enable(flag);
            } else {
                flags.disable(flag);
            }
        }
    }

    fn update_feature_flags(&mut self, new_flags: HashMap<FeatureFlag, bool>) {
        if let Ok(mut flags) = self.flags.write() {
            for (flag, enabled) in new_flags {
                if enabled {
                    flags.enable(flag);
                } else {
                    flags.disable(flag);
                }
            }
        }
    }
}

/// Feature Flag 中间件
///
/// 用于在 Web API 层检查功能是否启用
pub struct FeatureFlagMiddleware {
    service: FeatureFlagServiceImpl,
}

impl FeatureFlagMiddleware {
    pub fn new(service: FeatureFlagServiceImpl) -> Self {
        Self { service }
    }

    /// 检查请求是否可以访问指定功能
    pub fn check_feature_access(&self, feature: &FeatureFlag) -> Result<(), FeatureFlagError> {
        self.service.require_feature(feature)
    }

    /// 获取当前启用的功能列表（用于前端显示）
    pub fn get_enabled_features_for_client(&self) -> HashMap<String, bool> {
        let flags = self.service.get_feature_flags();
        flags.to_config()
    }
}

/// Feature Flag 宏
///
/// 简化Feature Flag检查的宏
#[macro_export]
macro_rules! require_enterprise_feature {
    ($service:expr, $feature:expr) => {
        $service.require_feature(&$feature).map_err(|e| {
            tracing::warn!("Feature not enabled: {}", e);
            anyhow::anyhow!("Feature not available: {}", e)
        })?;
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_flag_service_creation() {
        let mut flags = FeatureFlags::new();
        flags.enable(FeatureFlag::EnableOrganizations);

        let service = FeatureFlagServiceImpl::new(flags);

        assert!(service.is_feature_enabled(&FeatureFlag::EnableOrganizations));
        assert!(!service.is_feature_enabled(&FeatureFlag::EnableUserRoles));
    }

    #[test]
    fn test_feature_flag_updates() {
        let flags = FeatureFlags::new();
        let mut service = FeatureFlagServiceImpl::new(flags);

        // 启用功能
        service.update_feature_flag(FeatureFlag::EnableUserRoles, true);
        assert!(service.is_feature_enabled(&FeatureFlag::EnableUserRoles));

        // 禁用功能
        service.update_feature_flag(FeatureFlag::EnableUserRoles, false);
        assert!(!service.is_feature_enabled(&FeatureFlag::EnableUserRoles));
    }

    #[test]
    fn test_require_feature() {
        let mut flags = FeatureFlags::new();
        flags.enable(FeatureFlag::EnableBotMessages);

        let service = FeatureFlagServiceImpl::new(flags);

        // 启用的功能应该通过
        assert!(service
            .require_feature(&FeatureFlag::EnableBotMessages)
            .is_ok());

        // 未启用的功能应该失败
        assert!(service
            .require_feature(&FeatureFlag::EnableProxySystem)
            .is_err());
    }

    #[test]
    fn test_enterprise_features_bulk_operations() {
        let flags = FeatureFlags::new();
        let service = FeatureFlagServiceImpl::new(flags);

        // 启用所有企业功能
        assert!(service.enable_all_enterprise_features().is_ok());
        let enabled = service.get_enabled_enterprise_features();
        assert_eq!(enabled.len(), 5);

        // 禁用所有企业功能
        assert!(service.disable_all_enterprise_features().is_ok());
        let enabled = service.get_enabled_enterprise_features();
        assert_eq!(enabled.len(), 0);
    }

    #[test]
    fn test_feature_flag_middleware() {
        let mut flags = FeatureFlags::new();
        flags.enable(FeatureFlag::EnableOrganizations);

        let service = FeatureFlagServiceImpl::new(flags);
        let middleware = FeatureFlagMiddleware::new(service);

        // 启用的功能应该允许访问
        assert!(middleware
            .check_feature_access(&FeatureFlag::EnableOrganizations)
            .is_ok());

        // 未启用的功能应该拒绝访问
        assert!(middleware
            .check_feature_access(&FeatureFlag::EnableProxySystem)
            .is_err());

        // 获取客户端功能配置
        let client_config = middleware.get_enabled_features_for_client();
        assert_eq!(client_config.get("enable_organizations"), Some(&true));
        assert_eq!(client_config.get("enable_proxy_system"), Some(&false));
    }
}
