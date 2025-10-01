use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use application::repository::{OrganizationRepository, PaginationParams, UserRepository};
use domain::{OrgId, Organization, Timestamp, UserId};

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Serialize)]
pub struct OrganizationResponse {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub level: i32,
    pub parent_path: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: sqlx::types::time::OffsetDateTime,
    pub updated_at: sqlx::types::time::OffsetDateTime,
}

impl From<Organization> for OrganizationResponse {
    fn from(org: Organization) -> Self {
        Self {
            id: Uuid::from(org.id),
            name: org.name.clone(),
            level: org.level(),
            parent_path: org.parent_path().map(|p| p.to_string()),
            path: org.path.to_string(),
            metadata: org.metadata.clone(),
            created_at: org.created_at,
            updated_at: org.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationPayload {
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrganizationPayload {
    pub name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct MoveOrganizationPayload {
    pub new_parent_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ListOrganizationsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct OrganizationTreeResponse {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub metadata: Option<serde_json::Value>,
    pub children: Vec<OrganizationTreeResponse>,
}

pub fn org_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_organization))
        .route("/", get(list_organizations))
        .route("/{org_id}", get(get_organization))
        .route("/{org_id}", patch(update_organization))
        .route("/{org_id}", delete(delete_organization))
        .route("/{org_id}/tree", get(get_organization_tree))
        .route("/{org_id}/move", post(move_organization))
}

/// 创建组织
async fn create_organization(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<CreateOrganizationPayload>,
) -> Result<(StatusCode, Json<OrganizationResponse>), ApiError> {
    // 验证用户身份和权限
    let user_id = UserId::from(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 获取父组织路径
    let parent_path = if let Some(parent_id) = payload.parent_id {
        let parent = state
            .org_repository
            .find_by_id(OrgId::from(parent_id))
            .await?
            .ok_or_else(|| ApiError::not_found("父组织不存在"))?;
        Some(parent.path)
    } else {
        None
    };

    // 创建组织
    let now = Timestamp::now_utc();
    let mut org = Organization::new(OrgId::new(), payload.name, parent_path.as_ref(), now)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    // 设置元数据
    if let Some(metadata) = payload.metadata {
        org.update_metadata(Some(metadata), now);
    }

    // 保存到数据库
    state.org_repository.create(&org).await?;

    Ok((StatusCode::CREATED, Json(org.into())))
}

/// 获取组织列表
async fn list_organizations(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<ListOrganizationsQuery>,
) -> Result<Json<Vec<OrganizationResponse>>, ApiError> {
    // 验证用户身份
    let user_id = UserId::from(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 查询组织列表
    let limit = query.limit.unwrap_or(50).min(200); // 最多返回200个
    let params = if let Some(offset) = query.offset {
        PaginationParams::with_offset(limit, offset)
    } else {
        PaginationParams::new(limit)
    };

    let organizations = state.org_repository.list_with_pagination(params).await?;
    let responses: Vec<OrganizationResponse> = organizations.into_iter().map(Into::into).collect();

    Ok(Json(responses))
}

/// 获取单个组织
async fn get_organization(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<OrganizationResponse>, ApiError> {
    // 验证用户身份
    let user_id = UserId::from(state.jwt_service.extract_user_from_headers(&headers)?);
    let _user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    // 查询组织
    let org = state
        .org_repository
        .find_by_id(OrgId::from(org_id))
        .await?
        .ok_or_else(|| ApiError::not_found("组织不存在"))?;

    Ok(Json(org.into()))
}

/// 更新组织
async fn update_organization(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<UpdateOrganizationPayload>,
) -> Result<Json<OrganizationResponse>, ApiError> {
    // 验证用户身份和权限
    let user_id = UserId::from(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 查询组织
    let mut org = state
        .org_repository
        .find_by_id(OrgId::from(org_id))
        .await?
        .ok_or_else(|| ApiError::not_found("组织不存在"))?;

    let now = Timestamp::now_utc();

    // 更新名称
    if let Some(name) = payload.name {
        org.rename(name, now)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
    }

    // 更新元数据
    if payload.metadata.is_some() {
        org.update_metadata(payload.metadata, now);
    }

    // 保存到数据库
    state.org_repository.update(&org).await?;

    Ok(Json(org.into()))
}

/// 删除组织
async fn delete_organization(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // 验证用户身份和权限
    let user_id = UserId::from(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 检查组织是否存在
    let org = state
        .org_repository
        .find_by_id(OrgId::from(org_id))
        .await?
        .ok_or_else(|| ApiError::not_found("组织不存在"))?;

    // 检查是否有子组织
    let children = state
        .org_repository
        .find_children(&org.path.to_string())
        .await?;
    if !children.is_empty() {
        return Err(ApiError::bad_request(
            "无法删除有子组织的组织，请先删除所有子组织",
        ));
    }

    // 检查是否有用户
    let users = state
        .org_repository
        .find_users_in_organization(org.id)
        .await?;
    if !users.is_empty() {
        return Err(ApiError::bad_request(format!(
            "无法删除有{}个用户的组织，请先移除所有用户",
            users.len()
        )));
    }

    // 删除组织
    state.org_repository.delete(org.id).await?;

    Ok(StatusCode::NO_CONTENT)
}

/// 获取组织树（包含所有后代）
async fn get_organization_tree(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<OrganizationTreeResponse>, ApiError> {
    // 验证用户身份
    let user_id = UserId::from(state.jwt_service.extract_user_from_headers(&headers)?);
    let _user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    // 查询组织
    let org = state
        .org_repository
        .find_by_id(OrgId::from(org_id))
        .await?
        .ok_or_else(|| ApiError::not_found("组织不存在"))?;

    // 查询所有后代
    let descendants = state
        .org_repository
        .find_descendants(&org.path.to_string())
        .await?;

    // 构建树形结构
    let tree = build_organization_tree(&org, &descendants);

    Ok(Json(tree))
}

/// 移动组织到新的父组织
async fn move_organization(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<MoveOrganizationPayload>,
) -> Result<Json<OrganizationResponse>, ApiError> {
    // 验证用户身份和权限
    let user_id = UserId::from(state.jwt_service.extract_user_from_headers(&headers)?);
    let user = state
        .storage
        .user_repository
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户未登录"))?;

    if !user.is_system_admin() {
        return Err(ApiError::forbidden("需要系统管理员权限"));
    }

    // 检查目标父组织是否存在
    let new_parent = state
        .org_repository
        .find_by_id(OrgId::from(payload.new_parent_id))
        .await?
        .ok_or_else(|| ApiError::not_found("目标父组织不存在"))?;

    // 移动组织
    state
        .org_repository
        .move_organization(OrgId::from(org_id), &new_parent.path.to_string())
        .await?;

    // 查询更新后的组织
    let org = state
        .org_repository
        .find_by_id(OrgId::from(org_id))
        .await?
        .ok_or_else(|| ApiError::not_found("组织不存在"))?;

    Ok(Json(org.into()))
}

/// 构建组织树形结构
fn build_organization_tree(
    root: &Organization,
    all_descendants: &[Organization],
) -> OrganizationTreeResponse {
    let root_path_str = root.path.to_string();
    let root_level = root.level();

    // 找出直接子节点
    let children: Vec<OrganizationTreeResponse> = all_descendants
        .iter()
        .filter(|org| {
            // 检查是否是直接子节点：level 等于 root_level + 1，且路径以 root_path 开头
            org.level() == root_level + 1
                && org
                    .path
                    .to_string()
                    .starts_with(&format!("{}.", root_path_str))
        })
        .map(|child| build_organization_tree(child, all_descendants))
        .collect();

    OrganizationTreeResponse {
        id: Uuid::from(root.id),
        name: root.name.clone(),
        path: root_path_str,
        metadata: root.metadata.clone(),
        children,
    }
}
