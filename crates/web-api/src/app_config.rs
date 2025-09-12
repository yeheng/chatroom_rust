use figment::providers::{Env, Format, Json, Toml, Yaml};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ServerConfig {
    #[validate(length(min = 1))]
    pub host: String,
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct DatabaseConfig {
    #[validate(url)]
    pub url: String,
    #[serde(default)]
    pub max_connections: u32,
    #[serde(default)]
    pub min_connections: u32,
    #[serde(default)]
    pub acquire_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default)]
pub struct RedisConfigSimple {
    #[validate(url)]
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureFlags {
    #[serde(default)]
    pub enable_organizations: bool,
    #[serde(default)]
    pub enable_user_roles: bool,
    #[serde(default)]
    pub enable_proxy_system: bool,
    #[serde(default)]
    pub enable_bot_messages: bool,
    #[serde(default)]
    pub enable_online_statistics: bool,
    #[serde(default)]
    pub enable_advanced_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AppConfig {
    #[validate(nested)]
    pub server: ServerConfig,
    #[validate(nested)]
    pub database: DatabaseConfig,
    #[serde(default)]
    #[validate(nested)]
    pub redis: RedisConfigSimple,
    #[serde(default)]
    pub feature_flags: FeatureFlags,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".into(),
                port: 8080,
                cors_origins: vec!["*".into()],
            },
            database: DatabaseConfig {
                url: "postgres://user:pass@localhost/db".into(),
                max_connections: 10,
                min_connections: 1,
                acquire_timeout_seconds: 30,
            },
            redis: RedisConfigSimple {
                url: "redis://localhost:6379".into(),
            },
            feature_flags: FeatureFlags::default(),
        }
    }
}

impl AppConfig {
    /// Load config with precedence: defaults -> optional file (APP_CONFIG_FILE) -> env (APP_*)
    pub async fn load() -> anyhow::Result<Self> {
        let mut fig = figment::Figment::new().merge(figment::providers::Serialized::defaults(
            AppConfig::default(),
        ));
        if let Ok(path) = std::env::var("APP_CONFIG_FILE") {
            if path.ends_with(".yml") || path.ends_with(".yaml") {
                fig = fig.merge(Yaml::file(path));
            } else if path.ends_with(".json") {
                fig = fig.merge(Json::file(path));
            } else {
                fig = fig.merge(Toml::file(path));
            }
        }
        fig = fig.merge(Env::prefixed("APP_").split("__"));

        let cfg: AppConfig = fig.extract()?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Parse from TOML/YAML/JSON string; auto-detect by simple heuristics
    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        let cfg: AppConfig = if s.trim_start().starts_with('{') {
            serde_json::from_str(s)?
        } else if s.contains('[') || s.contains('=') {
            toml::from_str(s)?
        } else {
            serde_yaml::from_str(s)?
        };
        cfg.validate()?;
        Ok(cfg)
    }

    /// Return a sanitized string representation (for logs)
    pub fn sanitize(&self) -> String {
        let mut text = format!("{:?}", self);
        if let Some(start) = text.find("postgres://") {
            let end = text[start..]
                .find(' ')
                .map(|i| start + i)
                .unwrap_or(text.len());
            text.replace_range(start..end, "postgres://[REDACTED]");
        }
        text = text.replace("password", "[REDACTED]");
        text
    }
}
