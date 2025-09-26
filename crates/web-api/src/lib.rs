//! Web API 层。
//!
//! 提供 Axum 路由，将 HTTP / WebSocket 请求委托给应用层的用例服务。

mod auth;
mod error;
mod routes;
mod state;
mod ws_connection;

pub use auth::{JwtService, LoginResponse};
pub use config::JwtConfig;
pub use routes::router;
pub use state::AppState;
