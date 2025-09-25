use std::sync::Arc;

use application::{ChatService, UserService};

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub chat_service: Arc<ChatService>,
}

impl AppState {
    pub fn new(user_service: Arc<UserService>, chat_service: Arc<ChatService>) -> Self {
        Self {
            user_service,
            chat_service,
        }
    }
}
