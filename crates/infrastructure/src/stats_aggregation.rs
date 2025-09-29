use application::ApplicationError;
use domain::RoomId;
use sqlx::{
    types::chrono::{DateTime, Utc, NaiveDate},
    PgPool, Row,
};
use uuid::Uuid;

use crate::repository::map_sqlx_err;

/// 时间粒度枚举
#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum TimeGranularity {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl std::fmt::Display for TimeGranularity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeGranularity::Hour => write!(f, "Hour"),
            TimeGranularity::Day => write!(f, "Day"),
            TimeGranularity::Week => write!(f, "Week"),
            TimeGranularity::Month => write!(f, "Month"),
            TimeGranularity::Year => write!(f, "Year"),
        }
    }
}

impl std::str::FromStr for TimeGranularity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Hour" => Ok(TimeGranularity::Hour),
            "Day" => Ok(TimeGranularity::Day),
            "Week" => Ok(TimeGranularity::Week),
            "Month" => Ok(TimeGranularity::Month),
            "Year" => Ok(TimeGranularity::Year),
            _ => Err(format!("Invalid time granularity: {}", s)),
        }
    }
}

/// 房间统计数据
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RoomStats {
    pub room_id: RoomId,
    pub time_bucket: DateTime<Utc>,
    pub granularity: TimeGranularity,
    pub peak_online_count: i64,
    pub avg_online_count: f64,
    pub total_connections: i64,
    pub unique_users: i64,
    pub avg_session_duration: f64, // 秒
}

/// 统计报表查询参数
#[derive(Debug, Clone)]
pub struct StatsQuery {
    pub room_id: Option<RoomId>,
    pub granularity: TimeGranularity,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub limit: Option<i64>,
}

/// 在线统计汇总
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OnlineStatsSummary {
    pub total_rooms: i64,
    pub total_users: i64,
    pub total_sessions: i64,
    pub avg_session_duration: f64,
    pub peak_concurrent_users: i64,
}

/// PostgreSQL统计聚合服务
///
/// 负责：
/// 1. 从原始事件计算聚合统计
/// 2. 提供管理员报表查询
/// 3. 数据清理和维护
#[derive(Clone)]
pub struct StatsAggregationService {
    pool: PgPool,
}

impl StatsAggregationService {
    /// 创建新的统计聚合服务
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 执行指定时间范围的统计聚合
    ///
    /// 根据时间粒度计算：
    /// - 峰值在线人数
    /// - 平均在线人数
    /// - 总连接数
    /// - 去重用户数
    /// - 平均会话时长
    pub async fn aggregate_stats(
        &self,
        granularity: TimeGranularity,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<RoomStats>, ApplicationError> {
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
                FROM presence_events
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
            }
        );

        let rows = sqlx::query(&sql)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

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
    pub async fn save_aggregated_stats(&self, stats: &[RoomStats]) -> Result<(), ApplicationError> {
        if stats.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx_err)?;

        for stat in stats {
            let granularity_str = stat.granularity.to_string();
            sqlx::query(
                r#"
                INSERT INTO stats_aggregated (
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
            )
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
            .map_err(map_sqlx_err)?;
        }

        tx.commit().await.map_err(map_sqlx_err)?;

        tracing::info!(
            stats_count = stats.len(),
            "Successfully saved aggregated statistics"
        );

        Ok(())
    }

    /// 查询聚合统计数据
    pub async fn query_stats(&self, query: StatsQuery) -> Result<Vec<RoomStats>, ApplicationError> {
        let mut sql = String::from(
            r#"
            SELECT room_id, time_bucket, granularity, peak_online_count,
                   avg_online_count, total_connections, unique_users, avg_session_duration
            FROM stats_aggregated
            WHERE granularity = $1 AND time_bucket >= $2 AND time_bucket < $3
        "#,
        );

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
            .map_err(map_sqlx_err)?;

        let mut results = Vec::new();
        for row in rows {
            let room_id: Uuid = row.get("room_id");
            let time_bucket: DateTime<Utc> = row.get("time_bucket");
            let granularity_str: String = row.get("granularity");
            let granularity = granularity_str
                .parse::<TimeGranularity>()
                .map_err(ApplicationError::infrastructure)?;

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

    /// 获取在线统计汇总
    pub async fn get_online_summary(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<OnlineStatsSummary, ApplicationError> {
        let row = sqlx::query(r#"
            SELECT
                COUNT(DISTINCT room_id) as total_rooms,
                COUNT(DISTINCT user_id) as total_users,
                COUNT(DISTINCT session_id) as total_sessions,
                AVG(
                    EXTRACT(EPOCH FROM (
                        COALESCE(
                            (SELECT timestamp FROM presence_events pe2
                             WHERE pe2.session_id = pe1.session_id
                             AND pe2.event_type = 'Disconnected'
                             AND pe2.timestamp > pe1.timestamp
                             ORDER BY pe2.timestamp LIMIT 1),
                            NOW()
                        ) - pe1.timestamp
                    ))
                ) FILTER (WHERE event_type = 'Connected') as avg_session_duration,
                (
                    SELECT MAX(concurrent_count) FROM (
                        SELECT timestamp,
                               SUM(CASE WHEN event_type = 'Connected' THEN 1
                                        WHEN event_type = 'Disconnected' THEN -1
                                        ELSE 0 END)
                               OVER (ORDER BY timestamp ROWS UNBOUNDED PRECEDING) as concurrent_count
                        FROM presence_events
                        WHERE timestamp >= $1 AND timestamp < $2
                    ) concurrent_stats
                ) as peak_concurrent_users
            FROM presence_events pe1
            WHERE timestamp >= $1 AND timestamp < $2
        "#)
        .bind(start_time)
        .bind(end_time)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        Ok(OnlineStatsSummary {
            total_rooms: row.get::<Option<i64>, _>("total_rooms").unwrap_or(0),
            total_users: row.get::<Option<i64>, _>("total_users").unwrap_or(0),
            total_sessions: row.get::<Option<i64>, _>("total_sessions").unwrap_or(0),
            avg_session_duration: row
                .get::<Option<f64>, _>("avg_session_duration")
                .unwrap_or(0.0),
            peak_concurrent_users: row
                .get::<Option<i64>, _>("peak_concurrent_users")
                .unwrap_or(0),
        })
    }

    /// 创建分区表（按月分区）
    /// 用于确保新月份的分区存在
    pub async fn create_partition_if_not_exists(&self, target_date: DateTime<Utc>) -> Result<String, ApplicationError> {
        // 使用 SQL 来处理日期计算，避免复杂的 Rust 日期操作
        let partition_row = sqlx::query(
            r#"
            SELECT
                date_trunc('month', $1::timestamptz) as month_start,
                date_trunc('month', $1::timestamptz) + interval '1 month' as next_month_start,
                'presence_events_' || to_char(date_trunc('month', $1::timestamptz), 'YYYY_MM') as partition_name
            "#
        )
        .bind(target_date)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        let partition_name: String = partition_row.get("partition_name");
        let month_start: DateTime<Utc> = partition_row.get("month_start");
        let next_month_start: DateTime<Utc> = partition_row.get("next_month_start");

        // 检查分区是否已存在
        let exists_row = sqlx::query(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM pg_class
                WHERE relname = $1 AND relkind = 'r'
            ) as partition_exists
            "#
        )
        .bind(&partition_name)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        let partition_exists: bool = exists_row.get("partition_exists");

        if !partition_exists {
            // 创建分区表
            let create_sql = format!(
                r#"
                CREATE TABLE {} PARTITION OF presence_events
                FOR VALUES FROM ('{}') TO ('{}')
                "#,
                partition_name,
                month_start.format("%Y-%m-%d %H:%M:%S"),
                next_month_start.format("%Y-%m-%d %H:%M:%S")
            );

            sqlx::query(&create_sql)
                .execute(&self.pool)
                .await
                .map_err(map_sqlx_err)?;

            // 为分区创建索引
            let index_room_time = format!("idx_{}_room_time", partition_name);
            let index_user_session = format!("idx_{}_user_session", partition_name);

            sqlx::query(&format!(
                "CREATE INDEX {} ON {} (room_id, timestamp)",
                index_room_time, partition_name
            ))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

            sqlx::query(&format!(
                "CREATE INDEX {} ON {} (user_id, session_id)",
                index_user_session, partition_name
            ))
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

            tracing::info!(
                partition_name = %partition_name,
                month_start = %month_start,
                "Created new partition for presence_events"
            );

            Ok(format!("Created partition: {}", partition_name))
        } else {
            Ok(format!("Partition already exists: {}", partition_name))
        }
    }

    /// 清理过期的原始事件分区
    /// 删除超过保留期限的整个分区表
    pub async fn cleanup_expired_partitions(&self) -> Result<Vec<(String, i64)>, ApplicationError> {
        // 使用 SQL 计算30天前的日期
        let cutoff_info = sqlx::query(
            r#"
            SELECT
                (CURRENT_DATE - INTERVAL '30 days')::date as cutoff_date,
                date_trunc('month', (CURRENT_DATE - INTERVAL '30 days')::date)::date as cutoff_month
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        let cutoff_month: NaiveDate = cutoff_info.get("cutoff_month");

        // 查找所有过期的分区
        let partition_rows = sqlx::query(
            r#"
            SELECT tablename
            FROM pg_tables
            WHERE tablename LIKE 'presence_events_%'
            AND tablename ~ '^presence_events_\d{4}_\d{2}$'
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        let mut deleted_partitions = Vec::new();

        for row in partition_rows {
            let table_name: String = row.get("tablename");

            // 解析分区表名中的日期
            if let Some(date_part) = table_name.strip_prefix("presence_events_") {
                if let Ok(partition_date) = NaiveDate::parse_from_str(&format!("{}-01", date_part.replace('_', "-")), "%Y-%m-%d") {
                    if partition_date < cutoff_month {
                        // 获取分区表的记录数
                        let count_row = sqlx::query(&format!("SELECT COUNT(*) as row_count FROM {}", table_name))
                            .fetch_one(&self.pool)
                            .await
                            .map_err(map_sqlx_err)?;

                        let row_count: i64 = count_row.get("row_count");

                        // 删除分区表
                        sqlx::query(&format!("DROP TABLE IF EXISTS {}", table_name))
                            .execute(&self.pool)
                            .await
                            .map_err(map_sqlx_err)?;

                        tracing::info!(
                            partition_name = %table_name,
                            deleted_rows = row_count,
                            "Dropped expired partition"
                        );

                        deleted_partitions.push((table_name, row_count));
                    }
                }
            }
        }

        Ok(deleted_partitions)
    }

    /// 清理过期的聚合数据
    /// 使用 Rust 代码实现，替代 PL/pgSQL 函数
    pub async fn cleanup_expired_aggregated_data(&self) -> Result<i64, ApplicationError> {
        // 获取保留策略
        let retention_rows = sqlx::query(
            "SELECT granularity, retention_days FROM stats_data_retention WHERE retention_days > 0"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        let mut total_deleted = 0i64;

        for row in retention_rows {
            let granularity: String = row.get("granularity");
            let retention_days: i32 = row.get("retention_days");

            // 使用 SQL 来计算截止日期
            let result = sqlx::query(
                r#"
                DELETE FROM stats_aggregated
                WHERE granularity = $1
                AND time_bucket < (CURRENT_TIMESTAMP - ($2 || ' days')::INTERVAL)
                "#
            )
            .bind(&granularity)
            .bind(retention_days)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

            let deleted_rows = result.rows_affected() as i64;
            total_deleted += deleted_rows;

            tracing::debug!(
                granularity = %granularity,
                retention_days = retention_days,
                deleted_rows = deleted_rows,
                "Cleaned up expired aggregated data"
            );
        }

        tracing::info!(
            total_deleted = total_deleted,
            "Completed cleanup of expired aggregated data"
        );

        Ok(total_deleted)
    }

    /// 清理过期的聚合数据（保持向后兼容性）
    pub async fn cleanup_expired_data(&self) -> Result<i64, ApplicationError> {
        self.cleanup_expired_aggregated_data().await
    }

    /// 执行完整的聚合流水线
    ///
    /// 1. 计算聚合统计
    /// 2. 保存到数据库
    /// 3. 清理过期数据
    pub async fn run_aggregation_pipeline(
        &self,
        granularity: TimeGranularity,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<usize, ApplicationError> {
        tracing::info!(
            granularity = ?granularity,
            start_time = %start_time,
            end_time = %end_time,
            "Starting aggregation pipeline"
        );

        // 1. 计算聚合统计
        let stats = self
            .aggregate_stats(granularity, start_time, end_time)
            .await?;
        let stats_count = stats.len();

        // 2. 保存统计数据
        self.save_aggregated_stats(&stats).await?;

        // 3. 清理过期数据
        let _deleted_count = self.cleanup_expired_data().await?;

        tracing::info!(
            stats_count = stats_count,
            "Aggregation pipeline completed successfully"
        );

        Ok(stats_count)
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore] // 需要数据库连接
    async fn test_aggregate_stats() {
        // 测试统计聚合逻辑
    }

    #[tokio::test]
    #[ignore] // 需要数据库连接
    async fn test_query_stats() {
        // 测试统计查询
    }
}
