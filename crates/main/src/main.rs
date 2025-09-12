//! 主应用程序入口
//!
//! 启动 Axum Web API 服务。

use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let addr: SocketAddr = match "0.0.0.0:8080".parse() {
        Ok(x) => x,
        Err(_) => todo!(),
    };
    web_api::run(addr).await
}
