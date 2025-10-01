use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tracing::{error, info};

/// 统一维度统计聚合器
/// 根据设计文档，这个服务统一处理所有维度（房间、组织、用户等）的统计数据
pub struct UnifiedStatsAggregator {
    pool: Arc<PgPool>,
}

impl UnifiedStatsAggregator {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// 统一的聚合方法，处理所有维度
    pub async fn aggregate(&self, time_range: TimeRange) -> Result<()> {
        info!("开始统一维度统计聚合: {:?}", time_range);

        // 1. 聚合房间维度
        if let Err(e) = self.aggregate_dimension("room", &time_range).await {
            error!("房间维度聚合失败: {}", e);
        }

        // 2. 聚合组织维度
        if let Err(e) = self.aggregate_dimension("org", &time_range).await {
            error!("组织维度聚合失败: {}", e);
        }

        // 3. 未来可以添加更多维度
        // self.aggregate_dimension("user", &time_range).await?;

        info!("统一维度统计聚合完成");
        Ok(())
    }

    /// 聚合指定维度
    async fn aggregate_dimension(&self, dimension_type: &str, time_range: &TimeRange) -> Result<()> {
        info!("开始聚合 {} 维度", dimension_type);

        let sql = match dimension_type {
            "room" => self.build_room_aggregation_sql(time_range),
            "org" => self.build_org_aggregation_sql(time_range),
            "user" => self.build_user_aggregation_sql(time_range),
            _ => return Err(anyhow::anyhow!("不支持的维度类型: {}", dimension_type)),
        };

        let result = sqlx::query(&sql)
            .bind(time_range.start_time)
            .bind(time_range.end_time)
            .execute(self.pool.as_ref())
            .await?;

        info!(
            "{} 维度聚合完成，影响行数: {}",
            dimension_type,
            result.rows_affected()
        );

        Ok(())
    }

    /// 构建房间维度的聚合SQL
    fn build_room_aggregation_sql(&self, time_range: &TimeRange) -> String {
        r#"
        INSERT INTO stats_aggregated_v2 (
            dimension_type, dimension_id, time_bucket, granularity,
            peak_online_count, avg_online_count, total_connections,
            unique_users, avg_session_duration
        )
        SELECT
            'room'::TEXT AS dimension_type,
            room_id AS dimension_id,
            date_trunc('hour', timestamp) AS time_bucket,
            'Hour'::time_granularity AS granularity,
            MAX(online_count) AS peak_online_count,
            AVG(online_count) AS avg_online_count,
            COUNT(*) AS total_connections,
            COUNT(DISTINCT user_id) AS unique_users,
            -- 简化的会话时长计算（实际实现可能需要更复杂的逻辑）
            EXTRACT(EPOCH FROM (MAX(timestamp) - MIN(timestamp))) / NULLIF(COUNT(DISTINCT user_id), 0) AS avg_session_duration
        FROM presence_events
        WHERE timestamp >= $1 AND timestamp < $2
        GROUP BY room_id, date_trunc('hour', timestamp)
        ON CONFLICT (dimension_type, dimension_id, time_bucket, granularity) DO UPDATE SET
            peak_online_count = GREATEST(stats_aggregated_v2.peak_online_count, EXCLUDED.peak_online_count),
            avg_online_count = EXCLUDED.avg_online_count,
            total_connections = stats_aggregated_v2.total_connections + EXCLUDED.total_connections,
            unique_users = EXCLUDED.unique_users,
            avg_session_duration = EXCLUDED.avg_session_duration
        "#.to_string()
    }

    /// 构建组织维度的聚合SQL（核心SQL，展示ltree的强大之处）
    fn build_org_aggregation_sql(&self, time_range: &TimeRange) -> String {
        r#"
        INSERT INTO stats_aggregated_v2 (
            dimension_type, dimension_id, time_bucket, granularity,
            peak_online_count, avg_online_count, total_connections,
            unique_users, avg_session_duration
        )
        SELECT
            'org'::TEXT AS dimension_type,
            o.id AS dimension_id,
            date_trunc('hour', p.timestamp) AS time_bucket,
            'Hour'::time_granularity AS granularity,
            MAX(p.online_count) AS peak_online_count,
            AVG(p.online_count) AS avg_online_count,
            COUNT(*) AS total_connections,
            COUNT(DISTINCT p.user_id) AS unique_users,
            EXTRACT(EPOCH FROM (MAX(p.timestamp) - MIN(p.timestamp))) / NULLIF(COUNT(DISTINCT p.user_id), 0) AS avg_session_duration
        FROM presence_events p
        JOIN organizations o ON p.org_path <@ o.path  -- 关键：ltree祖先匹配操作符
        WHERE p.timestamp >= $1
          AND p.timestamp < $2
          AND p.org_path IS NOT NULL
        GROUP BY o.id, date_trunc('hour', p.timestamp)
        ON CONFLICT (dimension_type, dimension_id, time_bucket, granularity) DO UPDATE SET
            peak_online_count = GREATEST(stats_aggregated_v2.peak_online_count, EXCLUDED.peak_online_count),
            avg_online_count = EXCLUDED.avg_online_count,
            total_connections = stats_aggregated_v2.total_connections + EXCLUDED.total_connections,
            unique_users = EXCLUDED.unique_users,
            avg_session_duration = EXCLUDED.avg_session_duration
        "#.to_string()
    }

    /// 构建用户维度的聚合SQL（预留）
    fn build_user_aggregation_sql(&self, time_range: &TimeRange) -> String {
        // 用户维度的聚合逻辑（暂未实现）
        r#"
        SELECT 'user'::TEXT AS dimension_type,
               NULL::UUID AS dimension_id,
               NULL::TIMESTAMPTZ AS time_bucket,
               'Hour'::time_granularity AS granularity,
               0 AS peak_online_count,
               0.0 AS avg_online_count,
               0 AS total_connections,
               0 AS unique_users,
               0.0 AS avg_session_duration
        WHERE false  -- 暂不实现用户维度聚合
        "#.to_string()
    }

    /// 清理过期的聚合数据
    pub async fn cleanup_old_data(&self, retention_days: i32) -> Result<()> {
        info!("开始清理 {} 天前的聚合数据", retention_days);

        let cutoff_time = Utc::now() - chrono::Duration::days(retention_days as i64);

        let result = sqlx::query(
            r#"
            DELETE FROM stats_aggregated_v2
            WHERE time_bucket < $1
              AND granularity IN ('Hour', 'Day')
            -- 保留周、月、年数据
            "#
        )
        .bind(cutoff_time)
        .execute(self.pool.as_ref())
        .await?;

        info!(
            "清理完成，删除了 {} 条过期聚合记录",
            result.rows_affected()
        );

        Ok(())
    }

    /// 手动触发指定时间范围的聚合
    pub async fn aggregate_time_range(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<()> {
        let time_range = TimeRange { start_time, end_time };
        self.aggregate(time_range).await
    }
}

/// 时间范围参数
#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

impl TimeRange {
    /// 创建最近N小时的时间范围
    pub fn last_hours(hours: i64) -> Self {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::hours(hours);
        Self { start_time, end_time }
    }

    /// 创建最近N天的时间范围
    pub fn last_days(days: i64) -> Self {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::days(days);
        Self { start_time, end_time }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_time_range_creation() {
        let range = TimeRange::last_hours(24);
        assert!(range.end_time > range.start_time);

        let duration = range.end_time - range.start_time;
        assert_eq!(duration.num_hours(), 24);
    }

    #[test]
    fn test_org_aggregation_sql_generation() {
        let aggregator = UnifiedStatsAggregator::new(Arc::new(
            sqlx::PgPool::connect("postgresql://dummy").await.unwrap()
        ));

        let time_range = TimeRange::last_hours(1);
        let sql = aggregator.build_org_aggregation_sql(&time_range);

        // 验证SQL包含关键的组织聚合逻辑
        assert!(sql.contains("p.org_path <@ o.path"));
        assert!(sql.contains("'org'::TEXT AS dimension_type"));
        assert!(sql.contains("JOIN organizations o"));
    }
}