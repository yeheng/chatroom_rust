//! Web API 层。
//!
//! 提供 Axum 路由，将 HTTP / WebSocket 请求委托给应用层的用例服务。

mod admin_routes;
mod auth;
mod error;
mod routes;
mod state;
mod ws_connection;

pub use admin_routes::admin_routes;
pub use auth::{JwtService, LoginResponse};
pub use config::JwtConfig;
pub use routes::router;
pub use state::AppState;
