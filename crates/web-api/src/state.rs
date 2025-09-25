use std::sync::Arc;

use application::{ChatService, UserService};
use infrastructure::LocalMessageBroadcaster;

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub chat_service: Arc<ChatService>,
    pub broadcaster: Arc<LocalMessageBroadcaster>,
}

impl AppState {
    pub fn new(
        user_service: Arc<UserService>,
        chat_service: Arc<ChatService>,
        broadcaster: Arc<LocalMessageBroadcaster>,
    ) -> Self {
        Self {
            user_service,
            chat_service,
            broadcaster,
        }
    }
}
