use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use application::repository::{OrganizationRepository, UserRepository};
use application::services::{BulkCreateUsersRequest, BulkTask, CreateUserRequest, TaskStatus};
use domain::{OrgId, UserId};

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct BulkCreatePayload {
    pub org_id: Uuid,
    pub users: Vec<UserCreateRequest>,
}

#[derive(Debug, Deserialize)]
pub struct UserCreateRequest {
    pub username: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct BulkTaskResponse {
    pub id: Uuid,
    pub task_type: String,
    pub status: String,
    pub total_count: i32,
    pub processed_count: i32,
    pub success_count: i32,
    pub failed_count: i32,
    pub created_at: sqlx::types::time::OffsetDateTime,
    pub started_at: Option<sqlx::types::time::OffsetDateTime>,
    pub completed_at: Option<sqlx::types::time::OffsetDateTime>,
}

impl From<BulkTask> for BulkTaskResponse {
    fn from(task: BulkTask) -> Self {
        Self {
            id: task.id,
            task_type: task.task_type,
            status: task.status.to_string(),
            total_count: task.total_count,
            processed_count: task.processed_count,
            success_count: task.success_count,
            failed_count: task.failed_count,
            created_at: task.created_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
        }
    }
}

pub fn bulk_user_routes() -> Router<AppState> {
    Router::new()
        .route("/bulk", post(bulk_create_users))
        .route("/tasks/:task_id", get(get_task_status))
        .route("/tasks/:task_id/download", get(download_credentials))
}

/// 批量创建用户
async fn bulk_create_users(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<BulkCreatePayload>,
) -> Result<(StatusCode, Json<BulkTaskResponse>), ApiError> {
    // 验证用户身份和权限
    let user_id = UserId::new(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 验证组织是否存在
    let org_id = OrgId::from(payload.org_id);
    let _org = state
        .org_repository
        .find_by_id(org_id)
        .await?
        .ok_or_else(|| ApiError::not_found("目标组织不存在"))?;

    // 验证用户数量
    if payload.users.is_empty() {
        return Err(ApiError::bad_request("用户列表不能为空"));
    }

    if payload.users.len() > 10000 {
        return Err(ApiError::bad_request("单次最多创建10000个用户"));
    }

    // 转换请求
    let request = BulkCreateUsersRequest {
        created_by: user_id,
        org_id,
        users: payload
            .users
            .into_iter()
            .map(|u| CreateUserRequest {
                username: u.username,
                email: u.email,
            })
            .collect(),
    };

    // 提交批量创建任务
    let task = state.bulk_user_service.create_bulk_users(request).await?;

    Ok((StatusCode::ACCEPTED, Json(task.into())))
}

/// 查询批量任务状态
async fn get_task_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Json<BulkTaskResponse>, ApiError> {
    // 验证用户身份
    let user_id = UserId::new(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 查询任务状态
    let task = state
        .bulk_user_service
        .get_task_status(task_id)
        .await?
        .ok_or_else(|| ApiError::not_found("任务不存在"))?;

    Ok(Json(task.into()))
}

/// 下载用户凭证（CSV格式）
async fn download_credentials(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    // 验证用户身份
    let user_id = UserId::new(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 查询任务状态
    let task = state
        .bulk_user_service
        .get_task_status(task_id)
        .await?
        .ok_or_else(|| ApiError::not_found("任务不存在"))?;

    // 检查任务是否完成
    if task.status != TaskStatus::Completed {
        return Err(ApiError::bad_request("任务尚未完成，无法下载凭证"));
    }

    // 下载凭证
    let csv = state
        .bulk_user_service
        .download_credentials(task_id)
        .await?
        .ok_or_else(|| ApiError::not_found("凭证数据不存在"))?;

    // 返回CSV文件
    let filename = format!("credentials_{}.csv", task_id);
    let response = (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        csv,
    )
        .into_response();

    Ok(response)
}
