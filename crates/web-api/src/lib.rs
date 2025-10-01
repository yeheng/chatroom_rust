//! Web API 层。
//!
//! 提供 Axum 路由，将 HTTP / WebSocket 请求委托给应用层的用例服务。

mod admin_routes;
mod auth;
mod bulk_user_routes;
mod error;
mod org_routes;
mod routes;
mod state;
mod stats_routes;
mod ws_connection;

pub use admin_routes::admin_routes;
pub use auth::{JwtService, LoginResponse};
pub use bulk_user_routes::bulk_user_routes;
pub use config::JwtConfig;
pub use org_routes::org_routes;
pub use routes::router;
pub use state::AppState;
pub use stats_routes::stats_routes;
