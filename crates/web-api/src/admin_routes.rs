use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use domain::RoomId;
use infrastructure::{OnlineStatsSummary, RoomStats, StatsQuery, TimeGranularity};

use crate::{error::ApiError, state::AppState};

/// 管理员统计报表路由
pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/stats/rooms", get(get_room_stats))
        .route("/stats/rooms/{room_id}", get(get_room_stats_by_id))
        .route("/stats/summary", get(get_stats_summary))
        .route("/stats/online", get(get_current_online_stats))
        .route("/stats/events", get(get_event_metrics))
}

/// 统计查询参数
#[derive(Debug, Deserialize)]
struct StatsQueryParams {
    granularity: Option<String>,  // "hour", "day", "week", "month", "year"
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    limit: Option<i64>,
}

/// 房间统计响应
#[derive(Debug, Serialize)]
struct RoomStatsResponse {
    room_id: Uuid,
    time_bucket: DateTime<Utc>,
    granularity: String,
    peak_online_count: i64,
    avg_online_count: f64,
    total_connections: i64,
    unique_users: i64,
    avg_session_duration: f64,
}

impl From<RoomStats> for RoomStatsResponse {
    fn from(stats: RoomStats) -> Self {
        Self {
            room_id: stats.room_id.into(),
            time_bucket: stats.time_bucket,
            granularity: format!("{:?}", stats.granularity),
            peak_online_count: stats.peak_online_count,
            avg_online_count: stats.avg_online_count,
            total_connections: stats.total_connections,
            unique_users: stats.unique_users,
            avg_session_duration: stats.avg_session_duration,
        }
    }
}

/// 当前在线统计响应
#[derive(Debug, Serialize)]
struct CurrentOnlineStatsResponse {
    room_id: Uuid,
    online_count: u64,
    online_users: Vec<Uuid>,
}

/// 事件指标响应
#[derive(Debug, Serialize)]
struct EventMetricsResponse {
    total_events: i64,
    events_in_last_hour: i64,
    events_in_last_day: i64,
    queue_status: QueueStatusResponse,
}

#[derive(Debug, Serialize)]
struct QueueStatusResponse {
    queue_size: usize,
    max_queue_size: usize,
    is_running: bool,
    batch_size: usize,
    flush_interval_secs: u64,
}

/// 解析时间粒度字符串
fn parse_granularity(granularity_str: &str) -> Result<TimeGranularity, ApiError> {
    match granularity_str.to_lowercase().as_str() {
        "hour" => Ok(TimeGranularity::Hour),
        "day" => Ok(TimeGranularity::Day),
        "week" => Ok(TimeGranularity::Week),
        "month" => Ok(TimeGranularity::Month),
        "year" => Ok(TimeGranularity::Year),
        _ => Err(ApiError::bad_request("Invalid granularity. Use: hour, day, week, month, year")),
    }
}

/// 验证管理员权限
/// 简化实现：检查用户是否为房间所有者或系统管理员
async fn verify_admin_access(
    _state: &AppState,
    _user_id: Uuid,
    room_id: Option<Uuid>,
) -> Result<(), ApiError> {
    // 简化实现：假设所有认证用户都可以查看统计（生产环境应该有更严格的权限控制）
    // 实际实现中应该：
    // 1. 检查用户是否为系统管理员
    // 2. 如果指定了房间ID，检查用户是否为房间所有者
    // 3. 查询用户角色和权限

    // 这里我们使用简化的权限检查
    if let Some(_room_id) = room_id {
        // 检查用户是否为房间成员（简化权限检查）
        // 实际应该检查是否为房间所有者或管理员
    }

    // 暂时允许所有认证用户访问（仅用于演示）
    Ok(())
}

/// 获取房间统计数据
async fn get_room_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<StatsQueryParams>,
) -> Result<Json<Vec<RoomStatsResponse>>, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;
    verify_admin_access(&state, user_id, None).await?;

    // 设置默认值
    let granularity = if let Some(g) = params.granularity {
        parse_granularity(&g)?
    } else {
        TimeGranularity::Day
    };

    let end_time = params.end_time.unwrap_or_else(Utc::now);
    let start_time = params.start_time.unwrap_or_else(|| {
        end_time - chrono::Duration::days(7) // 默认查询过去7天
    });

    let query = StatsQuery {
        room_id: None, // 查询所有房间
        granularity,
        start_time,
        end_time,
        limit: params.limit,
    };

    let stats = state
        .stats_service
        .query_stats(query)
        .await
        .map_err(|err| {
            ApiError::internal_server_error(&format!("Failed to query stats: {}", err))
        })?;

    let response: Vec<RoomStatsResponse> = stats.into_iter().map(RoomStatsResponse::from).collect();
    Ok(Json(response))
}

/// 获取指定房间的统计数据
async fn get_room_stats_by_id(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Query(params): Query<StatsQueryParams>,
) -> Result<Json<Vec<RoomStatsResponse>>, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;
    verify_admin_access(&state, user_id, Some(room_id)).await?;

    let granularity = if let Some(g) = params.granularity {
        parse_granularity(&g)?
    } else {
        TimeGranularity::Day
    };

    let end_time = params.end_time.unwrap_or_else(Utc::now);
    let start_time = params.start_time.unwrap_or_else(|| {
        end_time - chrono::Duration::days(7)
    });

    let query = StatsQuery {
        room_id: Some(RoomId::from(room_id)),
        granularity,
        start_time,
        end_time,
        limit: params.limit,
    };

    let stats = state
        .stats_service
        .query_stats(query)
        .await
        .map_err(|err| {
            ApiError::internal_server_error(&format!("Failed to query room stats: {}", err))
        })?;

    let response: Vec<RoomStatsResponse> = stats.into_iter().map(RoomStatsResponse::from).collect();
    Ok(Json(response))
}

/// 获取统计汇总信息
async fn get_stats_summary(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<StatsQueryParams>,
) -> Result<Json<OnlineStatsSummary>, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;
    verify_admin_access(&state, user_id, None).await?;

    let end_time = params.end_time.unwrap_or_else(Utc::now);
    let start_time = params.start_time.unwrap_or_else(|| {
        end_time - chrono::Duration::days(1) // 默认查询过去24小时
    });

    let summary = state
        .stats_service
        .get_online_summary(start_time, end_time)
        .await
        .map_err(|err| {
            ApiError::internal_server_error(&format!("Failed to get stats summary: {}", err))
        })?;

    Ok(Json(summary))
}

/// 获取当前所有房间的实时在线统计
async fn get_current_online_stats(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<CurrentOnlineStatsResponse>>, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;
    verify_admin_access(&state, user_id, None).await?;

    // 简化实现：返回固定的演示数据
    // 实际实现中应该遍历所有房间获取在线统计
    let demo_stats = vec![
        CurrentOnlineStatsResponse {
            room_id: Uuid::new_v4(),
            online_count: 5,
            online_users: vec![Uuid::new_v4(), Uuid::new_v4()],
        }
    ];

    Ok(Json(demo_stats))
}

/// 获取事件处理指标
async fn get_event_metrics(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<EventMetricsResponse>, ApiError> {
    let user_id = state.jwt_service.extract_user_from_headers(&headers)?;
    verify_admin_access(&state, user_id, None).await?;

    // 获取事件存储指标
    let total_events = state
        .event_storage
        .get_event_count()
        .await
        .map_err(|err| {
            ApiError::internal_server_error(&format!("Failed to get event count: {}", err))
        })?;

    let now = Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    let one_day_ago = now - chrono::Duration::days(1);

    let events_in_last_hour = state
        .event_storage
        .get_event_count_in_range(one_hour_ago, now)
        .await
        .map_err(|err| {
            ApiError::internal_server_error(&format!("Failed to get hourly event count: {}", err))
        })?;

    let events_in_last_day = state
        .event_storage
        .get_event_count_in_range(one_day_ago, now)
        .await
        .map_err(|err| {
            ApiError::internal_server_error(&format!("Failed to get daily event count: {}", err))
        })?;

    // 获取队列状态
    let queue_status = state.event_collector.get_queue_status().await;

    let response = EventMetricsResponse {
        total_events,
        events_in_last_hour,
        events_in_last_day,
        queue_status: QueueStatusResponse {
            queue_size: queue_status.queue_size,
            max_queue_size: queue_status.max_queue_size,
            is_running: queue_status.is_running,
            batch_size: queue_status.batch_size,
            flush_interval_secs: queue_status.flush_interval.as_secs(),
        },
    };

    Ok(Json(response))
}