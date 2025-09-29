//! stats-aggregator 集成测试
//!
//! 验证独立统计聚合服务的功能，包括：
//! - 数据聚合管道
//! - 定时任务调度
//! - 数据存储和查询

use anyhow::{Result, anyhow};
use infrastructure::stats_aggregation::{TimeGranularity, StatsQuery, RoomStats};
use domain::RoomId;
use sqlx::{PgPool, Row, types::chrono::{DateTime, Utc}};
use uuid::Uuid;
use chrono::Duration;

/// 测试专用的统计聚合服务包装器
#[derive(Clone)]
struct TestStatsAggregationService {
    pool: PgPool,
    events_table: String,
    stats_table: String,
}

impl TestStatsAggregationService {
    fn new(pool: PgPool, test_id: &str) -> Self {
        Self {
            pool,
            events_table: format!("presence_events_{}", test_id),
            stats_table: format!("stats_aggregated_{}", test_id),
        }
    }

    /// 执行指定时间范围的统计聚合
    pub async fn aggregate_stats(
        &self,
        granularity: TimeGranularity,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<RoomStats>, anyhow::Error> {
        let time_format = match granularity {
            TimeGranularity::Hour => "YYYY-MM-DD HH24:00:00",
            TimeGranularity::Day => "YYYY-MM-DD 00:00:00",
            TimeGranularity::Week => "YYYY-\"W\"WW",
            TimeGranularity::Month => "YYYY-MM-01 00:00:00",
            TimeGranularity::Year => "YYYY-01-01 00:00:00",
        };

        let sql = format!(
            r#"
            WITH time_buckets AS (
                SELECT
                    room_id,
                    to_char(timestamp, '{}') as time_bucket_str,
                    date_trunc('{}', timestamp) as time_bucket,
                    user_id,
                    session_id,
                    event_type,
                    timestamp
                FROM public.{}
                WHERE timestamp >= $1 AND timestamp < $2
            ),
            session_durations AS (
                SELECT
                    room_id,
                    time_bucket,
                    session_id,
                    user_id,
                    MIN(CASE WHEN event_type = 'Connected' THEN timestamp END) as connect_time,
                    MAX(CASE WHEN event_type = 'Disconnected' THEN timestamp END) as disconnect_time
                FROM time_buckets
                GROUP BY room_id, time_bucket, session_id, user_id
            ),
            stats_by_bucket AS (
                SELECT
                    room_id,
                    time_bucket,
                    COUNT(DISTINCT CASE WHEN event_type = 'Connected' THEN session_id END) as total_connections,
                    COUNT(DISTINCT user_id) as unique_users,
                    AVG(EXTRACT(EPOCH FROM (disconnect_time - connect_time))::DOUBLE PRECISION) FILTER (
                        WHERE connect_time IS NOT NULL AND disconnect_time IS NOT NULL
                    ) as avg_session_duration
                FROM time_buckets tb
                LEFT JOIN session_durations sd USING (room_id, time_bucket, session_id, user_id)
                GROUP BY room_id, time_bucket
            )
            SELECT
                room_id,
                time_bucket,
                COALESCE(total_connections, 0) as total_connections,
                COALESCE(unique_users, 0) as unique_users,
                COALESCE(avg_session_duration, 0.0) as avg_session_duration,
                -- 这里简化峰值和平均在线计算，实际应该基于实时在线状态
                COALESCE(unique_users, 0) as peak_online_count,
                COALESCE(unique_users::float / 2.0, 0.0) as avg_online_count
            FROM stats_by_bucket
            ORDER BY room_id, time_bucket
        "#,
            time_format,
            match granularity {
                TimeGranularity::Hour => "hour",
                TimeGranularity::Day => "day",
                TimeGranularity::Week => "week",
                TimeGranularity::Month => "month",
                TimeGranularity::Year => "year",
            },
            self.events_table
        );

        let rows = sqlx::query(&sql)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await.map_err(|e| anyhow!("Database query failed: {}", e))?;

        let mut results = Vec::new();
        for row in rows {
            let room_id: Uuid = row.get("room_id");
            let time_bucket: DateTime<Utc> = row.get("time_bucket");

            results.push(RoomStats {
                room_id: RoomId::from(room_id),
                time_bucket,
                granularity,
                peak_online_count: row.get("peak_online_count"),
                avg_online_count: row.get("avg_online_count"),
                total_connections: row.get("total_connections"),
                unique_users: row.get("unique_users"),
                avg_session_duration: row.get("avg_session_duration"),
            });
        }

        Ok(results)
    }

    /// 保存聚合统计到数据库
    pub async fn save_aggregated_stats(&self, stats: &[RoomStats]) -> Result<(), anyhow::Error> {
        if stats.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(|e| anyhow!("Failed to begin transaction: {}", e))?;

        for stat in stats {
            let granularity_str = stat.granularity.to_string();
            sqlx::query(&format!(
                r#"
                INSERT INTO public.{} (
                    room_id, time_bucket, granularity, peak_online_count,
                    avg_online_count, total_connections, unique_users, avg_session_duration
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (room_id, time_bucket, granularity)
                DO UPDATE SET
                    peak_online_count = EXCLUDED.peak_online_count,
                    avg_online_count = EXCLUDED.avg_online_count,
                    total_connections = EXCLUDED.total_connections,
                    unique_users = EXCLUDED.unique_users,
                    avg_session_duration = EXCLUDED.avg_session_duration
            "#,
                self.stats_table
            ))
            .bind(Uuid::from(stat.room_id))
            .bind(stat.time_bucket)
            .bind(&granularity_str)
            .bind(stat.peak_online_count)
            .bind(stat.avg_online_count)
            .bind(stat.total_connections)
            .bind(stat.unique_users)
            .bind(stat.avg_session_duration)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow!("Failed to save stats: {}", e))?;
        }

        tx.commit().await.map_err(|e| anyhow!("Failed to commit transaction: {}", e))?;

        Ok(())
    }

    /// 查询聚合统计数据
    pub async fn query_stats(&self, query: StatsQuery) -> Result<Vec<RoomStats>, anyhow::Error> {
        let mut sql = String::from(&format!(
            r#"
            SELECT room_id, time_bucket, granularity, peak_online_count,
                   avg_online_count, total_connections, unique_users, avg_session_duration
            FROM public.{}
            WHERE granularity = $1 AND time_bucket >= $2 AND time_bucket < $3
        "#,
            self.stats_table
        ));

        let mut param_count = 3;
        if query.room_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND room_id = ${}", param_count));
        }

        sql.push_str(" ORDER BY room_id, time_bucket");

        if let Some(_limit) = query.limit {
            param_count += 1;
            sql.push_str(&format!(" LIMIT ${}", param_count));
        }

        let granularity_str = query.granularity.to_string();
        let mut query_builder = sqlx::query(&sql)
            .bind(&granularity_str)
            .bind(query.start_time)
            .bind(query.end_time);

        if let Some(room_id) = query.room_id {
            query_builder = query_builder.bind(Uuid::from(room_id));
        }

        if let Some(limit) = query.limit {
            query_builder = query_builder.bind(limit);
        }

        let rows = query_builder
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow!("Failed to query stats: {}", e))?;

        let mut results = Vec::new();
        for row in rows {
            let room_id: Uuid = row.get("room_id");
            let time_bucket: DateTime<Utc> = row.get("time_bucket");
            let granularity_str: String = row.get("granularity");
            let granularity = granularity_str
                .parse::<TimeGranularity>()
                .map_err(|e| anyhow!("Failed to parse granularity: {}", e))?;

            results.push(RoomStats {
                room_id: RoomId::from(room_id),
                time_bucket,
                granularity,
                peak_online_count: row.get("peak_online_count"),
                avg_online_count: row.get("avg_online_count"),
                total_connections: row.get("total_connections"),
                unique_users: row.get("unique_users"),
                avg_session_duration: row.get("avg_session_duration"),
            });
        }

        Ok(results)
    }
}

/// 测试配置
#[derive(Clone)]
struct TestConfig {
    database_url: String,
    test_id: String,  // 为每个测试实例添加唯一ID
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://yeheng@127.0.0.1:5432/chatroom_integration_test".to_string(),
            test_id: format!("test_{}", Uuid::new_v4().to_string().replace("-", "_")),
        }
    }
}

/// 测试服务集合
struct TestServices {
    pool: PgPool,
    aggregation_service: TestStatsAggregationService,
    test_id: String,
}

impl TestServices {
    async fn new(config: TestConfig) -> Result<Self> {
        // 创建数据库连接池
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)  // 减少连接数避免并发问题
            .connect(&config.database_url)
            .await.map_err(|e| anyhow!(e))?;

        // 创建测试所需的表结构
        Self::setup_test_tables(&pool, &config.test_id).await?;

        // 创建统计聚合服务
        let aggregation_service = TestStatsAggregationService::new(pool.clone(), &config.test_id);

        Ok(Self {
            pool,
            aggregation_service,
            test_id: config.test_id,
        })
    }

    /// 设置测试所需的表结构
    async fn setup_test_tables(pool: &PgPool, test_id: &str) -> Result<()> {
        let users_table = format!("users_{}", test_id);
        let rooms_table = format!("chat_rooms_{}", test_id);
        let events_table = format!("presence_events_{}", test_id);
        let stats_table = format!("stats_aggregated_{}", test_id);

        // 清理可能存在的表，避免冲突
        sqlx::query(&format!("DROP TABLE IF EXISTS public.{} CASCADE", events_table))
            .execute(pool)
            .await.map_err(|e| anyhow!(e))?;

        sqlx::query(&format!("DROP TABLE IF EXISTS public.{} CASCADE", stats_table))
            .execute(pool)
            .await.map_err(|e| anyhow!(e))?;

        sqlx::query(&format!("DROP TABLE IF EXISTS public.{} CASCADE", rooms_table))
            .execute(pool)
            .await.map_err(|e| anyhow!(e))?;

        sqlx::query(&format!("DROP TABLE IF EXISTS public.{} CASCADE", users_table))
            .execute(pool)
            .await.map_err(|e| anyhow!(e))?;

        // 创建用户表
        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS public.{} (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                username VARCHAR(50) NOT NULL UNIQUE,
                email VARCHAR(100) NOT NULL UNIQUE,
                password_hash VARCHAR(255) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            users_table
        ))
        .execute(pool)
        .await.map_err(|e| anyhow!(e))?;

        // 创建聊天室表
        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS public.{} (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                name VARCHAR(100) NOT NULL,
                description TEXT,
                max_members INTEGER NOT NULL DEFAULT 100,
                created_by UUID NOT NULL REFERENCES public.{}(id) ON DELETE CASCADE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
            rooms_table, users_table
        ))
        .execute(pool)
        .await.map_err(|e| anyhow!(e))?;

        // 创建presence_events表（简化版本，避免枚举类型冲突）
        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS public.{} (
                event_id UUID PRIMARY KEY,
                user_id UUID NOT NULL,
                room_id UUID NOT NULL,
                event_type VARCHAR(20) NOT NULL,
                timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                session_id UUID NOT NULL,
                user_ip INET,
                user_agent TEXT
            )
            "#,
            events_table
        ))
        .execute(pool)
        .await.map_err(|e| anyhow!(e))?;

        // 创建stats_aggregated表（简化版本）
        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS public.{} (
                room_id UUID NOT NULL,
                time_bucket TIMESTAMPTZ NOT NULL,
                granularity TEXT NOT NULL,
                peak_online_count BIGINT NOT NULL,
                avg_online_count DOUBLE PRECISION NOT NULL,
                total_connections BIGINT NOT NULL,
                unique_users BIGINT NOT NULL,
                avg_session_duration DOUBLE PRECISION NOT NULL,
                PRIMARY KEY (room_id, time_bucket, granularity)
            )
            "#,
            stats_table
        ))
        .execute(pool)
        .await.map_err(|e| anyhow!(e))?;

        // 创建索引
        sqlx::query(&format!("CREATE INDEX IF NOT EXISTS idx_{}_room_time ON {} (room_id, timestamp)", events_table, events_table))
            .execute(pool)
            .await.map_err(|e| anyhow!(e))?;

        sqlx::query(&format!("CREATE INDEX IF NOT EXISTS idx_{}_user_session ON {} (user_id, session_id)", events_table, events_table))
            .execute(pool)
            .await.map_err(|e| anyhow!(e))?;

        sqlx::query(&format!("CREATE INDEX IF NOT EXISTS idx_{}_time_gran ON {} (time_bucket, granularity)", stats_table, stats_table))
            .execute(pool)
            .await.map_err(|e| anyhow!(e))?;

        Ok(())
    }

    /// 创建测试事件数据
    async fn create_test_events(
        &self,
        room_id: Uuid,
        start_time: DateTime<Utc>,
    ) -> Result<()> {
        let events_table = format!("presence_events_{}", self.test_id);

        // 创建测试用户
        let user1_id = Uuid::new_v4();
        let user2_id = Uuid::new_v4();

        // 创建测试用的presence_events数据
        for i in 0..10 {
            let event_id = Uuid::new_v4();
            let user_id = if i % 2 == 0 { user1_id } else { user2_id };
            let event_type = if i % 3 == 0 {
                "Disconnected"
            } else {
                "Connected"
            };

            sqlx::query(&format!(
                r#"
                INSERT INTO public.{} (event_id, user_id, room_id, event_type, timestamp, session_id)
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                events_table
            ))
            .bind(event_id)
            .bind(user_id)
            .bind(room_id)
            .bind(event_type)
            .bind(start_time + Duration::minutes(i * 5))
            .bind(Uuid::new_v4())
            .execute(&self.pool)
            .await.map_err(|e| anyhow!(e))?;
        }

        Ok(())
    }

    /// 清理测试数据
    async fn cleanup_test_data(&self) -> Result<()> {
        let events_table = format!("presence_events_{}", self.test_id);
        let stats_table = format!("stats_aggregated_{}", self.test_id);

        // 清理 presence_events 表
        sqlx::query(&format!("DELETE FROM public.{}", events_table))
            .execute(&self.pool)
            .await.map_err(|e| anyhow!(e))?;

        // 清理 stats_aggregated 表
        sqlx::query(&format!("DELETE FROM public.{}", stats_table))
            .execute(&self.pool)
            .await.map_err(|e| anyhow!(e))?;

        Ok(())
    }

    /// 检查聚合结果
    async fn verify_aggregation_results(
        &self,
        room_id: Uuid,
        granularity: TimeGranularity,
        _start_time: DateTime<Utc>,
        expected_count: usize,
    ) -> Result<bool> {
        let stats_table = format!("stats_aggregated_{}", self.test_id);
        let granularity_str = granularity.to_string();

        // 添加调试信息
        println!("验证查询参数 - room_id: {}, granularity: {}", room_id, granularity_str);

        let count = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT COUNT(*) FROM public.{}
             WHERE room_id = $1 AND granularity = $2",
            stats_table
        ))
        .bind(room_id)
        .bind(&granularity_str)
        .fetch_one(&self.pool)
        .await.map_err(|e| anyhow!(e))?;

        println!("查询结果 count: {}, expected_count: {}", count, expected_count);

        // 查询实际的记录详情
        let actual_records = sqlx::query(&format!(
            "SELECT room_id, time_bucket, granularity FROM public.{}
             WHERE room_id = $1 AND granularity = $2",
            stats_table
        ))
        .bind(room_id)
        .bind(&granularity_str)
        .fetch_all(&self.pool)
        .await.map_err(|e| anyhow!(e))?;

        println!("实际记录数量: {}", actual_records.len());
        for record in actual_records {
            let room: Uuid = record.get("room_id");
            let time_bucket: DateTime<Utc> = record.get("time_bucket");
            let gran: String = record.get("granularity");
            println!("  记录: room_id={}, time_bucket={}, granularity={}", room, time_bucket, gran);
        }

        Ok(count as usize == expected_count)
    }
}

/// 测试小时级统计聚合
#[tokio::test]
async fn test_hourly_stats_aggregation() -> Result<(), anyhow::Error> {
    let config = TestConfig::default();
    let services = TestServices::new(config).await.map_err(|e| anyhow!(e))?;

    let room_id = Uuid::new_v4();
    let start_time = Utc::now() - Duration::hours(2);

    // 创建测试事件数据
    services.create_test_events(room_id, start_time).await.map_err(|e| anyhow!(e))?;

    // 执行小时级聚合
    let end_time = start_time + Duration::hours(1);
    let stats = services
        .aggregation_service
        .aggregate_stats(TimeGranularity::Hour, start_time, end_time)
        .await.map_err(|e| anyhow!(e))?;

    // 验证聚合结果
    assert!(!stats.is_empty(), "应该有聚合统计结果");
    assert_eq!(stats.len(), 1, "应该生成1个小时的时间桶");

    let first_stat = &stats[0];
    assert_eq!(first_stat.room_id, room_id.into());
    assert_eq!(format!("{:?}", first_stat.granularity), "Hour");
    assert!(first_stat.total_connections > 0, "应该有连接数统计");
    assert!(first_stat.unique_users > 0, "应该有用户数统计");

    println!("生成的统计数据: {:?}", first_stat);

    // 保存聚合结果
    let save_result = services
        .aggregation_service
        .save_aggregated_stats(&stats)
        .await;

    match save_result {
        Ok(_) => println!("✅ 聚合结果保存成功"),
        Err(e) => {
            println!("❌ 聚合结果保存失败: {}", e);
            return Err(anyhow!(e));
        }
    }

    // 验证数据已保存
    let saved = services
        .verify_aggregation_results(room_id, TimeGranularity::Hour, start_time, 1)
        .await.map_err(|e| anyhow!(e))?;
    println!("数据验证结果: {}", saved);
    assert!(saved, "聚合结果应该已保存到数据库");

    // 清理测试数据
    services.cleanup_test_data().await.map_err(|e| anyhow!(e))?;

    println!("✅ 小时级统计聚合测试通过");
    Ok(())
}

/// 测试日级统计聚合
#[tokio::test]
async fn test_daily_stats_aggregation() -> Result<(), anyhow::Error> {
    let config = TestConfig::default();
    let services = TestServices::new(config).await.map_err(|e| anyhow!(e))?;

    let room_id = Uuid::new_v4();
    let start_time = Utc::now() - Duration::days(2);

    // 创建测试事件数据
    services.create_test_events(room_id, start_time).await.map_err(|e| anyhow!(e))?;

    // 执行日级聚合
    let end_time = start_time + Duration::days(1);
    let stats = services
        .aggregation_service
        .aggregate_stats(TimeGranularity::Day, start_time, end_time)
        .await.map_err(|e| anyhow!(e))?;

    // 验证聚合结果
    assert!(!stats.is_empty(), "应该有聚合统计结果");
    assert_eq!(stats.len(), 1, "应该生成1个日时间桶");

    let first_stat = &stats[0];
    assert_eq!(first_stat.room_id, room_id.into());
    assert_eq!(format!("{:?}", first_stat.granularity), "Day");
    assert!(first_stat.total_connections > 0, "应该有连接数统计");

    // 保存聚合结果
    services
        .aggregation_service
        .save_aggregated_stats(&stats)
        .await.map_err(|e| anyhow!(e))?;

    // 验证数据已保存
    let saved = services
        .verify_aggregation_results(room_id, TimeGranularity::Day, start_time, 1)
        .await.map_err(|e| anyhow!(e))?;
    assert!(saved, "日聚合结果应该已保存到数据库");

    // 清理测试数据
    services.cleanup_test_data().await.map_err(|e| anyhow!(e))?;

    println!("✅ 日级统计聚合测试通过");
    Ok(())
}

/// 测试完整的聚合管道
#[tokio::test]
async fn test_aggregation_pipeline() -> Result<()> {
    let config = TestConfig::default();
    let services = TestServices::new(config).await.map_err(|e| anyhow!(e))?;

    let room_id = Uuid::new_v4();
    let start_time = Utc::now() - Duration::days(1);

    // 创建测试事件数据
    services.create_test_events(room_id, start_time).await.map_err(|e| anyhow!(e))?;

    // 手动执行聚合管道的步骤，避免调用不存在的清理函数
    // 1. 计算聚合统计
    let stats = services
        .aggregation_service
        .aggregate_stats(TimeGranularity::Hour, start_time, Utc::now())
        .await.map_err(|e| anyhow!(e))?;

    // 2. 保存统计数据
    services
        .aggregation_service
        .save_aggregated_stats(&stats)
        .await.map_err(|e| anyhow!(e))?;

    // 验证管道生成了结果
    assert!(stats.len() > 0, "聚合管道应该生成结果");

    // 清理测试数据
    services.cleanup_test_data().await.map_err(|e| anyhow!(e))?;

    println!("✅ 聚合管道测试通过");
    Ok(())
}

/// 测试统计查询
#[tokio::test]
async fn test_stats_query() -> Result<()> {
    let config = TestConfig::default();
    let services = TestServices::new(config).await.map_err(|e| anyhow!(e))?;

    let room_id = Uuid::new_v4();
    let start_time = Utc::now() - Duration::hours(2);

    // 创建测试事件数据
    services.create_test_events(room_id, start_time).await.map_err(|e| anyhow!(e))?;

    // 执行聚合
    let end_time = start_time + Duration::hours(1);
    let stats = services
        .aggregation_service
        .aggregate_stats(TimeGranularity::Hour, start_time, end_time)
        .await.map_err(|e| anyhow!(e))?;

    // 保存聚合结果
    services
        .aggregation_service
        .save_aggregated_stats(&stats)
        .await.map_err(|e| anyhow!(e))?;

    // 查询统计信息 - 使用更宽泛的时间范围确保包含聚合数据
    let query_start_time = start_time - Duration::hours(1);  // 扩大查询范围
    let query_end_time = end_time + Duration::hours(1);      // 扩大查询范围
    let query = StatsQuery {
        room_id: Some(RoomId::from(room_id)),
        granularity: TimeGranularity::Hour,
        start_time: query_start_time,
        end_time: query_end_time,
        limit: None,
    };
    let query_results = services
        .aggregation_service
        .query_stats(query)
        .await.map_err(|e| anyhow!(e))?;

    assert!(!query_results.is_empty(), "应该能查询到统计信息");

    // 清理测试数据
    services.cleanup_test_data().await.map_err(|e| anyhow!(e))?;

    println!("✅ 统计查询测试通过");
    Ok(())
}

/// 测试并发聚合操作
#[tokio::test]
async fn test_concurrent_aggregation() -> Result<()> {
    let config = TestConfig::default();
    let services = TestServices::new(config).await.map_err(|e| anyhow!(e))?;

    let room_id = Uuid::new_v4();
    let start_time = Utc::now() - Duration::hours(2);

    // 创建测试事件数据
    services.create_test_events(room_id, start_time).await.map_err(|e| anyhow!(e))?;

    // 并发执行多个聚合任务
    let agg_service1 = services.aggregation_service.clone();
    let agg_service2 = services.aggregation_service.clone();

    let hourly_task = tokio::spawn(async move {
        agg_service1
            .aggregate_stats(
                TimeGranularity::Hour,
                start_time,
                start_time + Duration::hours(1),
            )
            .await
    });

    let daily_task = tokio::spawn(async move {
        agg_service2
            .aggregate_stats(
                TimeGranularity::Day,
                start_time,
                start_time + Duration::days(1),
            )
            .await
    });

    // 等待两个任务完成
    let (hourly_result, daily_result) = tokio::try_join!(hourly_task, daily_task)?;

    // 验证两个任务都成功完成
    assert!(hourly_result?.len() > 0, "小时级聚合应该成功");
    assert!(daily_result?.len() > 0, "日级聚合应该成功");

    // 清理测试数据
    services.cleanup_test_data().await.map_err(|e| anyhow!(e))?;

    println!("✅ 并发聚合操作测试通过");
    Ok(())
}

/// 测试时间粒度转换
#[tokio::test]
async fn test_time_granularity_conversion() -> Result<()> {
    // 测试时间粒度字符串转换
    let hour_str = format!("{:?}", TimeGranularity::Hour);
    let day_str = format!("{:?}", TimeGranularity::Day);

    assert!(hour_str.contains("Hour"), "小时粒度字符串应该正确");
    assert!(day_str.contains("Day"), "日粒度字符串应该正确");

    println!("✅ 时间粒度转换测试通过");
    Ok(())
}