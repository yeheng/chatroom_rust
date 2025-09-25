//! Web API 层。
//!
//! 提供 Axum 路由，将 HTTP / WebSocket 请求委托给应用层的用例服务。

mod error;
mod routes;
mod state;

pub use routes::router;
pub use state::AppState;
