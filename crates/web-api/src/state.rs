use std::sync::Arc;

use application::{ChatService, UserService};
use infrastructure::BroadcasterType;

use crate::JwtService;

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub chat_service: Arc<ChatService>,
    pub broadcaster: BroadcasterType,
    pub jwt_service: Arc<JwtService>,  // 添加 JWT 服务
}

impl AppState {
    pub fn new(
        user_service: Arc<UserService>,
        chat_service: Arc<ChatService>,
        broadcaster: BroadcasterType,
        jwt_service: Arc<JwtService>,  // 添加 JWT 服务参数
    ) -> Self {
        Self {
            user_service,
            chat_service,
            broadcaster,
            jwt_service,
        }
    }
}