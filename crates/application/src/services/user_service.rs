use std::sync::Arc;

use domain::{User, UserEmail, UserId, UserStatus, Username};
use uuid::Uuid;

use crate::{
    clock::Clock, error::ApplicationError, password::PasswordHasher, presence::PresenceManager,
    repository::UserRepository,
};

#[derive(Debug, Clone)]
pub struct RegisterUserRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticateUserRequest {
    pub email: String,
    pub password: String,
}

pub struct UserServiceDependencies {
    pub user_repository: Arc<dyn UserRepository>,
    pub password_hasher: Arc<dyn PasswordHasher>,
    pub clock: Arc<dyn Clock>,
    pub presence_manager: Arc<dyn PresenceManager>,
}

pub struct UserService {
    deps: UserServiceDependencies,
}

impl UserService {
    pub fn new(deps: UserServiceDependencies) -> Self {
        Self { deps }
    }

    pub async fn register(&self, request: RegisterUserRequest) -> Result<User, ApplicationError> {
        let username = Username::parse(request.username)?;
        let email = UserEmail::parse(request.email.clone())?;

        if self
            .deps
            .user_repository
            .find_by_email(email.clone())
            .await?
            .is_some()
        {
            return Err(ApplicationError::Domain(
                domain::DomainError::UserAlreadyExists,
            ));
        }

        let password_hash = self.deps.password_hasher.hash(&request.password).await?;

        let now = self.deps.clock.now();
        let mut user = User::register(
            UserId::from(Uuid::new_v4()),
            username,
            email,
            password_hash,
            now,
        );
        user.activate(now);

        let stored = self.deps.user_repository.create(user).await?;
        Ok(stored)
    }

    pub async fn authenticate(
        &self,
        request: AuthenticateUserRequest,
    ) -> Result<User, ApplicationError> {
        let email = UserEmail::parse(request.email)?;
        let user = self
            .deps
            .user_repository
            .find_by_email(email)
            .await?
            .ok_or(ApplicationError::Authentication)?;

        let password_ok = self
            .deps
            .password_hasher
            .verify(&request.password, &user.password)
            .await?;
        if !password_ok {
            return Err(ApplicationError::Authentication);
        }

        if user.status != UserStatus::Active {
            return Err(ApplicationError::Authentication);
        }

        Ok(user)
    }

    pub async fn logout(&self, user_id: Uuid) -> Result<(), ApplicationError> {
        let user_id = UserId::from(user_id);
        self.deps
            .presence_manager
            .cleanup_user_presence(user_id)
            .await
    }
}
