use crate::value_objects::{PasswordHash, Timestamp, UserEmail, UserId, Username};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_status")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: UserId,
    pub username: Username,
    pub email: UserEmail,
    #[serde(skip_serializing)]  // 密码字段不暴露给客户端
    pub password: PasswordHash,
    pub status: UserStatus,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl User {
    pub fn register(
        id: UserId,
        username: Username,
        email: UserEmail,
        password: PasswordHash,
        now: Timestamp,
    ) -> Self {
        Self {
            id,
            username,
            email,
            password,
            status: UserStatus::Inactive,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn activate(&mut self, now: Timestamp) {
        self.status = UserStatus::Active;
        self.updated_at = now;
    }

    pub fn suspend(&mut self, now: Timestamp) {
        self.status = UserStatus::Suspended;
        self.updated_at = now;
    }

    pub fn update_profile(
        &mut self,
        username: Option<Username>,
        email: Option<UserEmail>,
        now: Timestamp,
    ) {
        if let Some(new_username) = username {
            self.username = new_username;
        }
        if let Some(new_email) = email {
            self.email = new_email;
        }
        self.updated_at = now;
    }

    pub fn set_password(&mut self, password: PasswordHash, now: Timestamp) {
        self.password = password;
        self.updated_at = now;
    }
}
