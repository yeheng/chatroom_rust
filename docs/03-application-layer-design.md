# 应用层设计

应用层负责协调领域对象和基础设施，实现系统的用例。本层采用CQRS（命令查询职责分离）模式，将写操作（命令）和读操作（查询）分离。

## 🏗️ 应用层架构

### 核心组件

```rust
// 应用服务
pub struct ChatRoomApplicationService {
    pub command_bus: Arc<dyn CommandBus>,
    pub query_bus: Arc<dyn QueryBus>,
    pub event_bus: Arc<dyn EventBus>,
}

// 命令总线接口
#[async_trait]
pub trait CommandBus: Send + Sync {
    async fn dispatch(&self, command: Command) -> Result<()>;
}

// 查询总线接口
#[async_trait]
pub trait QueryBus: Send + Sync {
    async fn dispatch(&self, query: Query) -> Result<QueryResult>;
}

// 事件总线接口
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<()>;
    async fn subscribe(&self, handler: Arc<dyn EventHandler>) -> Result<()>;
}
```

### CQRS模式实现

```rust
// 命令特征
pub trait Command: Send + Sync {
    type Result;
}

// 查询特征
pub trait Query: Send + Sync {
    type Result;
}

// 命令处理器接口
#[async_trait]
pub trait CommandHandler<C: Command>: Send + Sync {
    async fn handle(&self, command: C) -> Result<C::Result>;
}

// 查询处理器接口
#[async_trait]
pub trait QueryHandler<Q: Query>: Send + Sync {
    async fn handle(&self, query: Q) -> Result<Q::Result>;
}

// 事件处理器接口
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: DomainEvent) -> Result<()>;
    fn can_handle(&self, event_type: &str) -> bool;
}
```

## 📦 命令处理器

### 用户管理命令

```rust
// 用户注册命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterUserCommand {
    pub username: String,
    pub email: String,
    pub password: String,
    pub avatar_url: Option<String>,
}

// 用户登录命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginUserCommand {
    pub email: String,
    pub password: String,
}

// 更新用户信息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserCommand {
    pub user_id: Uuid,
    pub username: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[async_trait]
impl Command for RegisterUserCommand {
    type Result = User;
}

#[async_trait]
impl CommandHandler<RegisterUserCommand> for UserCommandHandler {
    async fn handle(&self, command: RegisterUserCommand) -> Result<User> {
        // 验证用户名和邮箱唯一性
        self.validate_user_uniqueness(&command.username, &command.email).await?;
        
        // 创建用户聚合根
        let user = User::new(
            Uuid::new_v4(),
            command.username,
            command.email,
            command.avatar_url,
        )?;
        
        // 加密密码
        let password_hash = self.hash_password(&command.password).await?;
        user.set_password_hash(password_hash);
        
        // 保存用户
        let saved_user = self.user_repository.save(&user).await?;
        
        // 发布用户注册事件
        self.event_bus.publish(DomainEvent::UserRegistered {
            user_id: saved_user.id,
            username: saved_user.username.clone(),
            email: saved_user.email.clone(),
        }).await?;
        
        Ok(saved_user)
    }
}
```

### 聊天室管理命令

```rust
// 创建聊天室命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChatRoomCommand {
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_private: bool,
    pub password: Option<String>,
}

// 加入聊天室命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinChatRoomCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub password: Option<String>,
}

// 离开聊天室命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveChatRoomCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
}

// 发送消息命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageCommand {
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to_message_id: Option<Uuid>,
}

#[async_trait]
impl Command for SendMessageCommand {
    type Result = Message;
}

#[async_trait]
impl CommandHandler<SendMessageCommand> for ChatRoomCommandHandler {
    async fn handle(&self, command: SendMessageCommand) -> Result<Message> {
        // 验证用户是否在房间中
        let is_member = self.room_repository.is_user_in_room(command.room_id, command.user_id).await?;
        if !is_member {
            return Err(DomainError::UserNotInRoom);
        }
        
        // 创建消息实体
        let message = Message::new(
            Uuid::new_v4(),
            command.room_id,
            command.user_id,
            command.content,
            command.message_type,
            command.reply_to_message_id,
        )?;
        
        // 保存消息
        let saved_message = self.message_repository.save(&message).await?;
        
        // 发布消息发送事件
        self.event_bus.publish(DomainEvent::MessageSent {
            message: saved_message.clone(),
            room_id: command.room_id,
        }).await?;
        
        Ok(saved_message)
    }
}
```

### 组织管理命令

```rust
// 创建组织命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrganizationCommand {
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub settings: Option<OrganizationSettings>,
}

// 添加用户到组织命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddUserToOrganizationCommand {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub department_id: Option<Uuid>,
    pub position_id: Option<Uuid>,
}

#[async_trait]
impl CommandHandler<CreateOrganizationCommand> for OrganizationCommandHandler {
    async fn handle(&self, command: CreateOrganizationCommand) -> Result<Organization> {
        // 验证用户权限
        let user = self.user_repository.find_by_id(command.owner_id).await?
            .ok_or(DomainError::UserNotFound)?;
        
        // 创建组织
        let organization = Organization::new(
            Uuid::new_v4(),
            command.name,
            command.description,
            command.owner_id,
            command.settings.unwrap_or_default(),
        )?;
        
        // 保存组织
        let saved_org = self.organization_repository.save(&organization).await?;
        
        // 创建所有者角色关系
        let owner_role = self.role_repository.find_by_name("owner").await?
            .ok_or(DomainError::RoleNotFound)?;
        
        let user_role = UserRole::new(
            Uuid::new_v4(),
            command.owner_id,
            saved_org.id,
            owner_role.id,
        )?;
        
        self.user_role_repository.save(&user_role).await?;
        
        // 发布组织创建事件
        self.event_bus.publish(DomainEvent::OrganizationCreated {
            organization: saved_org.clone(),
            created_by: command.owner_id,
        }).await?;
        
        Ok(saved_org)
    }
}
```

## 🔍 查询处理器

### 用户查询

```rust
// 根据ID查询用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserByIdQuery {
    pub user_id: Uuid,
}

// 根据邮箱查询用户
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserByEmailQuery {
    pub email: String,
}

// 搜索用户查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchUsersQuery {
    pub keyword: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[async_trait]
impl Query for GetUserByIdQuery {
    type Result = Option<UserDto>;
}

#[async_trait]
impl QueryHandler<GetUserByIdQuery> for UserQueryHandler {
    async fn handle(&self, query: GetUserByIdQuery) -> Result<Option<UserDto>> {
        let user = self.user_repository.find_by_id(query.user_id).await?;
        
        match user {
            Some(user) => {
                let dto = UserDto {
                    id: user.id,
                    username: user.username,
                    email: user.email,
                    avatar_url: user.avatar_url,
                    status: user.status,
                    created_at: user.created_at,
                    updated_at: user.updated_at,
                };
                Ok(Some(dto))
            }
            None => Ok(None),
        }
    }
}
```

### 聊天室查询

```rust
// 获取用户加入的聊天室列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserRoomsQuery {
    pub user_id: Uuid,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// 获取聊天室消息历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRoomMessagesQuery {
    pub room_id: Uuid,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub before: Option<DateTime<Utc>>,
}

// 搜索消息查询
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMessagesQuery {
    pub room_id: Option<Uuid>,
    pub keyword: String,
    pub message_type: Option<MessageType>,
    pub user_id: Option<Uuid>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[async_trait]
impl QueryHandler<GetUserRoomsQuery> for ChatRoomQueryHandler {
    async fn handle(&self, query: GetUserRoomsQuery) -> Result<Vec<ChatRoomDto>> {
        let rooms = self.room_repository.find_by_user_id(
            query.user_id,
            query.limit.unwrap_or(50),
            query.offset.unwrap_or(0),
        ).await?;
        
        let dtos = rooms.into_iter().map(|room| ChatRoomDto {
            id: room.id,
            name: room.name,
            description: room.description,
            owner_id: room.owner_id,
            is_private: room.is_private,
            member_count: self.room_repository.get_member_count(room.id).await.unwrap_or(0),
            created_at: room.created_at,
            updated_at: room.updated_at,
        }).collect();
        
        Ok(dtos)
    }
}
```

### 组织查询

```rust
// 获取用户的组织列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserOrganizationsQuery {
    pub user_id: Uuid,
    pub include_details: bool,
}

// 获取组织成员列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetOrganizationMembersQuery {
    pub organization_id: Uuid,
    pub department_id: Option<Uuid>,
    pub role_id: Option<Uuid>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[async_trait]
impl QueryHandler<GetUserOrganizationsQuery> for OrganizationQueryHandler {
    async fn handle(&self, query: GetUserOrganizationsQuery) -> Result<Vec<OrganizationDto>> {
        let organizations = self.organization_repository.find_by_user_id(query.user_id).await?;
        
        let mut dtos = Vec::new();
        
        for org in organizations {
            let dto = OrganizationDto {
                id: org.id,
                name: org.name,
                description: org.description,
                owner_id: org.owner_id,
                settings: org.settings.clone(),
                member_count: self.organization_repository.get_member_count(org.id).await.unwrap_or(0),
                created_at: org.created_at,
                updated_at: org.updated_at,
            };
            
            dtos.push(dto);
        }
        
        Ok(dtos)
    }
}
```

## 🎯 DTO（数据传输对象）

### 用户相关DTO

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDto {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub avatar_url: Option<String>,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileDto {
    pub user: UserDto,
    pub organizations: Vec<OrganizationDto>,
    pub rooms: Vec<ChatRoomDto>,
    pub statistics: UserStatisticsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStatisticsDto {
    pub total_rooms_created: u32,
    pub total_messages_sent: u32,
    pub total_organizations_joined: u32,
    pub last_active_at: DateTime<Utc>,
}
```

### 聊天室相关DTO

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoomDto {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub is_private: bool,
    pub member_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRoomDetailDto {
    pub room: ChatRoomDto,
    pub members: Vec<RoomMemberDto>,
    pub recent_messages: Vec<MessageDto>,
    pub settings: RoomSettingsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMemberDto {
    pub user_id: Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
    pub last_active_at: Option<DateTime<Utc>>,
}
```

### 消息相关DTO

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDto {
    pub id: Uuid,
    pub room_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub avatar_url: Option<String>,
    pub content: String,
    pub message_type: MessageType,
    pub reply_to_message_id: Option<Uuid>,
    pub reply_to_username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageThreadDto {
    pub root_message: MessageDto,
    pub replies: Vec<MessageDto>,
    pub total_replies: u32,
}
```

## 🎨 应用服务

### 认证服务

```rust
pub struct AuthenticationService {
    pub user_repository: Arc<dyn UserRepository>,
    pub jwt_service: Arc<dyn JwtService>,
    pub password_service: Arc<dyn PasswordService>,
    pub cache_service: Arc<dyn CacheService>,
}

impl AuthenticationService {
    pub async fn register(&self, request: RegisterUserRequest) -> Result<AuthResponse> {
        // 执行注册命令
        let command = RegisterUserCommand {
            username: request.username,
            email: request.email,
            password: request.password,
            avatar_url: request.avatar_url,
        };
        
        let user = self.command_bus.dispatch(command).await?;
        
        // 生成JWT令牌
        let access_token = self.jwt_service.generate_access_token(user.id)?;
        let refresh_token = self.jwt_service.generate_refresh_token(user.id)?;
        
        // 缓存用户信息
        self.cache_user_session(user.id, &access_token).await?;
        
        Ok(AuthResponse {
            user: UserDto::from(user),
            access_token,
            refresh_token,
            expires_in: 3600, // 1小时
        })
    }
    
    pub async fn login(&self, request: LoginUserRequest) -> Result<AuthResponse> {
        // 执行登录命令
        let command = LoginUserCommand {
            email: request.email,
            password: request.password,
        };
        
        let user = self.command_bus.dispatch(command).await?;
        
        // 生成JWT令牌
        let access_token = self.jwt_service.generate_access_token(user.id)?;
        let refresh_token = self.jwt_service.generate_refresh_token(user.id)?;
        
        // 更新用户状态
        user.update_status(UserStatus::Online);
        self.user_repository.save(&user).await?;
        
        // 缓存用户会话
        self.cache_user_session(user.id, &access_token).await?;
        
        Ok(AuthResponse {
            user: UserDto::from(user),
            access_token,
            refresh_token,
            expires_in: 3600,
        })
    }
    
    pub async fn logout(&self, user_id: Uuid) -> Result<()> {
        // 清除缓存
        self.cache_service.remove(&format!("user_session:{}", user_id)).await?;
        
        // 更新用户状态
        if let Some(mut user) = self.user_repository.find_by_id(user_id).await? {
            user.update_status(UserStatus::Offline);
            self.user_repository.save(&user).await?;
        }
        
        Ok(())
    }
    
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<AuthResponse> {
        // 验证刷新令牌
        let token_data = self.jwt_service.verify_refresh_token(refresh_token)?;
        let user_id = token_data.claims.sub;
        
        // 检查用户是否存在
        let user = self.user_repository.find_by_id(user_id).await?
            .ok_or(DomainError::UserNotFound)?;
        
        // 生成新的访问令牌
        let new_access_token = self.jwt_service.generate_access_token(user.id)?;
        let new_refresh_token = self.jwt_service.generate_refresh_token(user.id)?;
        
        // 缓存用户会话
        self.cache_user_session(user.id, &new_access_token).await?;
        
        Ok(AuthResponse {
            user: UserDto::from(user),
            access_token: new_access_token,
            refresh_token: new_refresh_token,
            expires_in: 3600,
        })
    }
    
    async fn cache_user_session(&self, user_id: Uuid, access_token: &str) -> Result<()> {
        let key = format!("user_session:{}", user_id);
        let value = serde_json::to_string(&UserSession {
            user_id,
            access_token: access_token.to_string(),
            created_at: Utc::now(),
        })?;
        
        self.cache_service.setex(&key, &value, 3600).await?;
        Ok(())
    }
}
```

### 聊天室服务

```rust
pub struct ChatRoomService {
    pub command_bus: Arc<dyn CommandBus>,
    pub query_bus: Arc<dyn QueryBus>,
    pub room_repository: Arc<dyn ChatRoomRepository>,
    pub user_repository: Arc<dyn UserRepository>,
    pub message_repository: Arc<dyn MessageRepository>,
    pub event_bus: Arc<dyn EventBus>,
}

impl ChatRoomService {
    pub async fn create_room(&self, request: CreateRoomRequest, user_id: Uuid) -> Result<ChatRoomDto> {
        let command = CreateChatRoomCommand {
            name: request.name,
            description: request.description,
            owner_id: user_id,
            is_private: request.is_private,
            password: request.password,
        };
        
        let room = self.command_bus.dispatch(command).await?;
        Ok(ChatRoomDto::from(room))
    }
    
    pub async fn join_room(&self, room_id: Uuid, user_id: Uuid, password: Option<String>) -> Result<()> {
        let command = JoinChatRoomCommand {
            room_id,
            user_id,
            password,
        };
        
        self.command_bus.dispatch(command).await?;
        Ok(())
    }
    
    pub async fn leave_room(&self, room_id: Uuid, user_id: Uuid) -> Result<()> {
        let command = LeaveChatRoomCommand {
            room_id,
            user_id,
        };
        
        self.command_bus.dispatch(command).await?;
        Ok(())
    }
    
    pub async fn send_message(&self, request: SendMessageRequest, user_id: Uuid) -> Result<MessageDto> {
        let command = SendMessageCommand {
            room_id: request.room_id,
            user_id,
            content: request.content,
            message_type: request.message_type,
            reply_to_message_id: request.reply_to_message_id,
        };
        
        let message = self.command_bus.dispatch(command).await?;
        Ok(MessageDto::from(message))
    }
    
    pub async fn get_user_rooms(&self, user_id: Uuid, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<ChatRoomDto>> {
        let query = GetUserRoomsQuery {
            user_id,
            limit,
            offset,
        };
        
        let rooms = self.query_bus.dispatch(query).await?;
        Ok(rooms)
    }
    
    pub async fn get_room_messages(&self, room_id: Uuid, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<MessageDto>> {
        let query = GetRoomMessagesQuery {
            room_id,
            limit,
            offset,
            before: None,
        };
        
        let messages = self.query_bus.dispatch(query).await?;
        Ok(messages)
    }
    
    pub async fn search_messages(&self, request: SearchMessagesRequest) -> Result<Vec<MessageDto>> {
        let query = SearchMessagesQuery {
            room_id: request.room_id,
            keyword: request.keyword,
            message_type: request.message_type,
            user_id: request.user_id,
            start_date: request.start_date,
            end_date: request.end_date,
            limit: request.limit,
            offset: request.offset,
        };
        
        let messages = self.query_bus.dispatch(query).await?;
        Ok(messages)
    }
}
```

### 组织服务

```rust
pub struct OrganizationService {
    pub command_bus: Arc<dyn CommandBus>,
    pub query_bus: Arc<dyn QueryBus>,
    pub organization_repository: Arc<dyn OrganizationRepository>,
    pub user_repository: Arc<dyn UserRepository>,
    pub role_repository: Arc<dyn RoleRepository>,
    pub department_repository: Arc<dyn DepartmentRepository>,
    pub feature_flags: FeatureFlags,
}

impl OrganizationService {
    pub async fn create_organization(&self, request: CreateOrganizationRequest, user_id: Uuid) -> Result<OrganizationDto> {
        if !self.feature_flags.enable_organizations {
            return Err(DomainError::FeatureNotEnabled("organizations".to_string()));
        }
        
        let command = CreateOrganizationCommand {
            name: request.name,
            description: request.description,
            owner_id: user_id,
            settings: request.settings,
        };
        
        let organization = self.command_bus.dispatch(command).await?;
        Ok(OrganizationDto::from(organization))
    }
    
    pub async fn add_user_to_organization(&self, request: AddUserToOrganizationRequest) -> Result<()> {
        if !self.feature_flags.enable_organizations {
            return Err(DomainError::FeatureNotEnabled("organizations".to_string()));
        }
        
        let command = AddUserToOrganizationCommand {
            organization_id: request.organization_id,
            user_id: request.user_id,
            role_id: request.role_id,
            department_id: request.department_id,
            position_id: request.position_id,
        };
        
        self.command_bus.dispatch(command).await?;
        Ok(())
    }
    
    pub async fn get_user_organizations(&self, user_id: Uuid) -> Result<Vec<OrganizationDto>> {
        let query = GetUserOrganizationsQuery {
            user_id,
            include_details: true,
        };
        
        let organizations = self.query_bus.dispatch(query).await?;
        Ok(organizations)
    }
    
    pub async fn get_organization_members(&self, organization_id: Uuid, filters: OrganizationMemberFilters) -> Result<Vec<OrganizationMemberDto>> {
        let query = GetOrganizationMembersQuery {
            organization_id,
            department_id: filters.department_id,
            role_id: filters.role_id,
            limit: filters.limit,
            offset: filters.offset,
        };
        
        let members = self.query_bus.dispatch(query).await?;
        Ok(members)
    }
}
```

## 🔄 事件处理器

```rust
pub struct MessageEventHandler {
    pub websocket_manager: Arc<dyn WebSocketManager>,
    pub notification_service: Arc<dyn NotificationService>,
    pub search_service: Arc<dyn SearchService>,
}

#[async_trait]
impl EventHandler for MessageEventHandler {
    fn can_handle(&self, event_type: &str) -> bool {
        matches!(event_type, "message_sent" | "message_updated" | "message_deleted")
    }
    
    async fn handle(&self, event: DomainEvent) -> Result<()> {
        match event {
            DomainEvent::MessageSent { message, room_id } => {
                // 实时推送到WebSocket
                self.websocket_manager.broadcast_to_room(room_id, message.clone()).await?;
                
                // 发送通知
                self.notification_service.notify_room_members(room_id, &message).await?;
                
                // 索引到搜索引擎
                self.search_service.index_message(&message).await?;
                
                Ok(())
            }
            DomainEvent::MessageUpdated { message, room_id } => {
                // 更新消息并推送到WebSocket
                self.websocket_manager.broadcast_to_room(room_id, message.clone()).await?;
                
                // 更新搜索索引
                self.search_service.update_message_index(&message).await?;
                
                Ok(())
            }
            DomainEvent::MessageDeleted { message_id, room_id } => {
                // 通知房间成员消息已删除
                self.websocket_manager.notify_message_deleted(room_id, message_id).await?;
                
                // 从搜索索引中删除
                self.search_service.remove_message_from_index(message_id).await?;
                
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

pub struct UserEventHandler {
    pub presence_service: Arc<dyn PresenceService>,
    pub notification_service: Arc<dyn NotificationService>,
    pub analytics_service: Arc<dyn AnalyticsService>,
}

#[async_trait]
impl EventHandler for UserEventHandler {
    fn can_handle(&self, event_type: &str) -> bool {
        matches!(event_type, "user_registered" | "user_logged_in" | "user_logged_out" | "user_status_changed")
    }
    
    async fn handle(&self, event: DomainEvent) -> Result<()> {
        match event {
            DomainEvent::UserRegistered { user_id, username, email } => {
                // 欢迎通知
                self.notification_service.send_welcome_email(user_id, &username, &email).await?;
                
                // 初始化用户分析数据
                self.analytics_service.initialize_user(user_id).await?;
                
                Ok(())
            }
            DomainEvent::UserLoggedIn { user_id } => {
                // 更新在线状态
                self.presence_service.update_user_status(user_id, UserStatus::Online).await?;
                
                Ok(())
            }
            DomainEvent::UserLoggedOut { user_id } => {
                // 更新离线状态
                self.presence_service.update_user_status(user_id, UserStatus::Offline).await?;
                
                Ok(())
            }
            DomainEvent::UserStatusChanged { user_id, status } => {
                // 更新在线状态
                self.presence_service.update_user_status(user_id, status).await?;
                
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
```

## 📊 应用层依赖注入

```rust
pub struct ApplicationContainer {
    pub command_bus: Arc<dyn CommandBus>,
    pub query_bus: Arc<dyn QueryBus>,
    pub event_bus: Arc<dyn EventBus>,
    pub auth_service: Arc<AuthenticationService>,
    pub chat_room_service: Arc<ChatRoomService>,
    pub organization_service: Arc<OrganizationService>,
}

impl ApplicationContainer {
    pub fn new(
        user_repository: Arc<dyn UserRepository>,
        room_repository: Arc<dyn ChatRoomRepository>,
        message_repository: Arc<dyn MessageRepository>,
        organization_repository: Arc<dyn OrganizationRepository>,
        role_repository: Arc<dyn RoleRepository>,
        department_repository: Arc<dyn DepartmentRepository>,
        user_role_repository: Arc<dyn UserRoleRepository>,
        position_repository: Arc<dyn PositionRepository>,
        user_proxy_repository: Arc<dyn UserProxyRepository>,
        online_time_repository: Arc<dyn OnlineTimeRepository>,
        websocket_manager: Arc<dyn WebSocketManager>,
        jwt_service: Arc<dyn JwtService>,
        password_service: Arc<dyn PasswordService>,
        cache_service: Arc<dyn CacheService>,
        notification_service: Arc<dyn NotificationService>,
        search_service: Arc<dyn SearchService>,
        presence_service: Arc<dyn PresenceService>,
        analytics_service: Arc<dyn AnalyticsService>,
        feature_flags: FeatureFlags,
    ) -> Self {
        // 创建事件总线
        let event_bus = Arc::new(InMemoryEventBus::new());
        
        // 创建命令总线
        let command_bus = Arc::new(InMemoryCommandBus::new());
        
        // 创建查询总线
        let query_bus = Arc::new(InMemoryQueryBus::new());
        
        // 注册命令处理器
        let user_command_handler = Arc::new(UserCommandHandler::new(
            user_repository.clone(),
            password_service.clone(),
            event_bus.clone(),
        ));
        
        let room_command_handler = Arc::new(ChatRoomCommandHandler::new(
            room_repository.clone(),
            message_repository.clone(),
            user_repository.clone(),
            event_bus.clone(),
        ));
        
        let organization_command_handler = Arc::new(OrganizationCommandHandler::new(
            organization_repository.clone(),
            user_repository.clone(),
            role_repository.clone(),
            user_role_repository.clone(),
            event_bus.clone(),
        ));
        
        command_bus.register_handler::<RegisterUserCommand>(user_command_handler.clone());
        command_bus.register_handler::<LoginUserCommand>(user_command_handler.clone());
        command_bus.register_handler::<UpdateUserCommand>(user_command_handler.clone());
        
        command_bus.register_handler::<CreateChatRoomCommand>(room_command_handler.clone());
        command_bus.register_handler::<JoinChatRoomCommand>(room_command_handler.clone());
        command_bus.register_handler::<LeaveChatRoomCommand>(room_command_handler.clone());
        command_bus.register_handler::<SendMessageCommand>(room_command_handler.clone());
        
        command_bus.register_handler::<CreateOrganizationCommand>(organization_command_handler.clone());
        command_bus.register_handler::<AddUserToOrganizationCommand>(organization_command_handler.clone());
        
        // 注册查询处理器
        let user_query_handler = Arc::new(UserQueryHandler::new(user_repository.clone()));
        let room_query_handler = Arc::new(ChatRoomQueryHandler::new(
            room_repository.clone(),
            message_repository.clone(),
        ));
        let organization_query_handler = Arc::new(OrganizationQueryHandler::new(
            organization_repository.clone(),
            user_repository.clone(),
            role_repository.clone(),
        ));
        
        query_bus.register_handler::<GetUserByIdQuery>(user_query_handler.clone());
        query_bus.register_handler::<GetUserByEmailQuery>(user_query_handler.clone());
        query_bus.register_handler::<SearchUsersQuery>(user_query_handler.clone());
        
        query_bus.register_handler::<GetUserRoomsQuery>(room_query_handler.clone());
        query_bus.register_handler::<GetRoomMessagesQuery>(room_query_handler.clone());
        query_bus.register_handler::<SearchMessagesQuery>(room_query_handler.clone());
        
        query_bus.register_handler::<GetUserOrganizationsQuery>(organization_query_handler.clone());
        query_bus.register_handler::<GetOrganizationMembersQuery>(organization_query_handler.clone());
        
        // 注册事件处理器
        let message_event_handler = Arc::new(MessageEventHandler::new(
            websocket_manager.clone(),
            notification_service.clone(),
            search_service.clone(),
        ));
        
        let user_event_handler = Arc::new(UserEventHandler::new(
            presence_service.clone(),
            notification_service.clone(),
            analytics_service.clone(),
        ));
        
        event_bus.subscribe(message_event_handler).await?;
        event_bus.subscribe(user_event_handler).await?;
        
        // 创建应用服务
        let auth_service = Arc::new(AuthenticationService::new(
            user_repository.clone(),
            jwt_service.clone(),
            password_service.clone(),
            cache_service.clone(),
        ));
        
        let chat_room_service = Arc::new(ChatRoomService::new(
            command_bus.clone(),
            query_bus.clone(),
            room_repository.clone(),
            user_repository.clone(),
            message_repository.clone(),
            event_bus.clone(),
        ));
        
        let organization_service = Arc::new(OrganizationService::new(
            command_bus.clone(),
            query_bus.clone(),
            organization_repository.clone(),
            user_repository.clone(),
            role_repository.clone(),
            department_repository.clone(),
            feature_flags,
        ));
        
        Self {
            command_bus,
            query_bus,
            event_bus,
            auth_service,
            chat_room_service,
            organization_service,
        }
    }
}
```

---

**下一步**: 阅读[04-infrastructure-layer-design.md](./04-infrastructure-layer-design.md)了解基础设施层的详细设计。
