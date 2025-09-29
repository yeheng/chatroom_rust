//! Stats Consumer 服务
//!
//! 从 Redis Stream 读取用户状态事件，批量写入 PostgreSQL

use application::{PresenceEventType, UserPresenceEvent};
use chrono::{DateTime, Utc};
use config::AppConfig;
use domain::{RoomId, UserId};
use redis::streams::StreamReadReply;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

mod event_storage;
mod pg_event_storage;

use event_storage::EventStorage;
use pg_event_storage::{create_event_storage, PgEventStorage};

/// Stats Consumer 配置
#[derive(Debug, Clone)]
struct ConsumerConfig {
    stream_name: String,
    consumer_group: String,
    consumer_name: String,
    batch_size: i64,
    poll_interval: Duration,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            stream_name: "presence_events".to_string(),
            consumer_group: "stats_consumers".to_string(),
            consumer_name: "consumer_1".to_string(),
            batch_size: 10,
            poll_interval: Duration::from_secs(1),
        }
    }
}

/// Stats Consumer 主服务
pub struct StatsConsumer {
    redis_client: Arc<redis::Client>,
    event_storage: PgEventStorage,
    config: ConsumerConfig,
}

impl StatsConsumer {
    /// 创建 Stats Consumer
    fn new(
        redis_client: Arc<redis::Client>,
        event_storage: PgEventStorage,
        config: ConsumerConfig,
    ) -> Self {
        Self {
            redis_client,
            event_storage,
            config,
        }
    }

    /// 启动消费者主循环
    pub async fn run(&self) -> anyhow::Result<()> {
        info!(
            stream_name = %self.config.stream_name,
            consumer_group = %self.config.consumer_group,
            consumer_name = %self.config.consumer_name,
            "Stats Consumer 开始运行"
        );

        // 尝试创建消费者组（如果不存在）
        self.ensure_consumer_group().await?;

        loop {
            match self.process_batch().await {
                Ok(processed_count) => {
                    if processed_count > 0 {
                        info!(count = processed_count, "已处理事件批次");
                    }
                }
                Err(e) => {
                    error!(error = %e, "处理批次时发生错误");
                    // 等待一段时间后重试
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }

            // 轮询间隔
            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    /// 确保消费者组存在
    async fn ensure_consumer_group(&self) -> anyhow::Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 尝试创建消费者组，忽略 BUSYGROUP 错误（组已存在）
        let result: Result<String, redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&self.config.stream_name)
            .arg(&self.config.consumer_group)
            .arg("0") // 从流的开始位置
            .arg("MKSTREAM") // 如果流不存在则创建
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => {
                info!(
                    stream_name = %self.config.stream_name,
                    group = %self.config.consumer_group,
                    "消费者组已创建"
                );
            }
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                info!(
                    stream_name = %self.config.stream_name,
                    group = %self.config.consumer_group,
                    "消费者组已存在"
                );
            }
            Err(e) => {
                return Err(anyhow::anyhow!("创建消费者组失败: {}", e));
            }
        }

        Ok(())
    }

    /// 处理一个批次的事件
    async fn process_batch(&self) -> anyhow::Result<usize> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 使用 XREADGROUP 读取事件
        let stream_reply: StreamReadReply = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&self.config.consumer_group)
            .arg(&self.config.consumer_name)
            .arg("COUNT")
            .arg(self.config.batch_size)
            .arg("BLOCK")
            .arg(1000) // 阻塞 1 秒
            .arg("STREAMS")
            .arg(&self.config.stream_name)
            .arg(">") // 只读取新消息
            .query_async(&mut conn)
            .await?;

        if stream_reply.keys.is_empty() {
            return Ok(0); // 没有新消息
        }

        let mut events = Vec::new();
        let mut message_ids = Vec::new();

        // 解析消息
        for stream_key in &stream_reply.keys {
            for stream_id in &stream_key.ids {
                if let Some(event) = self.parse_event(&stream_id.map) {
                    events.push(event);
                    message_ids.push(stream_id.id.clone());
                } else {
                    warn!(
                        message_id = %stream_id.id,
                        "无法解析事件，跳过此消息"
                    );
                }
            }
        }

        if events.is_empty() {
            return Ok(0);
        }

        // 批量写入数据库
        self.event_storage.insert_events(&events).await?;

        // 确认消息已处理
        for message_id in message_ids {
            let _: i64 = redis::cmd("XACK")
                .arg(&self.config.stream_name)
                .arg(&self.config.consumer_group)
                .arg(&message_id)
                .query_async(&mut conn)
                .await?;
        }

        Ok(events.len())
    }

    /// 解析 Redis Stream 消息为用户状态事件
    fn parse_event(
        &self,
        fields: &std::collections::HashMap<String, redis::Value>,
    ) -> Option<UserPresenceEvent> {
        let event_id = self.get_uuid_field(fields, "event_id")?;
        let user_id = UserId::from(self.get_uuid_field(fields, "user_id")?);
        let room_id = RoomId::from(self.get_uuid_field(fields, "room_id")?);
        let session_id = self.get_uuid_field(fields, "session_id")?;

        let event_type_str = self.get_string_field(fields, "event_type")?;
        let event_type = match event_type_str.as_str() {
            "Connected" => PresenceEventType::Connected,
            "Disconnected" => PresenceEventType::Disconnected,
            "Heartbeat" => PresenceEventType::Heartbeat,
            _ => {
                warn!(event_type = %event_type_str, "未知的事件类型");
                return None;
            }
        };

        let timestamp_str = self.get_string_field(fields, "timestamp")?;
        let timestamp = timestamp_str.parse::<DateTime<Utc>>().ok()?;

        let user_ip = self
            .get_string_field(fields, "user_ip")
            .filter(|s| !s.is_empty());
        let user_agent = self
            .get_string_field(fields, "user_agent")
            .filter(|s| !s.is_empty());

        Some(UserPresenceEvent {
            event_id,
            user_id,
            room_id,
            event_type,
            timestamp,
            session_id,
            user_ip,
            user_agent,
        })
    }

    /// 从字段中获取 UUID
    fn get_uuid_field(
        &self,
        fields: &std::collections::HashMap<String, redis::Value>,
        key: &str,
    ) -> Option<Uuid> {
        self.get_string_field(fields, key)
            .and_then(|s| s.parse::<Uuid>().ok())
    }

    /// 从字段中获取字符串
    fn get_string_field(
        &self,
        fields: &std::collections::HashMap<String, redis::Value>,
        key: &str,
    ) -> Option<String> {
        match fields.get(key) {
            Some(redis::Value::BulkString(bytes)) => String::from_utf8(bytes.clone()).ok(),
            _ => None,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Stats Consumer 启动中...");

    // 加载配置
    let app_config = AppConfig::from_env_with_defaults();
    app_config
        .validate()
        .map_err(|e| anyhow::anyhow!("配置验证失败: {}", e))?;

    // 创建数据库连接池
    let pg_pool = infrastructure::create_pg_pool(
        &app_config.database.url,
        app_config.database.max_connections,
    )
    .await?;

    // 运行迁移
    sqlx::migrate!("../../migrations").run(&pg_pool).await?;

    // 创建事件存储
    let event_storage = create_event_storage(pg_pool);

    // 创建 Redis 客户端
    let redis_url = app_config
        .broadcast
        .redis_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Redis URL 未配置"))?;
    let redis_client = Arc::new(redis::Client::open(redis_url.clone())?);

    // 创建消费者配置
    let consumer_config = ConsumerConfig::default();

    // 创建并启动消费者
    let consumer = StatsConsumer::new(redis_client, event_storage, consumer_config);

    info!("Stats Consumer 启动完成，开始处理事件...");
    consumer.run().await?;

    Ok(())
}
