use std::sync::Arc;

use application::{
    stats_collector::EventStorage, ChatService, MessageBroadcaster, PresenceEventCollector,
    PresenceManager, UserService,
};
use infrastructure::StatsAggregationService;

use crate::JwtService;

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub chat_service: Arc<ChatService>,
    pub broadcaster: Arc<dyn MessageBroadcaster>,
    pub jwt_service: Arc<JwtService>,
    pub presence_manager: Arc<dyn PresenceManager>,
    pub stats_service: Arc<StatsAggregationService>,
    pub event_storage: Arc<dyn EventStorage>,
    pub event_collector: Arc<PresenceEventCollector>,
}

impl AppState {
    pub fn new(
        user_service: Arc<UserService>,
        chat_service: Arc<ChatService>,
        broadcaster: Arc<dyn MessageBroadcaster>,
        jwt_service: Arc<JwtService>,
        presence_manager: Arc<dyn PresenceManager>,
        stats_service: Arc<StatsAggregationService>,
        event_storage: Arc<dyn EventStorage>,
        event_collector: Arc<PresenceEventCollector>,
    ) -> Self {
        Self {
            user_service,
            chat_service,
            broadcaster,
            jwt_service,
            presence_manager,
            stats_service,
            event_storage,
            event_collector,
        }
    }
}
