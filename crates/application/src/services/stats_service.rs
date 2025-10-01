use anyhow::Result;
use chrono::{DateTime, Utc};
use domain::RepositoryError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// 统一的统计数据查询服务
/// 根据设计文档，这个服务提供统一的查询接口，支持多维度查询
pub struct StatsService {
    pool: Arc<PgPool>,
}

impl StatsService {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// 统一的查询接口，按维度查询统计数据
    pub async fn get_stats(
        &self,
        dimension: Dimension,
        time_range: TimeRange,
        granularity: Granularity,
    ) -> Result<Vec<StatsData>, RepositoryError> {
        let sql = match dimension {
            Dimension::Room(_) => self.build_room_query_sql(),
            Dimension::Org(_) => self.build_org_query_sql(),
            Dimension::User(_) => self.build_user_query_sql(),
        };

        let dimension_id = match dimension {
            Dimension::Room(id) => Some(Uuid::from(id)),
            Dimension::Org(id) => Some(Uuid::from(id)),
            Dimension::User(id) => Some(Uuid::from(id)),
        };

        let records = sqlx::query_as::<_, StatsDataRecord>(&sql)
            .bind(dimension_type_to_string(&dimension))
            .bind(dimension_id)
            .bind(time_range.start_time)
            .bind(time_range.end_time)
            .bind(granularity_to_string(&granularity))
            .fetch_all(self.pool.as_ref())
            .await
            .map_err(map_sqlx_err)?;

        let mut stats_data = Vec::new();
        for record in records {
            stats_data.push(StatsData::try_from(record)?);
        }

        Ok(stats_data)
    }

    /// 获取实时在线统计（通过Redis查询）
    pub async fn get_realtime_stats(
        &self,
        dimension: Dimension,
    ) -> Result<RealtimeStats, RepositoryError> {
        // 这里应该查询Redis获取实时数据
        // 目前简化实现，返回基础结构
        match dimension {
            Dimension::Org(org_id) => {
                // 查询组织下当前在线用户数
                let online_count = sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(DISTINCT user_id)
                    FROM presence_events p
                    JOIN users u ON p.user_id = u.id
                    WHERE u.org_id = $1
                      AND p.event_type = 'Connected'
                      AND p.timestamp > NOW() - INTERVAL '5 minutes'
                    "#,
                )
                .bind(Uuid::from(org_id))
                .fetch_one(self.pool.as_ref())
                .await
                .map_err(map_sqlx_err)?;

                Ok(RealtimeStats {
                    dimension_type: "org".to_string(),
                    dimension_id: Uuid::from(org_id),
                    online_users: online_count as u64,
                    timestamp: Utc::now(),
                })
            }
            _ => {
                // 其他维度的实时统计实现
                Ok(RealtimeStats {
                    dimension_type: dimension_type_to_string(&dimension),
                    dimension_id: match dimension {
                        Dimension::Room(id) => Uuid::from(id),
                        Dimension::Org(id) => Uuid::from(id),
                        Dimension::User(id) => Uuid::from(id),
                    },
                    online_users: 0,
                    timestamp: Utc::now(),
                })
            }
        }
    }

    fn build_room_query_sql(&self) -> String {
        r#"
        SELECT
            dimension_type,
            dimension_id,
            time_bucket,
            granularity,
            peak_online_count,
            avg_online_count,
            total_connections,
            unique_users,
            avg_session_duration
        FROM stats_aggregated_v2
        WHERE dimension_type = $1
          AND ($2::UUID IS NULL OR dimension_id = $2)
          AND time_bucket >= $3
          AND time_bucket < $4
          AND granularity = $5
        ORDER BY time_bucket DESC
        "#
        .to_string()
    }

    fn build_org_query_sql(&self) -> String {
        r#"
        SELECT
            dimension_type,
            dimension_id,
            time_bucket,
            granularity,
            peak_online_count,
            avg_online_count,
            total_connections,
            unique_users,
            avg_session_duration
        FROM stats_aggregated_v2
        WHERE dimension_type = $1
          AND ($2::UUID IS NULL OR dimension_id = $2)
          AND time_bucket >= $3
          AND time_bucket < $4
          AND granularity = $5
        ORDER BY time_bucket DESC
        "#
        .to_string()
    }

    fn build_user_query_sql(&self) -> String {
        r#"
        SELECT
            dimension_type,
            dimension_id,
            time_bucket,
            granularity,
            peak_online_count,
            avg_online_count,
            total_connections,
            unique_users,
            avg_session_duration
        FROM stats_aggregated_v2
        WHERE dimension_type = $1
          AND ($2::UUID IS NULL OR dimension_id = $2)
          AND time_bucket >= $3
          AND time_bucket < $4
          AND granularity = $5
        ORDER BY time_bucket DESC
        "#
        .to_string()
    }
}

/// 统计维度枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum Dimension {
    Room(domain::RoomId),
    Org(domain::OrgId),
    User(domain::UserId),
}

/// 时间粒度枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Granularity {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// 时间范围参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Self {
        Self {
            start_time,
            end_time,
        }
    }

    pub fn last_hours(hours: i64) -> Self {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::hours(hours);
        Self {
            start_time,
            end_time,
        }
    }

    pub fn last_days(days: i64) -> Self {
        let end_time = Utc::now();
        let start_time = end_time - chrono::Duration::days(days);
        Self {
            start_time,
            end_time,
        }
    }
}

/// 统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsData {
    pub dimension_type: String,
    pub dimension_id: Uuid,
    pub time_bucket: DateTime<Utc>,
    pub granularity: String,
    pub peak_online_count: i64,
    pub avg_online_count: f64,
    pub total_connections: i64,
    pub unique_users: i64,
    pub avg_session_duration: f64,
}

/// 实时统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeStats {
    pub dimension_type: String,
    pub dimension_id: Uuid, // 统一使用UUID类型
    pub online_users: u64,
    pub timestamp: DateTime<Utc>,
}

/// 数据库记录类型
#[derive(Debug, sqlx::FromRow)]
struct StatsDataRecord {
    dimension_type: String,
    dimension_id: Uuid,
    time_bucket: DateTime<Utc>,
    granularity: String,
    peak_online_count: i64,
    avg_online_count: f64,
    total_connections: i64,
    unique_users: i64,
    avg_session_duration: f64,
}

impl TryFrom<StatsDataRecord> for StatsData {
    type Error = RepositoryError;

    fn try_from(record: StatsDataRecord) -> Result<Self, Self::Error> {
        Ok(StatsData {
            dimension_type: record.dimension_type,
            dimension_id: record.dimension_id,
            time_bucket: record.time_bucket,
            granularity: record.granularity,
            peak_online_count: record.peak_online_count,
            avg_online_count: record.avg_online_count,
            total_connections: record.total_connections,
            unique_users: record.unique_users,
            avg_session_duration: record.avg_session_duration,
        })
    }
}

/// 辅助函数：将维度类型转换为字符串
fn dimension_type_to_string(dimension: &Dimension) -> String {
    match dimension {
        Dimension::Room(_) => "room".to_string(),
        Dimension::Org(_) => "org".to_string(),
        Dimension::User(_) => "user".to_string(),
    }
}

/// 辅助函数：将粒度转换为字符串
fn granularity_to_string(granularity: &Granularity) -> String {
    match granularity {
        Granularity::Hour => "Hour".to_string(),
        Granularity::Day => "Day".to_string(),
        Granularity::Week => "Week".to_string(),
        Granularity::Month => "Month".to_string(),
        Granularity::Year => "Year".to_string(),
    }
}

/// 映射SQL错误
fn map_sqlx_err(err: sqlx::Error) -> RepositoryError {
    match err {
        sqlx::Error::RowNotFound => RepositoryError::NotFound,
        sqlx::Error::Database(ref db_err) if db_err.code().is_some_and(|code| code == "23505") => {
            RepositoryError::Conflict
        }
        other => {
            let message = other.to_string();
            RepositoryError::storage_with_source(message, other)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_type_to_string() {
        let room_dim = Dimension::Room(domain::RoomId::from(uuid::Uuid::new_v4()));
        assert_eq!(dimension_type_to_string(&room_dim), "room");

        let org_dim = Dimension::Org(domain::OrgId::from(uuid::Uuid::new_v4()));
        assert_eq!(dimension_type_to_string(&org_dim), "org");

        let user_dim = Dimension::User(domain::UserId::from(uuid::Uuid::new_v4()));
        assert_eq!(dimension_type_to_string(&user_dim), "user");
    }

    #[test]
    fn test_granularity_to_string() {
        assert_eq!(granularity_to_string(&Granularity::Hour), "Hour");
        assert_eq!(granularity_to_string(&Granularity::Day), "Day");
        assert_eq!(granularity_to_string(&Granularity::Week), "Week");
        assert_eq!(granularity_to_string(&Granularity::Month), "Month");
        assert_eq!(granularity_to_string(&Granularity::Year), "Year");
    }

    #[test]
    fn test_time_range_creation() {
        let range = TimeRange::last_hours(24);
        assert!(range.end_time > range.start_time);

        let duration = range.end_time - range.start_time;
        assert_eq!(duration.num_hours(), 24);
    }
}
