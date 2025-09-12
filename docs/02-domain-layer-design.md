# 领域层设计

## 🎯 领域层概述

领域层是系统的核心，包含业务逻辑、实体、值对象、领域服务和领域事件。该层独立于任何具体技术实现，纯粹表达业务概念和规则。

## 🏗️ 核心实体

### 用户实体 (User)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserStatus {
    Active,     // 活跃状态
    Inactive,   // 未激活
    Suspended,  // 暂停
    Deleted,    // 已删除
}

impl User {
    /// 创建新用户
    pub fn new(username: String, email: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            username,
            email,
            avatar_url: None,
            status: UserStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 更新用户头像
    pub fn update_avatar(&mut self, avatar_url: Option<String>) {
        self.avatar_url = avatar_url;
        self.updated_at = Utc::now();
    }
    
    /// 激活用户
    pub fn activate(&mut self) {
        self.status = UserStatus::Active;
        self.updated_at = Utc::now();
    }
    
    /// 暂停用户
    pub fn suspend(&mut self) {
        self.status = UserStatus::Suspended;
        self.updated_at = Utc::now();
    }
}
```

### 聊天室实体 (ChatRoom)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use bcrypt::{hash, verify, DEFAULT_COST};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRoom {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_private: bool,
    pub password_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChatRoom {
    /// 创建公开房间
    pub fn new_public(name: String, description: Option<String>, owner_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            owner_id,
            is_private: false,
            password_hash: None,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 创建私密房间
    pub fn new_private(
        name: String, 
        description: Option<String>, 
        owner_id: Uuid, 
        password: &str
    ) -> Result<Self> {
        let now = Utc::now();
        let password_hash = Some(hash(password, DEFAULT_COST)?);
        
        Ok(Self {
            id: Uuid::new_v4(),
            name,
            description,
            owner_id,
            is_private: true,
            password_hash,
            created_at: now,
            updated_at: now,
        })
    }
    
    /// 验证房间密码
    pub fn verify_password(&self, password: &str) -> Result<bool> {
        match &self.password_hash {
            Some(hash) => verify(password, hash).map_err(|e| anyhow::anyhow!("密码验证失败: {}", e)),
            None => Ok(true), // 公开房间无需密码
        }
    }
    
    /// 更新房间信息
    pub fn update_info(&mut self, name: Option<String>, description: Option<String>) {
        if let Some(name) = name {
            self.name = name;
        }
        if let Some(description) = description {
            self.description = description;
        }
        self.updated_at = Utc::now();
    }
    
    /// 更改房间密码
    pub fn change_password(&mut self, new_password: Option<&str>) -> Result<()> {
        match new_password {
            Some(password) => {
                self.password_hash = Some(hash(password, DEFAULT_COST)?);
                self.is_private = true;
            }
            None => {
                self.password_hash = None;
                self.is_private = false;
            }
        }
        self.updated_at = Utc::now();
        Ok(())
    }
}
```

### 消息实体 (Message)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to_id: Option<Uuid>,
    pub is_bot_message: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    Text,                    // 文本消息
    Image {                  // 图片消息
        url: String,
        thumbnail: Option<String>,
    },
    File {                   // 文件消息
        url: String,
        filename: String,
        size: u64,
    },
    Emoji {                  // 表情消息
        emoji_id: String,
    },
}

impl Message {
    /// 创建文本消息
    pub fn new_text(room_id: Uuid, user_id: Uuid, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            room_id,
            user_id,
            content,
            message_type: MessageType::Text,
            reply_to_id: None,
            is_bot_message: false,
            created_at: now,
            updated_at: None,
        }
    }
    
    /// 创建回复消息
    pub fn new_reply(room_id: Uuid, user_id: Uuid, content: String, reply_to_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            room_id,
            user_id,
            content,
            message_type: MessageType::Text,
            reply_to_id: Some(reply_to_id),
            is_bot_message: false,
            created_at: now,
            updated_at: None,
        }
    }
    
    /// 创建机器人消息
    pub fn new_bot_message(room_id: Uuid, user_id: Uuid, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            room_id,
            user_id,
            content,
            message_type: MessageType::Text,
            reply_to_id: None,
            is_bot_message: true,
            created_at: now,
            updated_at: None,
        }
    }
    
    /// 更新消息内容
    pub fn update_content(&mut self, content: String) {
        self.content = content;
        self.updated_at = Some(Utc::now());
    }
}
```

### 房间成员实体 (RoomMember)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomMember {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberRole {
    Owner,    // 房主
    Admin,    // 管理员
    Member,   // 普通成员
    Bot,      // 机器人
}

impl RoomMember {
    /// 创建房间成员
    pub fn new(room_id: Uuid, user_id: Uuid, role: MemberRole) -> Self {
        Self {
            room_id,
            user_id,
            role,
            joined_at: Utc::now(),
        }
    }
    
    /// 检查是否为管理员
    pub fn is_admin(&self) -> bool {
        matches!(self.role, MemberRole::Owner | MemberRole::Admin)
    }
    
    /// 检查是否为房主
    pub fn is_owner(&self) -> bool {
        matches!(self.role, MemberRole::Owner)
    }
    
    /// 更新角色
    pub fn update_role(&mut self, role: MemberRole) {
        self.role = role;
    }
}
```

## 🏢 企业级扩展实体

### 组织实体 (Organization)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub level: i32,
    pub is_banned: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub banned_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Organization {
    /// 创建新组织
    pub fn new(name: String, parent_id: Option<Uuid>) -> Self {
        let now = Utc::now();
        let level = if parent_id.is_some() { 1 } else { 0 };
        
        Self {
            id: Uuid::new_v4(),
            name,
            parent_id,
            level,
            is_banned: false,
            banned_at: None,
            banned_by: None,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 禁止组织
    pub fn ban(&mut self, banned_by: Uuid) {
        self.is_banned = true;
        self.banned_at = Some(Utc::now());
        self.banned_by = Some(banned_by);
        self.updated_at = Utc::now();
    }
    
    /// 解除禁止
    pub fn unban(&mut self) {
        self.is_banned = false;
        self.banned_at = None;
        self.banned_by = None;
        self.updated_at = Utc::now();
    }
    
    /// 检查是否有子组织
    pub fn has_children(&self) -> bool {
        self.level < 10 // 假设最多10层
    }
    
    /// 获取组织层级路径
    pub fn get_path(&self) -> String {
        format!("{}-{}", self.level, self.id)
    }
}
```

### 用户组织关系 (UserOrganization)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOrganization {
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub joined_at: DateTime<Utc>,
}

impl UserOrganization {
    pub fn new(user_id: Uuid, organization_id: Uuid) -> Self {
        Self {
            user_id,
            organization_id,
            joined_at: Utc::now(),
        }
    }
}
```

### 用户角色实体 (UserRole)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRole {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<Permission>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Permission {
    // 聊天室权限
    CreateRoom,
    JoinPrivateRoom,
    ManageRoom,
    SendMessage,
    DeleteMessage,
    
    // 管理员权限
    ManageUsers,
    ManageOrganizations,
    ViewAuditLogs,
    
    // 代理权限
    ActAsProxy,
    SetProxy,
}

impl UserRole {
    pub fn new(name: String, description: Option<String>, permissions: Vec<Permission>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            permissions,
            created_at: Utc::now(),
        }
    }
    
    /// 检查是否有指定权限
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }
    
    /// 添加权限
    pub fn add_permission(&mut self, permission: Permission) {
        if !self.has_permission(&permission) {
            self.permissions.push(permission);
        }
    }
    
    /// 移除权限
    pub fn remove_permission(&mut self, permission: &Permission) {
        self.permissions.retain(|p| p != permission);
    }
}
```

### 用户角色分配 (UserRoleAssignment)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRoleAssignment {
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub assigned_at: DateTime<Utc>,
    pub assigned_by: Uuid,
}

impl UserRoleAssignment {
    pub fn new(user_id: Uuid, role_id: Uuid, assigned_by: Uuid) -> Self {
        Self {
            user_id,
            role_id,
            assigned_at: Utc::now(),
            assigned_by,
        }
    }
}
```

### 部门实体 (Department)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Department {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub manager_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Department {
    pub fn new(name: String, description: Option<String>, parent_id: Option<Uuid>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            parent_id,
            manager_id: None,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 设置部门经理
    pub fn set_manager(&mut self, manager_id: Uuid) {
        self.manager_id = Some(manager_id);
        self.updated_at = Utc::now();
    }
    
    /// 检查是否为根部门
    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }
}
```

### 职位实体 (Position)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub level: i32,
    pub department_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl Position {
    pub fn new(title: String, description: Option<String>, level: i32, department_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            description,
            level,
            department_id,
            created_at: Utc::now(),
        }
    }
}
```

### 用户职位关系 (UserPosition)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPosition {
    pub user_id: Uuid,
    pub position_id: Uuid,
    pub department_id: Uuid,
    pub is_primary: bool,
    pub start_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
}

impl UserPosition {
    pub fn new(
        user_id: Uuid, 
        position_id: Uuid, 
        department_id: Uuid, 
        is_primary: bool
    ) -> Self {
        Self {
            user_id,
            position_id,
            department_id,
            is_primary,
            start_date: Utc::now(),
            end_date: None,
        }
    }
    
    /// 设置为主要职位
    pub fn set_primary(&mut self) {
        self.is_primary = true;
    }
    
    /// 结束职位
    pub fn end_position(&mut self) {
        self.end_date = Some(Utc::now());
    }
    
    /// 检查是否为活跃职位
    pub fn is_active(&self) -> bool {
        self.end_date.is_none()
    }
}
```

### 代理关系实体 (UserProxy)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProxy {
    pub id: Uuid,
    pub principal_id: Uuid,         // 委托人
    pub proxy_id: Uuid,             // 代理人
    pub proxy_type: ProxyType,
    pub permissions: Vec<Permission>,
    pub start_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyType {
    Temporary,      // 临时代理
    Permanent,      // 永久代理
    Emergency,      // 紧急代理
    Vacation,       // 休假代理
}

impl UserProxy {
    pub fn new(
        principal_id: Uuid,
        proxy_id: Uuid,
        proxy_type: ProxyType,
        permissions: Vec<Permission>
    ) -> Result<Self> {
        if principal_id == proxy_id {
            return Err(anyhow::anyhow!("委托人和代理人不能为同一用户"));
        }
        
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            principal_id,
            proxy_id,
            proxy_type,
            permissions,
            start_date: now,
            end_date: None,
            is_active: true,
            created_at: now,
        })
    }
    
    /// 激活代理关系
    pub fn activate(&mut self) {
        self.is_active = true;
    }
    
    /// 停用代理关系
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }
    
    /// 设置结束日期
    pub fn set_end_date(&mut self, end_date: DateTime<Utc>) {
        self.end_date = Some(end_date);
        if end_date <= Utc::now() {
            self.is_active = false;
        }
    }
    
    /// 检查是否有指定权限
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }
    
    /// 检查是否为活跃代理
    pub fn is_active_proxy(&self) -> bool {
        self.is_active && 
        self.end_date.map_or(true, |end| end > Utc::now())
    }
}
```

### 用户在线统计实体 (UserOnlineStatistics)

```rust
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOnlineStatistics {
    pub id: Uuid,
    pub user_id: Uuid,
    pub date: NaiveDate,
    pub total_online_seconds: i64,
    pub session_count: i32,
    pub first_login_at: Option<DateTime<Utc>>,
    pub last_logout_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserOnlineStatistics {
    pub fn new(user_id: Uuid, date: NaiveDate) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            date,
            total_online_seconds: 0,
            session_count: 0,
            first_login_at: None,
            last_logout_at: None,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// 添加在线时长
    pub fn add_online_time(&mut self, seconds: i64) {
        self.total_online_seconds += seconds;
        self.updated_at = Utc::now();
    }
    
    /// 增加会话数
    pub fn increment_session_count(&mut self) {
        self.session_count += 1;
        self.updated_at = Utc::now();
    }
    
    /// 设置首次登录时间
    pub fn set_first_login(&mut self, login_time: DateTime<Utc>) {
        if self.first_login_at.is_none() || login_time < self.first_login_at.unwrap() {
            self.first_login_at = Some(login_time);
        }
        self.updated_at = Utc::now();
    }
    
    /// 设置最后登出时间
    pub fn set_last_logout(&mut self, logout_time: DateTime<Utc>) {
        self.last_logout_at = Some(logout_time);
        self.updated_at = Utc::now();
    }
    
    /// 获取在线小时数
    pub fn online_hours(&self) -> f64 {
        self.total_online_seconds as f64 / 3600.0
    }
}
```

### 用户会话实体 (UserSession)

```rust
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub disconnect_reason: Option<String>,
    pub server_instance: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl UserSession {
    pub fn new(user_id: Uuid, session_id: String, server_instance: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            session_id,
            start_time: now,
            end_time: None,
            duration_seconds: None,
            last_heartbeat: Some(now),
            disconnect_reason: None,
            server_instance,
            created_at: now,
        }
    }
    
    /// 更新心跳时间
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Some(Utc::now());
    }
    
    /// 结束会话
    pub fn end_session(&mut self, reason: String) {
        let now = Utc::now();
        self.end_time = Some(now);
        self.disconnect_reason = Some(reason);
        self.duration_seconds = Some(now.signed_duration_since(self.start_time).num_seconds());
    }
    
    /// 检查是否为活跃会话
    pub fn is_active(&self) -> bool {
        self.end_time.is_none()
    }
    
    /// 获取会话时长（秒）
    pub fn duration(&self) -> Option<i64> {
        self.duration_seconds.or_else(|| {
            self.end_time.map(|end| {
                end.signed_duration_since(self.start_time).num_seconds()
            })
        })
    }
}
```

## 🔧 领域服务接口

### 聊天室服务接口 (ChatRoomService)

```rust
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ChatRoomService: Send + Sync {
    /// 创建聊天室
    async fn create_room(&self, request: CreateRoomRequest) -> Result<ChatRoom>;
    
    /// 加入聊天室
    async fn join_room(&self, user_id: Uuid, room_id: Uuid, password: Option<String>) -> Result<()>;
    
    /// 离开聊天室
    async fn leave_room(&self, user_id: Uuid, room_id: Uuid) -> Result<()>;
    
    /// 发送消息
    async fn send_message(&self, command: SendMessageCommand) -> Result<Message>;
    
    /// 验证管理员权限
    async fn verify_admin_permission(&self, user_id: Uuid, room_id: Uuid) -> Result<bool>;
    
    /// 发送机器人消息
    async fn send_bot_message(&self, admin_id: Uuid, room_id: Uuid, content: String) -> Result<Message>;
    
    /// 获取用户房间列表
    async fn get_user_rooms(&self, user_id: Uuid) -> Result<Vec<ChatRoom>>;
    
    /// 获取房间历史消息
    async fn get_room_history(&self, query: GetRoomHistoryQuery) -> Result<Vec<Message>>;
}

#[derive(Debug, Clone)]
pub struct CreateRoomRequest {
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_private: bool,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SendMessageCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub is_bot_message: bool,
    pub reply_to_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct GetRoomHistoryQuery {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub page: u32,
    pub page_size: u32,
    pub before_message_id: Option<Uuid>,
}
```

### 组织管理服务接口 (OrganizationService)

```rust
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait OrganizationService: Send + Sync {
    /// 创建组织
    async fn create_organization(&self, request: CreateOrganizationRequest) -> Result<Organization>;
    
    /// 禁止组织
    async fn ban_organization(&self, org_id: Uuid, admin_id: Uuid) -> Result<()>;
    
    /// 解除禁止
    async fn unban_organization(&self, org_id: Uuid, admin_id: Uuid) -> Result<()>;
    
    /// 获取组织层级
    async fn get_organization_hierarchy(&self, org_id: Uuid) -> Result<Vec<Organization>>;
    
    /// 检查用户组织禁止状态
    async fn check_user_organization_ban(&self, user_id: Uuid) -> Result<bool>;
    
    /// 获取被禁止的组织列表
    async fn get_banned_organizations(&self, org_id: Uuid) -> Result<Vec<Uuid>>;
    
    /// 用户加入组织
    async fn join_organization(&self, user_id: Uuid, org_id: Uuid) -> Result<()>;
    
    /// 用户离开组织
    async fn leave_organization(&self, user_id: Uuid, org_id: Uuid) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub created_by: Uuid,
}
```

### 用户管理服务接口 (UserManagementService)

```rust
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait UserManagementService: Send + Sync {
    /// 创建用户角色
    async fn create_user_role(&self, request: CreateUserRoleRequest) -> Result<UserRole>;
    
    /// 分配用户角色
    async fn assign_user_role(&self, user_id: Uuid, role_id: Uuid, assigned_by: Uuid) -> Result<()>;
    
    /// 移除用户角色
    async fn remove_user_role(&self, user_id: Uuid, role_id: Uuid) -> Result<()>;
    
    /// 创建部门
    async fn create_department(&self, request: CreateDepartmentRequest) -> Result<Department>;
    
    /// 创建职位
    async fn create_position(&self, request: CreatePositionRequest) -> Result<Position>;
    
    /// 分配用户职位
    async fn assign_user_position(&self, user_id: Uuid, position_id: Uuid, department_id: Uuid) -> Result<()>;
    
    /// 检查用户权限
    async fn check_user_permission(&self, user_id: Uuid, permission: Permission) -> Result<bool>;
    
    /// 获取用户角色列表
    async fn get_user_roles(&self, user_id: Uuid) -> Result<Vec<UserRole>>;
    
    /// 获取用户权限列表
    async fn get_user_permissions(&self, user_id: Uuid) -> Result<Vec<Permission>>;
}

#[derive(Debug, Clone)]
pub struct CreateUserRoleRequest {
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<Permission>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateDepartmentRequest {
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub created_by: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreatePositionRequest {
    pub title: String,
    pub description: Option<String>,
    pub level: i32,
    pub department_id: Uuid,
    pub created_by: Uuid,
}
```

### 代理服务接口 (ProxyService)

```rust
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ProxyService: Send + Sync {
    /// 创建代理关系
    async fn create_proxy(&self, request: CreateProxyRequest) -> Result<UserProxy>;
    
    /// 激活代理
    async fn activate_proxy(&self, proxy_id: Uuid) -> Result<()>;
    
    /// 停用代理
    async fn deactivate_proxy(&self, proxy_id: Uuid) -> Result<()>;
    
    /// 检查代理权限
    async fn check_proxy_permission(&self, proxy_id: Uuid, principal_id: Uuid, permission: Permission) -> Result<bool>;
    
    /// 获取活跃的代理关系
    async fn get_active_proxies(&self, principal_id: Uuid) -> Result<Vec<UserProxy>>;
    
    /// 执行代理操作
    async fn execute_as_proxy(&self, proxy_id: Uuid, principal_id: Uuid, action: ProxyAction) -> Result<()>;
    
    /// 获取用户的代理关系
    async fn get_user_proxies(&self, user_id: Uuid) -> Result<Vec<UserProxy>>;
}

#[derive(Debug, Clone)]
pub struct CreateProxyRequest {
    pub principal_id: Uuid,
    pub proxy_id: Uuid,
    pub proxy_type: ProxyType,
    pub permissions: Vec<Permission>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub enum ProxyAction {
    JoinRoom { 
        room_id: Uuid, 
        password: Option<String> 
    },
    SendMessage { 
        room_id: Uuid, 
        content: String 
    },
    LeaveRoom { 
        room_id: Uuid 
    },
    CreateRoom { 
        name: String, 
        description: Option<String>, 
        is_private: bool,
        password: Option<String>
    },
}
```

### 在线时长统计服务接口 (OnlineTimeService)

```rust
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait OnlineTimeService: Send + Sync {
    /// 开始用户会话
    async fn start_session(&self, user_id: Uuid, session_id: String, server_instance: String) -> Result<()>;
    
    /// 更新心跳
    async fn update_heartbeat(&self, session_id: String) -> Result<()>;
    
    /// 结束会话
    async fn end_session(&self, session_id: String, disconnect_reason: String) -> Result<()>;
    
    /// 获取用户统计信息
    async fn get_user_statistics(&self, user_id: Uuid, date_range: DateRange) -> Result<Vec<UserOnlineStatistics>>;
    
    /// 获取活跃会话
    async fn get_active_sessions(&self, user_id: Uuid) -> Result<Vec<UserSession>>;
    
    /// 获取用户总在线时长
    async fn get_total_online_time(&self, user_id: Uuid, date_range: DateRange) -> Result<i64>;
}

#[derive(Debug, Clone)]
pub struct DateRange {
    pub start_date: chrono::NaiveDate,
    pub end_date: chrono::NaiveDate,
}
```

## 🎯 领域事件

### 聊天事件 (ChatEvent)

```rust
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatEvent {
    /// 消息发送事件
    MessageSent {
        message: Message,
        room_id: Uuid,
    },
    
    /// 用户加入房间事件
    UserJoined {
        user_id: Uuid,
        room_id: Uuid,
        username: String,
    },
    
    /// 用户离开房间事件
    UserLeft {
        user_id: Uuid,
        room_id: Uuid,
    },
    
    /// 房间创建事件
    RoomCreated {
        room: ChatRoom,
    },
    
    /// 组织禁止事件
    OrganizationBanned {
        organization_id: Uuid,
        banned_by: Uuid,
        affected_users: Vec<Uuid>,
    },
    
    /// 用户在线时长更新事件
    UserOnlineTimeUpdated {
        user_id: Uuid,
        session_id: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        duration_seconds: u64,
        disconnect_reason: String,
    },
    
    /// 用户心跳事件
    UserActivityHeartbeat {
        user_id: Uuid,
        session_id: String,
        timestamp: DateTime<Utc>,
        room_ids: Vec<Uuid>,
        server_instance_id: String,
    },
}
```

### 业务规则验证

#### 房间加入规则

```rust
impl ChatRoom {
    /// 验证用户是否可以加入房间
    pub fn can_user_join(&self, user_status: UserStatus, user_org_banned: bool) -> Result<()> {
        // 检查用户状态
        if !matches!(user_status, UserStatus::Active) {
            return Err(anyhow::anyhow!("用户状态不活跃，无法加入房间"));
        }
        
        // 检查用户是否被组织禁止
        if user_org_banned {
            return Err(anyhow::anyhow!("用户所属组织被禁止，无法加入房间"));
        }
        
        // 私密房间需要密码（在调用时验证）
        if self.is_private {
            // 密码验证在外部进行
        }
        
        Ok(())
    }
}
```

#### 消息发送规则

```rust
impl Message {
    /// 验证消息内容
    pub fn validate_content(&self) -> Result<()> {
        if self.content.trim().is_empty() {
            return Err(anyhow::anyhow!("消息内容不能为空"));
        }
        
        if self.content.len() > 10000 {
            return Err(anyhow::anyhow!("消息内容过长，最大限制10000字符"));
        }
        
        // 检查敏感词（简化版）
        if self.content.contains("敏感词") {
            return Err(anyhow::anyhow!("消息包含敏感内容"));
        }
        
        Ok(())
    }
}
```

#### 代理权限规则

```rust
impl UserProxy {
    /// 验证代理权限
    pub fn validate_proxy_action(&self, action: &ProxyAction) -> Result<()> {
        if !self.is_active_proxy() {
            return Err(anyhow::anyhow!("代理关系不活跃"));
        }
        
        let required_permission = match action {
            ProxyAction::JoinRoom { .. } => Permission::JoinPrivateRoom,
            ProxyAction::SendMessage { .. } => Permission::SendMessage,
            ProxyAction::LeaveRoom { .. } => Permission::SendMessage, // 离开房间需要消息权限
            ProxyAction::CreateRoom { .. } => Permission::CreateRoom,
        };
        
        if !self.has_permission(&required_permission) {
            return Err(anyhow::anyhow!("代理权限不足"));
        }
        
        Ok(())
    }
}
```

---

**领域层设计总结**:

- **实体设计**: 包含用户、聊天室、消息、组织、角色等核心业务实体
- **领域服务**: 定义业务操作接口，封装复杂业务逻辑
- **业务规则**: 在实体中内聚业务规则，确保数据一致性
- **领域事件**: 支持事件驱动架构，实现松耦合
- **扩展性**: 支持企业级功能，如组织管理、权限控制、代理系统等

**下一步**: 阅读[03-application-layer-design.md](./03-application-layer-design.md)了解应用层设计。
