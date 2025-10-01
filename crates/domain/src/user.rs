use crate::value_objects::{OrgId, PasswordHash, Timestamp, UserEmail, UserId, Username};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_status")]
#[sqlx(rename_all = "lowercase")]
pub enum UserStatus {
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "inactive")]
    Inactive,
    #[sqlx(rename = "suspended")]
    Suspended,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: UserId,
    pub username: Username,
    pub email: UserEmail,
    #[serde(skip_serializing)] // 密码字段不暴露给客户端
    pub password: PasswordHash,
    pub status: UserStatus,
    pub is_superuser: bool, // 系统级管理员标识
    // 新增组织字段（仅引用，不冗余path）
    pub org_id: Option<OrgId>,
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
            is_superuser: false, // 新注册用户默认不是超级用户
            org_id: None,        // 新注册用户默认不属于任何组织
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

    /// 提升用户为系统管理员
    pub fn grant_superuser(&mut self, now: Timestamp) {
        self.is_superuser = true;
        self.updated_at = now;
    }

    /// 撤销用户的系统管理员权限
    pub fn revoke_superuser(&mut self, now: Timestamp) {
        self.is_superuser = false;
        self.updated_at = now;
    }

    /// 检查用户是否为系统管理员
    pub fn is_system_admin(&self) -> bool {
        self.is_superuser && self.status == UserStatus::Active
    }

    /// 分配到组织
    pub fn assign_to_org(&mut self, org_id: OrgId, now: Timestamp) {
        self.org_id = Some(org_id);
        self.updated_at = now;
    }

    /// 移除组织关联
    pub fn remove_from_org(&mut self, now: Timestamp) {
        self.org_id = None;
        self.updated_at = now;
    }

    /// 检查用户是否属于某个组织
    pub fn belongs_to_org(&self, org_id: OrgId) -> bool {
        match self.org_id {
            Some(user_org_id) => user_org_id == org_id,
            None => false,
        }
    }

    /// 检查用户是否有组织
    pub fn has_org(&self) -> bool {
        self.org_id.is_some()
    }
}
