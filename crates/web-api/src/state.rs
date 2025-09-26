use std::sync::Arc;

use application::{ChatService, LocalMessageBroadcaster, PresenceManager, UserService};

use crate::JwtService;

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub chat_service: Arc<ChatService>,
    pub broadcaster: Arc<LocalMessageBroadcaster>,
    pub jwt_service: Arc<JwtService>,
    pub presence_manager: Arc<dyn PresenceManager>,
}

impl AppState {
    pub fn new(
        user_service: Arc<UserService>,
        chat_service: Arc<ChatService>,
        broadcaster: Arc<LocalMessageBroadcaster>,
        jwt_service: Arc<JwtService>,
        presence_manager: Arc<dyn PresenceManager>,
    ) -> Self {
        Self {
            user_service,
            chat_service,
            broadcaster,
            jwt_service,
            presence_manager,
        }
    }
}
