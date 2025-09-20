//! 房间成员Repository实现

use crate::db::DbPool;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    entities::room_member::{RoomMember, MemberRole},
    errors::{DomainError, DomainResult},
    repositories::{
        RoomMemberRepository, RoomMemberSearchParams, RoomMemberStatistics, MemberPermissions,
        Pagination, PaginatedResult, SortConfig,
    },
};
use sqlx::{query, query_as, FromRow, Row};
use std::sync::Arc;
use uuid::Uuid;

/// 辅助函数：将 MemberRole 转换为字符串
fn member_role_to_string(role: MemberRole) -> String {
    match role {
        MemberRole::Owner => "owner".to_string(),
        MemberRole::Admin => "admin".to_string(),
        MemberRole::Member => "member".to_string(),
        MemberRole::Bot => "bot".to_string(),
    }
}
use serde_json::Value as JsonValue;

/// 数据库房间成员模型
#[derive(Debug, Clone, FromRow)]
struct DbRoomMember {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub is_muted: Option<bool>,
    pub notifications_enabled: Option<bool>,
    pub last_read_message_id: Option<Uuid>,
    pub joined_at: DateTime<Utc>,
    pub custom_permissions: Option<String>, // JSON字符串
}

impl From<DbRoomMember> for RoomMember {
    fn from(db_member: DbRoomMember) -> Self {
        let role = match db_member.role.as_str() {
            "owner" => MemberRole::Owner,
            "admin" => MemberRole::Admin,
            "member" => MemberRole::Member,
            "bot" => MemberRole::Bot,
            _ => MemberRole::Member,
        };

        RoomMember::with_time(
            db_member.room_id,
            db_member.user_id,
            role,
            db_member.joined_at,
        )
    }
}

impl From<&RoomMember> for DbRoomMember {
    fn from(member: &RoomMember) -> Self {
        DbRoomMember {
            room_id: member.room_id,
            user_id: member.user_id,
            role: match member.role {
                domain::entities::room_member::MemberRole::Owner => "owner".to_string(),
                domain::entities::room_member::MemberRole::Admin => "admin".to_string(),
                domain::entities::room_member::MemberRole::Member => "member".to_string(),
                domain::entities::room_member::MemberRole::Bot => "bot".to_string(),
            },
            is_muted: Some(false), // 默认未静音
            notifications_enabled: Some(true), // 默认启用通知
            last_read_message_id: None,
            joined_at: member.joined_at,
            custom_permissions: None,
        }
    }
}

/// 房间成员Repository实现
pub struct PostgresRoomMemberRepository {
    pool: Arc<DbPool>,
}

impl PostgresRoomMemberRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    /// 构建搜索查询条件
    fn build_search_query(params: &RoomMemberSearchParams) -> (String, Vec<String>) {
        let mut conditions = Vec::new();
        let mut values = Vec::new();
        let mut param_count = 1;

        if let Some(room_id) = &params.room_id {
            conditions.push(format!("room_id = ${}", param_count));
            values.push(room_id.to_string());
            param_count += 1;
        }

        if let Some(user_id) = &params.user_id {
            conditions.push(format!("user_id = ${}", param_count));
            values.push(user_id.to_string());
            param_count += 1;
        }

        if let Some(role) = &params.role {
            conditions.push(format!("role = ${}", param_count));
            values.push(member_role_to_string(role.clone()));
            param_count += 1;
        }

        if let Some(is_muted) = params.is_muted {
            conditions.push(format!("is_muted = ${}", param_count));
            values.push(is_muted.to_string());
            param_count += 1;
        }

        if let Some(notifications_enabled) = params.notifications_enabled {
            conditions.push(format!("notifications_enabled = ${}", param_count));
            values.push(notifications_enabled.to_string());
            param_count += 1;
        }

        if let Some(joined_after) = &params.joined_after {
            conditions.push(format!("joined_at > ${}", param_count));
            values.push(joined_after.to_string());
            param_count += 1;
        }

        if let Some(joined_before) = &params.joined_before {
            conditions.push(format!("joined_at < ${}", param_count));
            values.push(joined_before.to_string());
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        (where_clause, values)
    }
}

#[async_trait]
impl RoomMemberRepository for PostgresRoomMemberRepository {
    async fn add_member(&self, member: &RoomMember) -> DomainResult<RoomMember> {
        let db_member = DbRoomMember::from(member);

        let result = query_as::<_, DbRoomMember>(
            r#"
            INSERT INTO room_members (room_id, user_id, role, is_muted, notifications_enabled, joined_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            "#,
        )
        .bind(db_member.room_id)
        .bind(db_member.user_id)
        .bind(&db_member.role)
        .bind(db_member.is_muted)
        .bind(db_member.notifications_enabled)
        .bind(db_member.joined_at)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn find_by_room_and_user(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<Option<RoomMember>> {
        let result = query_as::<_, DbRoomMember>(
            r#"
            SELECT room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            FROM room_members
            WHERE room_id = $1 AND user_id = $2
            "#,
        )
        .bind(room_id)
        .bind(user_id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.map(|m| m.into()))
    }

    async fn update(&self, member: &RoomMember) -> DomainResult<RoomMember> {
        let db_member = DbRoomMember::from(member);

        let result = query_as::<_, DbRoomMember>(
            r#"
            UPDATE room_members
            SET role = $3
            WHERE room_id = $1 AND user_id = $2
            RETURNING room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            "#,
        )
        .bind(db_member.room_id)
        .bind(db_member.user_id)
        .bind(&db_member.role)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.into())
    }

    async fn update_role(&self, room_id: Uuid, user_id: Uuid, role: MemberRole) -> DomainResult<()> {
        query("UPDATE room_members SET role = $3 WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(user_id)
            .bind(member_role_to_string(role))
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn set_muted(&self, room_id: Uuid, user_id: Uuid, is_muted: bool) -> DomainResult<()> {
        query("UPDATE room_members SET is_muted = $3 WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(user_id)
            .bind(is_muted)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn set_notifications(&self, room_id: Uuid, user_id: Uuid, enabled: bool) -> DomainResult<()> {
        query("UPDATE room_members SET notifications_enabled = $3 WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(user_id)
            .bind(enabled)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn update_last_read(&self, room_id: Uuid, user_id: Uuid, message_id: Uuid) -> DomainResult<()> {
        query("UPDATE room_members SET last_read_message_id = $3 WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(user_id)
            .bind(message_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn remove_member(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<bool> {
        let result = query("DELETE FROM room_members WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(user_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn find_by_room(&self, room_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<RoomMember>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM room_members WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取成员
        let members: Vec<DbRoomMember> = query_as(
            r#"
            SELECT room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            FROM room_members
            WHERE room_id = $1
            ORDER BY joined_at ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(room_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let members: Vec<RoomMember> = members.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(members, total_count as u64, pagination))
    }

    async fn find_by_user(&self, user_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<RoomMember>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM room_members WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取成员
        let members: Vec<DbRoomMember> = query_as(
            r#"
            SELECT room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            FROM room_members
            WHERE user_id = $1
            ORDER BY joined_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let members: Vec<RoomMember> = members.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(members, total_count as u64, pagination))
    }

    async fn find_by_room_and_role(
        &self,
        room_id: Uuid,
        role: MemberRole,
        pagination: Pagination,
    ) -> DomainResult<PaginatedResult<RoomMember>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM room_members WHERE room_id = $1 AND role = $2")
            .bind(room_id)
            .bind(member_role_to_string(role.clone()))
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取成员
        let members: Vec<DbRoomMember> = query_as(
            r#"
            SELECT room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            FROM room_members
            WHERE room_id = $1 AND role = $2
            ORDER BY joined_at ASC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(room_id)
        .bind(member_role_to_string(role))
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let members: Vec<RoomMember> = members.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(members, total_count as u64, pagination))
    }

    async fn is_member(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<bool> {
        let count: i64 = query("SELECT COUNT(*) FROM room_members WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(user_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        Ok(count > 0)
    }

    async fn check_permission(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        _permission: &str,
    ) -> DomainResult<bool> {
        // 简化实现：根据角色判断权限
        let member = self.find_by_room_and_user(room_id, user_id).await?;
        match member {
            Some(member) => Ok(member.is_admin()),
            None => Ok(false),
        }
    }

    async fn get_member_permissions(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<MemberPermissions> {
        let member = self.find_by_room_and_user(room_id, user_id).await?;
        match member {
            Some(member) => {
                let is_owner = member.is_owner();
                let is_admin = member.is_admin();

                Ok(MemberPermissions {
                    can_send_messages: true, // 所有成员都可以发消息
                    can_edit_messages: true, // 可以编辑自己的消息
                    can_delete_messages: is_admin, // 管理员可以删除消息
                    can_kick_members: is_admin,
                    can_ban_members: is_admin,
                    can_change_roles: is_owner, // 只有房主可以修改角色
                    can_edit_room_info: is_admin,
                    can_delete_room: is_owner,
                })
            }
            None => {
                // 非成员没有任何权限
                Ok(MemberPermissions {
                    can_send_messages: false,
                    can_edit_messages: false,
                    can_delete_messages: false,
                    can_kick_members: false,
                    can_ban_members: false,
                    can_change_roles: false,
                    can_edit_room_info: false,
                    can_delete_room: false,
                })
            }
        }
    }

    async fn count_by_room(&self, room_id: Uuid) -> DomainResult<u64> {
        let count: i64 = query("SELECT COUNT(*) FROM room_members WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        Ok(count as u64)
    }

    async fn count_online_by_room(&self, room_id: Uuid) -> DomainResult<u64> {
        // 需要与用户表关联查询在线用户
        let count: i64 = query(
            r#"
            SELECT COUNT(*)
            FROM room_members rm
            JOIN users u ON rm.user_id = u.id
            WHERE rm.room_id = $1 AND u.last_active_at > NOW() - INTERVAL '15 minutes'
            "#
        )
        .bind(room_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?
        .get(0);

        Ok(count as u64)
    }

    async fn find_admins_by_room(&self, room_id: Uuid) -> DomainResult<Vec<RoomMember>> {
        let members: Vec<DbRoomMember> = query_as(
            r#"
            SELECT room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            FROM room_members
            WHERE room_id = $1 AND (role = 'owner' OR role = 'admin')
            ORDER BY
                CASE role
                    WHEN 'owner' THEN 1
                    WHEN 'admin' THEN 2
                    ELSE 3
                END,
                joined_at ASC
            "#,
        )
        .bind(room_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(members.into_iter().map(|m| m.into()).collect())
    }

    async fn find_moderators_by_room(&self, room_id: Uuid) -> DomainResult<Vec<RoomMember>> {
        // 在这个简化的实现中，版主等同于管理员
        self.find_admins_by_room(room_id).await
    }

    async fn add_members_batch(&self, members: &[RoomMember]) -> DomainResult<Vec<RoomMember>> {
        let mut added_members = Vec::new();

        for member in members {
            match self.add_member(member).await {
                Ok(added_member) => added_members.push(added_member),
                Err(_) => continue, // 跳过添加失败的成员
            }
        }

        Ok(added_members)
    }

    async fn remove_members_batch(&self, room_id: Uuid, user_ids: &[Uuid]) -> DomainResult<u64> {
        let mut removed_count = 0;

        for user_id in user_ids {
            if self.remove_member(room_id, *user_id).await.unwrap_or(false) {
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }

    async fn search(
        &self,
        params: &RoomMemberSearchParams,
        pagination: Pagination,
        sort: Option<SortConfig>,
    ) -> DomainResult<PaginatedResult<RoomMember>> {
        let (where_clause, _values) = Self::build_search_query(params);

        let order_clause = match sort {
            Some(sort_config) => {
                let direction = if sort_config.ascending { "ASC" } else { "DESC" };
                format!("ORDER BY {} {}", sort_config.field, direction)
            }
            None => "ORDER BY joined_at DESC".to_string(),
        };

        let base_query = format!(
            "FROM room_members {}",
            if where_clause.is_empty() { "WHERE 1=1".to_string() } else { where_clause }
        );

        // 获取总数
        let count_query = format!("SELECT COUNT(*) {}", base_query);
        let total_count: i64 = query(&count_query)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取数据
        let data_query = format!(
            r#"
            SELECT room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            {} {} LIMIT {} OFFSET {}
            "#,
            base_query, order_clause, pagination.limit, pagination.offset
        );

        let members: Vec<DbRoomMember> = query_as(&data_query)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        let members: Vec<RoomMember> = members.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(members, total_count as u64, pagination))
    }

    async fn get_statistics(&self, room_id: Uuid) -> DomainResult<RoomMemberStatistics> {
        let row = query(
            r#"
            SELECT
                COUNT(*) as total_members,
                COUNT(*) FILTER (WHERE role = 'admin' OR role = 'owner') as admins,
                COUNT(*) FILTER (WHERE role = 'admin') as moderators,
                COUNT(*) FILTER (WHERE role = 'member') as regular_members,
                COUNT(*) FILTER (WHERE is_muted = true) as muted_members,
                COUNT(*) FILTER (WHERE joined_at::date = CURRENT_DATE) as members_joined_today
            FROM room_members rm
            WHERE rm.room_id = $1
            "#,
        )
        .bind(room_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        // 获取在线成员数
        let online_members = self.count_online_by_room(room_id).await?;

        Ok(RoomMemberStatistics {
            total_members: row.get::<i64, _>("total_members") as u64,
            admins: row.get::<i64, _>("admins") as u64,
            moderators: row.get::<i64, _>("moderators") as u64,
            regular_members: row.get::<i64, _>("regular_members") as u64,
            muted_members: row.get::<i64, _>("muted_members") as u64,
            online_members,
            members_joined_today: row.get::<i64, _>("members_joined_today") as u64,
        })
    }

    async fn get_unread_count(&self, room_id: Uuid, user_id: Uuid) -> DomainResult<u64> {
        // 这需要查询消息表来计算未读消息数量
        let row = query(
            r#"
            SELECT COUNT(*) as unread_count
            FROM messages m
            LEFT JOIN room_members rm ON rm.room_id = m.room_id AND rm.user_id = $2
            WHERE m.room_id = $1
            AND m.status != 'deleted'
            AND (rm.last_read_message_id IS NULL OR m.created_at > (
                SELECT created_at FROM messages WHERE id = rm.last_read_message_id
            ))
            "#
        )
        .bind(room_id)
        .bind(user_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(row.get::<i64, _>("unread_count") as u64)
    }

    async fn find_active_members(
        &self,
        room_id: Uuid,
        since: DateTime<Utc>,
        limit: u32,
    ) -> DomainResult<Vec<RoomMember>> {
        // 查找最近有消息的成员
        let members: Vec<DbRoomMember> = query_as(
            r#"
            SELECT DISTINCT rm.room_id, rm.user_id, rm.role, rm.is_muted, rm.notifications_enabled,
                   rm.last_read_message_id, rm.joined_at, rm.custom_permissions
            FROM room_members rm
            JOIN messages m ON rm.user_id = m.sender_id AND rm.room_id = m.room_id
            WHERE rm.room_id = $1 AND m.created_at > $2 AND m.status != 'deleted'
            ORDER BY MAX(m.created_at) DESC
            LIMIT $3
            "#,
        )
        .bind(room_id)
        .bind(since)
        .bind(limit as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(members.into_iter().map(|m| m.into()).collect())
    }

    async fn update_permissions(&self, room_id: Uuid, user_id: Uuid, permissions: &JsonValue) -> DomainResult<()> {
        query("UPDATE room_members SET custom_permissions = $3 WHERE room_id = $1 AND user_id = $2")
            .bind(room_id)
            .bind(user_id)
            .bind(permissions.to_string())
            .execute(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(())
    }

    async fn find_muted_members(&self, room_id: Uuid, pagination: Pagination) -> DomainResult<PaginatedResult<RoomMember>> {
        // 获取总数
        let total_count: i64 = query("SELECT COUNT(*) FROM room_members WHERE room_id = $1 AND is_muted = true")
            .bind(room_id)
            .fetch_one(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(e.to_string()))?
            .get(0);

        // 获取静音成员
        let members: Vec<DbRoomMember> = query_as(
            r#"
            SELECT room_id, user_id, role, is_muted, notifications_enabled, last_read_message_id, joined_at, custom_permissions
            FROM room_members
            WHERE room_id = $1 AND is_muted = true
            ORDER BY joined_at ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(room_id)
        .bind(pagination.limit as i32)
        .bind(pagination.offset as i32)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        let members: Vec<RoomMember> = members.into_iter().map(|m| m.into()).collect();

        Ok(PaginatedResult::new(members, total_count as u64, pagination))
    }

    async fn cleanup_inactive_members(&self, inactive_days: u32) -> DomainResult<u64> {
        // 清理不活跃成员：删除最后活跃时间超过指定天数且不是管理员的成员
        let result = query(
            r#"
            DELETE FROM room_members
            WHERE role = 'member'
            AND user_id IN (
                SELECT u.id
                FROM users u
                WHERE u.last_active_at < NOW() - INTERVAL '$1 days'
            )
            "#
        )
        .bind(inactive_days as i32)
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(e.to_string()))?;

        Ok(result.rows_affected())
    }
}

