//! Feature Flag系统
//!
//! 提供动态功能开关，支持企业级功能的渐进式启用

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Feature Flag标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureFlag {
    /// 启用组织层级管理
    EnableOrganizations,
    /// 启用用户角色和权限系统
    EnableUserRoles,
    /// 启用代理关系系统
    EnableProxySystem,
    /// 启用机器人消息功能
    EnableBotMessages,
    /// 启用用户在线时长统计
    EnableOnlineStatistics,
    /// 自定义Feature Flag
    Custom(String),
}

impl FeatureFlag {
    /// 获取Feature Flag的字符串标识
    pub fn as_str(&self) -> &str {
        match self {
            FeatureFlag::EnableOrganizations => "enable_organizations",
            FeatureFlag::EnableUserRoles => "enable_user_roles",
            FeatureFlag::EnableProxySystem => "enable_proxy_system",
            FeatureFlag::EnableBotMessages => "enable_bot_messages",
            FeatureFlag::EnableOnlineStatistics => "enable_online_statistics",
            FeatureFlag::Custom(name) => name,
        }
    }

    /// 从字符串创建Feature Flag
    pub fn from_str(s: &str) -> Self {
        match s {
            "enable_organizations" => FeatureFlag::EnableOrganizations,
            "enable_user_roles" => FeatureFlag::EnableUserRoles,
            "enable_proxy_system" => FeatureFlag::EnableProxySystem,
            "enable_bot_messages" => FeatureFlag::EnableBotMessages,
            "enable_online_statistics" => FeatureFlag::EnableOnlineStatistics,
            _ => FeatureFlag::Custom(s.to_string()),
        }
    }
}

/// Feature Flag配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Feature Flag状态映射
    flags: HashMap<FeatureFlag, bool>,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        let mut flags = HashMap::new();

        // 默认所有企业级功能都关闭
        flags.insert(FeatureFlag::EnableOrganizations, false);
        flags.insert(FeatureFlag::EnableUserRoles, false);
        flags.insert(FeatureFlag::EnableProxySystem, false);
        flags.insert(FeatureFlag::EnableBotMessages, false);
        flags.insert(FeatureFlag::EnableOnlineStatistics, false);

        Self { flags }
    }
}

impl FeatureFlags {
    /// 创建新的Feature Flags配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 启用指定的Feature Flag
    pub fn enable(&mut self, flag: FeatureFlag) {
        self.flags.insert(flag, true);
    }

    /// 禁用指定的Feature Flag
    pub fn disable(&mut self, flag: FeatureFlag) {
        self.flags.insert(flag, false);
    }

    /// 检查Feature Flag是否启用
    pub fn is_enabled(&self, flag: &FeatureFlag) -> bool {
        self.flags.get(flag).copied().unwrap_or(false)
    }

    /// 获取所有启用的Feature Flags
    pub fn enabled_flags(&self) -> Vec<&FeatureFlag> {
        self.flags
            .iter()
            .filter_map(|(flag, &enabled)| if enabled { Some(flag) } else { None })
            .collect()
    }

    /// 获取所有Feature Flags的状态
    pub fn all_flags(&self) -> &HashMap<FeatureFlag, bool> {
        &self.flags
    }

    /// 从配置创建Feature Flags
    pub fn from_config(config: HashMap<String, bool>) -> Self {
        let mut flags = HashMap::new();

        for (key, value) in config {
            flags.insert(FeatureFlag::from_str(&key), value);
        }

        Self { flags }
    }

    /// 转换为配置格式
    pub fn to_config(&self) -> HashMap<String, bool> {
        self.flags
            .iter()
            .map(|(flag, &enabled)| (flag.as_str().to_string(), enabled))
            .collect()
    }
}

/// Feature Flag服务接口
pub trait FeatureFlagService {
    /// 检查Feature Flag是否启用
    fn is_feature_enabled(&self, flag: &FeatureFlag) -> bool;

    /// 获取所有Feature Flags配置
    fn get_feature_flags(&self) -> FeatureFlags;

    /// 更新Feature Flag状态
    fn update_feature_flag(&mut self, flag: FeatureFlag, enabled: bool);

    /// 批量更新Feature Flags
    fn update_feature_flags(&mut self, flags: HashMap<FeatureFlag, bool>);
}

/// Feature Flag验证错误
#[derive(Debug, thiserror::Error)]
pub enum FeatureFlagError {
    #[error("功能未启用: {feature_name}")]
    FeatureDisabled { feature_name: String },

    #[error("无效的Feature Flag: {flag_name}")]
    InvalidFeatureFlag { flag_name: String },

    #[error("Feature Flag配置错误: {message}")]
    ConfigurationError { message: String },
}

/// Feature Flag验证宏
#[macro_export]
macro_rules! require_feature {
    ($flags:expr, $flag:expr) => {
        if !$flags.is_enabled(&$flag) {
            return Err($crate::feature_flags::FeatureFlagError::FeatureDisabled {
                feature_name: $flag.as_str().to_string(),
            });
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_flag_creation() {
        let mut flags = FeatureFlags::new();

        // 默认情况下所有功能都应该关闭
        assert!(!flags.is_enabled(&FeatureFlag::EnableOrganizations));
        assert!(!flags.is_enabled(&FeatureFlag::EnableUserRoles));

        // 启用功能
        flags.enable(FeatureFlag::EnableOrganizations);
        assert!(flags.is_enabled(&FeatureFlag::EnableOrganizations));
        assert!(!flags.is_enabled(&FeatureFlag::EnableUserRoles));

        // 禁用功能
        flags.disable(FeatureFlag::EnableOrganizations);
        assert!(!flags.is_enabled(&FeatureFlag::EnableOrganizations));
    }

    #[test]
    fn test_feature_flag_from_str() {
        assert_eq!(
            FeatureFlag::from_str("enable_organizations"),
            FeatureFlag::EnableOrganizations
        );
        assert_eq!(
            FeatureFlag::from_str("custom_feature"),
            FeatureFlag::Custom("custom_feature".to_string())
        );
    }

    #[test]
    fn test_feature_flag_config_conversion() {
        let mut config = HashMap::new();
        config.insert("enable_organizations".to_string(), true);
        config.insert("enable_user_roles".to_string(), false);

        let flags = FeatureFlags::from_config(config.clone());
        assert!(flags.is_enabled(&FeatureFlag::EnableOrganizations));
        assert!(!flags.is_enabled(&FeatureFlag::EnableUserRoles));

        // 转换回配置格式
        let back_to_config = flags.to_config();
        assert_eq!(back_to_config.get("enable_organizations"), Some(&true));
        assert_eq!(back_to_config.get("enable_user_roles"), Some(&false));
    }
}
