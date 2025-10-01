//! 基础设施层实现。
//!
//! 提供数据库仓储、密码哈希、消息广播等适配器，实现应用/领域层定义的接口。

pub mod broadcast;
pub mod builder;
pub mod delivery;
pub mod migrations;
pub mod password;
pub mod repository;
pub mod stats_aggregation;

pub use broadcast::{RedisMessageBroadcaster, RedisMessageStream};
pub use builder::{Infrastructure, InfrastructureError};
pub use delivery::PgDeliveryTracker;
pub use migrations::MIGRATOR;
pub use password::BcryptPasswordHasher;
pub use repository::{
    create_pg_pool, PgChatRoomRepository, PgMessageRepository, PgOrganizationRepository,
    PgRoomMemberRepository, PgStorage, PgUserRepository,
};
pub use stats_aggregation::{
    OnlineStatsSummary, RoomStats, StatsAggregationService, StatsQuery, TimeGranularity,
};
