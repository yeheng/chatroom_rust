use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use application::services::{Dimension, Granularity, RealtimeStats, StatsData, TimeRange};
use domain::{OrgId, RoomId, UserId};

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct StatsQuery {
    pub dimension: String, // "room", "org", "user"
    pub dimension_id: Option<Uuid>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub granularity: String, // "hour", "day", "week", "month", "year"
}

#[derive(Debug, Serialize)]
pub struct StatsDataResponse {
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

impl From<StatsData> for StatsDataResponse {
    fn from(data: StatsData) -> Self {
        Self {
            dimension_type: data.dimension_type,
            dimension_id: data.dimension_id,
            time_bucket: data.time_bucket,
            granularity: data.granularity,
            peak_online_count: data.peak_online_count,
            avg_online_count: data.avg_online_count,
            total_connections: data.total_connections,
            unique_users: data.unique_users,
            avg_session_duration: data.avg_session_duration,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RealtimeStatsResponse {
    pub dimension_type: String,
    pub dimension_id: Uuid,
    pub online_users: u64,
    pub timestamp: DateTime<Utc>,
}

impl From<RealtimeStats> for RealtimeStatsResponse {
    fn from(stats: RealtimeStats) -> Self {
        Self {
            dimension_type: stats.dimension_type,
            dimension_id: stats.dimension_id,
            online_users: stats.online_users,
            timestamp: stats.timestamp,
        }
    }
}

pub fn stats_routes() -> Router<AppState> {
    Router::new()
        .route("/query", get(query_stats))
        .route("/realtime", get(get_realtime_stats))
        .route(
            "/realtime/:dimension_type/:dimension_id",
            get(get_realtime_stats_by_dimension),
        )
}

/// 查询统计数据
async fn query_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<Vec<StatsDataResponse>>, ApiError> {
    let _user_id = state.jwt_service.extract_user_from_headers(&headers)?; // 验证身份

    // 解析维度类型
    let dimension = match query.dimension.as_str() {
        "room" => Dimension::Room(RoomId::from(query.dimension_id.ok_or_else(|| {
            ApiError::bad_request("dimension_id is required for room dimension")
        })?)),
        "org" => Dimension::Org(OrgId::from(query.dimension_id.ok_or_else(|| {
            ApiError::bad_request("dimension_id is required for org dimension")
        })?)),
        "user" => Dimension::User(UserId::from(query.dimension_id.ok_or_else(|| {
            ApiError::bad_request("dimension_id is required for user dimension")
        })?)),
        _ => {
            return Err(ApiError::bad_request(
                "Invalid dimension type. Must be one of: room, org, user",
            ))
        }
    };

    // 解析粒度
    let granularity = match query.granularity.as_str() {
        "hour" => Granularity::Hour,
        "day" => Granularity::Day,
        "week" => Granularity::Week,
        "month" => Granularity::Month,
        "year" => Granularity::Year,
        _ => {
            return Err(ApiError::bad_request(
                "Invalid granularity. Must be one of: hour, day, week, month, year",
            ))
        }
    };

    // 创建时间范围
    let time_range = TimeRange::new(query.start_time, query.end_time);

    // 查询统计数据
    let stats = state
        .stats_service
        .get_stats(dimension, time_range, granularity)
        .await?;
    let responses: Vec<StatsDataResponse> = stats.into_iter().map(Into::into).collect();

    Ok(Json(responses))
}

/// 获取实时统计数据
async fn get_realtime_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<StatsQuery>,
) -> Result<Json<RealtimeStatsResponse>, ApiError> {
    let _user_id = state.jwt_service.extract_user_from_headers(&headers)?; // 验证身份

    // 解析维度类型
    let dimension = match params.dimension.as_str() {
        "room" => Dimension::Room(RoomId::from(params.dimension_id.ok_or_else(|| {
            ApiError::bad_request("dimension_id is required for room dimension")
        })?)),
        "org" => Dimension::Org(OrgId::from(params.dimension_id.ok_or_else(|| {
            ApiError::bad_request("dimension_id is required for org dimension")
        })?)),
        "user" => Dimension::User(UserId::from(params.dimension_id.ok_or_else(|| {
            ApiError::bad_request("dimension_id is required for user dimension")
        })?)),
        _ => {
            return Err(ApiError::bad_request(
                "Invalid dimension type. Must be one of: room, org, user",
            ))
        }
    };

    // 获取实时统计数据
    let stats = state.stats_service.get_realtime_stats(dimension).await?;
    Ok(Json(stats.into()))
}

/// 根据维度ID获取实时统计数据
async fn get_realtime_stats_by_dimension(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((dimension_type, dimension_id)): Path<(String, Uuid)>,
) -> Result<Json<RealtimeStatsResponse>, ApiError> {
    let _user_id = state.jwt_service.extract_user_from_headers(&headers)?; // 验证身份

    // 解析维度类型
    let dimension = match dimension_type.as_str() {
        "room" => Dimension::Room(RoomId::from(dimension_id)),
        "org" => Dimension::Org(OrgId::from(dimension_id)),
        "user" => Dimension::User(UserId::from(dimension_id)),
        _ => {
            return Err(ApiError::bad_request(
                "Invalid dimension type. Must be one of: room, org, user",
            ))
        }
    };

    // 获取实时统计数据
    let stats = state.stats_service.get_realtime_stats(dimension).await?;
    Ok(Json(stats.into()))
}
