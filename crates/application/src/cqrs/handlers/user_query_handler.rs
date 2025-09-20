//! 用户查询处理器
//!
//! 处理用户相关的查询：根据 ID 查找、搜索用户等

use super::user_command_handler::UserRepository;
use crate::cqrs::{dtos::*, queries::*, QueryHandler};
use crate::errors::ApplicationResult;
use async_trait::async_trait;
use domain::entities::user::User;
use std::sync::Arc;

/// 用户查询处理器
pub struct UserQueryHandler {
    user_repository: Arc<dyn UserRepository>,
}

impl UserQueryHandler {
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    /// 将 User 实体转换为 UserDto
    fn user_to_dto(&self, user: User) -> UserDto {
        UserDto {
            id: user.id,
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            avatar_url: user.avatar_url,
            status: user.status,
            created_at: user.created_at,
            updated_at: user.updated_at,
            last_activity_at: user.last_active_at,
        }
    }
}

#[async_trait]
impl QueryHandler<GetUserByIdQuery> for UserQueryHandler {
    async fn handle(&self, query: GetUserByIdQuery) -> ApplicationResult<Option<UserDto>> {
        let user = self.user_repository.find_by_id(query.user_id).await?;
        Ok(user.map(|u| self.user_to_dto(u)))
    }
}

#[async_trait]
impl QueryHandler<GetUserByUsernameQuery> for UserQueryHandler {
    async fn handle(&self, query: GetUserByUsernameQuery) -> ApplicationResult<Option<UserDto>> {
        let user = self
            .user_repository
            .find_by_username(query.username)
            .await?;
        Ok(user.map(|u| self.user_to_dto(u)))
    }
}

#[async_trait]
impl QueryHandler<GetUserByEmailQuery> for UserQueryHandler {
    async fn handle(&self, query: GetUserByEmailQuery) -> ApplicationResult<Option<UserDto>> {
        let user = self.user_repository.find_by_email(query.email).await?;
        Ok(user.map(|u| self.user_to_dto(u)))
    }
}

#[async_trait]
impl QueryHandler<SearchUsersQuery> for UserQueryHandler {
    async fn handle(&self, query: SearchUsersQuery) -> ApplicationResult<Vec<UserDto>> {
        // 简化实现：暂时返回空列表
        // 实际应该根据 keyword、status、limit、offset 进行搜索
        let _keyword = query.keyword;
        let _status = query.status_filter;
        let _limit = query.limit;
        let _offset = query.offset;

        Ok(Vec::new())
    }
}

#[async_trait]
impl QueryHandler<GetUserProfileQuery> for UserQueryHandler {
    async fn handle(
        &self,
        query: GetUserProfileQuery,
    ) -> ApplicationResult<Option<UserProfileDto>> {
        let user = self.user_repository.find_by_id(query.user_id).await?;

        match user {
            Some(user) => {
                let user_dto = self.user_to_dto(user);

                // 构建用户资料 DTO（简化实现）
                let profile = UserProfileDto {
                    user: user_dto,
                    organizations: Vec::new(), // 实际应该从组织服务获取
                    rooms: Vec::new(),         // 实际应该从聊天室服务获取
                    statistics: UserStatisticsDto {
                        total_rooms_created: 0,
                        total_messages_sent: 0,
                        total_organizations_joined: 0,
                        last_active_at: chrono::Utc::now(),
                        total_users: 0,
                        active_users: 0,
                        online_users: 0,
                    },
                };

                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }
}
