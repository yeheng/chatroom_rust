use anyhow::Result;
use application::presence::{UserPresenceEvent, PresenceEventType};
use chrono::Utc;
use config::AppConfig;
use redis::{Client, RedisResult};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::signal;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

/// 消费者组名称
const CONSUMER_GROUP: &str = "stats_consumers";
/// 流名称
const STREAM_NAME: &str = "presence_events_stream";
/// 消费者名称（包含实例ID）
const CONSUMER_NAME: &str = "consumer-1";

/// 统计数据消费者
/// 专门消费Redis Stream中的用户状态事件并写入PostgreSQL
#[derive(Clone)]
pub struct StatsConsumer {
    redis_client: Arc<Client>,
    db_pool: PgPool,
    batch_size: usize,
    flush_interval: Duration,
}

impl StatsConsumer {
    /// 创建新的统计消费者
    pub fn new(redis_client: Arc<Client>, db_pool: PgPool) -> Self {
        Self {
            redis_client,
            db_pool,
            batch_size: 100, // 每批最多100条记录
            flush_interval: Duration::from_secs(5), // 5秒强制刷新
        }
    }

    /// 初始化消费者组和流
    pub async fn initialize(&self) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 创建消费者组（如果不存在）
        let result: RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(STREAM_NAME)
            .arg(CONSUMER_GROUP)
            .arg("0") // 从ID为0的消息开始
            .arg("MKSTREAM") // 如果流不存在则创建
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => {
                info!("消费者组 '{}' 创建成功", CONSUMER_GROUP);
            }
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                info!("消费者组 '{}' 已存在", CONSUMER_GROUP);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("创建消费者组失败: {}", e));
            }
        }

        Ok(())
    }

    /// 运行消费者主循环
    pub async fn run(&self) -> Result<()> {
        info!("启动统计消费者，批量大小: {}, 刷新间隔: {:?}",
              self.batch_size, self.flush_interval);

        let mut last_flush = Utc::now();
        let mut batch = Vec::new();

        loop {
            // 检查是否需要强制刷新
            if !batch.is_empty() && (Utc::now() - last_flush).num_milliseconds() > self.flush_interval.as_millis() as i64 {
                self.flush_batch(&mut batch).await?;
                last_flush = Utc::now();
            }

            // 从Redis Stream读取消息
            match self.read_messages().await {
                Ok(events) => {
                    if events.is_empty() {
                        // 没有消息，稍作等待
                        sleep(Duration::from_millis(100)).await;
                        continue;
                    }

                    info!("接收到 {} 个用户状态事件", events.len());

                    for event in events {
                        batch.push(event);

                        // 达到批量大小立即刷新
                        if batch.len() >= self.batch_size {
                            self.flush_batch(&mut batch).await?;
                            last_flush = Utc::now();
                        }
                    }
                }
                Err(e) => {
                    error!("读取消息失败: {}", e);
                    // 错误后等待一段时间再重试
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// 从Redis Stream读取消息
    async fn read_messages(&self) -> Result<Vec<UserPresenceEvent>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 使用XREADGROUP命令从消费者组读取消息
        let result: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(CONSUMER_GROUP)
            .arg(CONSUMER_NAME)
            .arg("COUNT") // 限制返回的消息数量
            .arg(self.batch_size.to_string())
            .arg("BLOCK") // 阻塞等待
            .arg("1000") // 1秒超时
            .arg("STREAMS")
            .arg(STREAM_NAME)
            .arg(">") // 只读取新消息
            .query_async(&mut conn)
            .await?;

        let mut events = Vec::new();

        for (_stream_name, messages) in result {
            for (message_id, fields) in messages {
                // 解析消息字段
                let event_data = self.parse_message_fields(&fields)?;

                // 确认消息已处理
                self.acknowledge_message(&message_id).await?;

                events.push(event_data);
            }
        }

        Ok(events)
    }

    /// 解析Redis Stream消息字段
    fn parse_message_fields(&self, fields: &[(String, String)]) -> Result<UserPresenceEvent> {
        let mut event_data = serde_json::Map::new();

        for (key, value) in fields {
            if key == "event_id" {
                continue; // 跳过消息ID字段
            }
            event_data.insert(key.clone(), serde_json::Value::String(value.clone()));
        }

        let event_json = serde_json::Value::Object(event_data);
        let event: UserPresenceEvent = serde_json::from_value(event_json)
            .map_err(|e| anyhow::anyhow!("解析事件数据失败: {}", e))?;

        Ok(event)
    }

    /// 确认消息已处理
    async fn acknowledge_message(&self, message_id: &str) -> Result<()> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let _: () = redis::cmd("XACK")
            .arg(STREAM_NAME)
            .arg(CONSUMER_GROUP)
            .arg(message_id)
            .query_async(&mut conn)
            .await?;

        Ok(())
    }

    /// 刷新批次数据到数据库
    async fn flush_batch(&self, batch: &mut Vec<UserPresenceEvent>) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        info!("写入 {} 条记录到数据库", batch.len());

        // 使用事务批量插入
        let mut tx = self.db_pool.begin().await?;

        for event in batch.iter() {
            sqlx::query(
                r#"
                INSERT INTO presence_events (
                    event_id, user_id, room_id, event_type,
                    timestamp, session_id, user_ip, user_agent
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (event_id) DO NOTHING
                "#,
            )
            .bind(event.event_id)
            .bind(Uuid::from(event.user_id))
            .bind(Uuid::from(event.room_id))
            .bind(event.event_type as PresenceEventType)
            .bind(event.timestamp)
            .bind(event.session_id)
            .bind(&event.user_ip)
            .bind(&event.user_agent)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        batch.clear();

        info!("批次写入完成");
        Ok(())
    }

    /// 处理积压的消息（用于启动时清理未确认的消息）
    pub async fn process_pending_messages(&self) -> Result<()> {
        info!("处理积压的消息...");

        let mut processed = 0;
        loop {
            let batch = self.read_pending_messages().await?;

            if batch.is_empty() {
                break;
            }

            let mut events = Vec::new();
            for (message_id, event) in batch {
                events.push(event);
                self.acknowledge_message(&message_id).await?;
                processed += 1;
            }

            // 写入数据库
            let mut batch_vec = events;
            self.flush_batch(&mut batch_vec).await?;
        }

        if processed > 0 {
            info!("处理了 {} 条积压消息", processed);
        }

        Ok(())
    }

    /// 读取积压的消息
    async fn read_pending_messages(&self) -> Result<Vec<(String, UserPresenceEvent)>> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        // 读取消费者组的积压消息
        let result: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(CONSUMER_GROUP)
            .arg(CONSUMER_NAME)
            .arg("COUNT")
            .arg(self.batch_size.to_string())
            .arg("STREAMS")
            .arg(STREAM_NAME)
            .arg("0") // 从积压消息开始读取
            .query_async(&mut conn)
            .await?;

        let mut messages = Vec::new();

        for (_stream_name, message_list) in result {
            for (message_id, fields) in message_list {
                let event = self.parse_message_fields(&fields)?;
                messages.push((message_id, event));
            }
        }

        Ok(messages)
    }

    /// 优雅停机处理
    pub async fn shutdown(&self) -> Result<()> {
        info!("开始优雅停机...");

        // 这里可以添加清理逻辑
        // 例如：将内存中的数据刷新到数据库

        info!("优雅停机完成");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("启动统计消费者服务");

    // 加载配置
    let config = AppConfig::from_env();

    // 创建数据库连接池
    let db_pool = infrastructure::repository::create_pg_pool(
        &config.database.url,
        config.database.max_connections
    ).await?;

    // 创建Redis客户端
    let redis_client = Arc::new(
        redis::Client::open(config.redis.url.clone())
            .map_err(|e| anyhow::anyhow!("Redis连接失败: {}", e))?
    );

    // 创建统计消费者
    let consumer = StatsConsumer::new(redis_client, db_pool);

    // 初始化消费者组和流
    consumer.initialize().await?;

    // 处理积压消息
    if let Err(e) = consumer.process_pending_messages().await {
        warn!("处理积压消息失败: {}", e);
    }

    // 设置优雅停机处理
    let consumer_for_shutdown = consumer.clone();
    let shutdown_signal = async {
        signal::ctrl_c()
            .await
            .expect("无法监听Ctrl+C信号");

        info!("接收到关闭信号，开始停机...");
        if let Err(e) = consumer_for_shutdown.shutdown().await {
            error!("优雅停机失败: {}", e);
        }
    };

    // 运行主循环
    tokio::select! {
        result = consumer.run() => {
            if let Err(e) = result {
                error!("消费者运行失败: {}", e);
            }
        }
        _ = shutdown_signal => {
            info!("服务已停止");
        }
    }

    Ok(())
}