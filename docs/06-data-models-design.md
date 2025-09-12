# 数据模型设计

本节详细说明系统的数据模型设计，包括数据库表结构、Kafka主题设计、缓存结构等。数据模型采用关系型数据库设计，遵循数据库范式原则。

## 🗄️ 数据库表结构

### 核心表设计

#### 用户表 (users)

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    avatar_url VARCHAR(500),
    password_hash VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    last_active_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 索引
    CONSTRAINT users_status_check CHECK (status IN ('active', 'inactive', 'banned')),
    CONSTRAINT users_email_check CHECK (email ~* '^[A-Za-z0-9._%-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,4}$')
);

-- 用户状态枚举
CREATE TYPE user_status AS ENUM ('active', 'inactive', 'banned');

-- 用户扩展表（用于存储额外字段）
CREATE TABLE user_extensions (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    bio TEXT,
    location VARCHAR(100),
    website VARCHAR(255),
    timezone VARCHAR(50),
    language VARCHAR(10) DEFAULT 'en',
    preferences JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 索引
CREATE INDEX idx_users_status ON users(status);
CREATE INDEX idx_users_created_at ON users(created_at DESC);
CREATE INDEX idx_users_last_active ON users(last_active_at DESC);
CREATE INDEX idx_users_username_like ON users USING gin(username gin_trgm_ops);
CREATE INDEX idx_users_email_like ON users USING gin(email gin_trgm_ops);
```

#### 聊天室表 (chat_rooms)

```sql
CREATE TABLE chat_rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_private BOOLEAN NOT NULL DEFAULT false,
    password_hash VARCHAR(255),
    max_members INTEGER DEFAULT 1000,
    allow_invites BOOLEAN DEFAULT true,
    require_approval BOOLEAN DEFAULT false,
    settings JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT chat_rooms_name_check CHECK (name ~* '^[a-zA-Z0-9_-]{3,50}$'),
    CONSTRAINT chat_rooms_password_check CHECK (
        (is_private = true AND password_hash IS NOT NULL) OR 
        (is_private = false AND password_hash IS NULL)
    )
);

-- 索引
CREATE INDEX idx_chat_rooms_owner ON chat_rooms(owner_id);
CREATE INDEX idx_chat_rooms_is_private ON chat_rooms(is_private);
CREATE INDEX idx_chat_rooms_created_at ON chat_rooms(created_at DESC);
CREATE INDEX idx_chat_rooms_name_like ON chat_rooms USING gin(name gin_trgm_ops);
```

#### 房间成员表 (room_members)

```sql
CREATE TABLE room_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID NOT NULL REFERENCES chat_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'member',
    joined_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_read_message_id UUID,
    is_muted BOOLEAN DEFAULT false,
    notifications_enabled BOOLEAN DEFAULT true,
    permissions JSONB DEFAULT '{}',
    
    -- 约束
    CONSTRAINT room_members_role_check CHECK (role IN ('owner', 'admin', 'moderator', 'member')),
    CONSTRAINT room_members_unique UNIQUE (room_id, user_id)
);

-- 房间角色枚举
CREATE TYPE room_member_role AS ENUM ('owner', 'admin', 'moderator', 'member');

-- 索引
CREATE INDEX idx_room_members_room_id ON room_members(room_id);
CREATE INDEX idx_room_members_user_id ON room_members(user_id);
CREATE INDEX idx_room_members_role ON room_members(role);
CREATE UNIQUE INDEX idx_room_members_unique ON room_members(room_id, user_id);
```

#### 消息表 (messages)

```sql
CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    room_id UUID NOT NULL REFERENCES chat_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    message_type VARCHAR(20) NOT NULL DEFAULT 'text',
    reply_to_message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    is_edited BOOLEAN DEFAULT false,
    is_deleted BOOLEAN DEFAULT false,
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT messages_type_check CHECK (message_type IN ('text', 'image', 'file', 'system', 'bot'))
);

-- 消息类型枚举
CREATE TYPE message_type AS ENUM ('text', 'image', 'file', 'system', 'bot');

-- 索引
CREATE INDEX idx_messages_room_id ON messages(room_id);
CREATE INDEX idx_messages_user_id ON messages(user_id);
CREATE INDEX idx_messages_created_at ON messages(created_at DESC);
CREATE INDEX idx_messages_room_created ON messages(room_id, created_at DESC);
CREATE INDEX idx_messages_reply_to ON messages(reply_to_message_id);
CREATE INDEX idx_messages_type ON messages(message_type);
-- 全文搜索索引
CREATE INDEX idx_messages_content_search ON messages USING gin(to_tsvector('english', content));
```

#### 消息回复表 (message_replies)

```sql
CREATE TABLE message_replies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    reply_message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    CONSTRAINT message_replies_unique UNIQUE (message_id, reply_message_id)
);

-- 索引
CREATE INDEX idx_message_replies_message_id ON message_replies(message_id);
CREATE INDEX idx_message_replies_reply_id ON message_replies(reply_message_id);
```

### 企业扩展表

#### 组织表 (organizations)

```sql
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    settings JSONB DEFAULT '{}',
    max_members INTEGER DEFAULT 1000,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT organizations_name_check CHECK (name ~* '^[a-zA-Z0-9_-]{3,50}$')
);

-- 索引
CREATE INDEX idx_organizations_owner ON organizations(owner_id);
CREATE INDEX idx_organizations_active ON organizations(is_active);
CREATE INDEX idx_organizations_name_like ON organizations USING gin(name gin_trgm_ops);
```

#### 角色表 (roles)

```sql
CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(50) NOT NULL,
    description TEXT,
    permissions JSONB DEFAULT '{}',
    is_system_role BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT roles_name_unique_in_org UNIQUE (organization_id, name),
    CONSTRAINT roles_system_role_check CHECK (
        (organization_id IS NULL AND is_system_role = true) OR
        (organization_id IS NOT NULL AND is_system_role = false)
    )
);

-- 索引
CREATE INDEX idx_roles_organization ON roles(organization_id);
CREATE INDEX idx_roles_system ON roles(is_system_role);

-- 系统默认角色
INSERT INTO roles (id, name, description, permissions, is_system_role) VALUES
    (gen_random_uuid(), 'owner', 'Organization owner with full permissions', '{"*": ["*"]}', true),
    (gen_random_uuid(), 'admin', 'Administrator with most permissions', '{"*": ["read", "write", "delete"]}', true),
    (gen_random_uuid(), 'member', 'Regular member with basic permissions', '{"*": ["read"]}', true);
```

#### 用户角色表 (user_roles)

```sql
CREATE TABLE user_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    department_id UUID REFERENCES departments(id) ON DELETE SET NULL,
    position_id UUID REFERENCES positions(id) ON DELETE SET NULL,
    assigned_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE,
    is_active BOOLEAN DEFAULT true,
    
    -- 约束
    CONSTRAINT user_roles_unique UNIQUE (user_id, organization_id),
    CONSTRAINT user_roles_expires_check CHECK (expires_at IS NULL OR expires_at > NOW())
);

-- 索引
CREATE INDEX idx_user_roles_user ON user_roles(user_id);
CREATE INDEX idx_user_roles_organization ON user_roles(organization_id);
CREATE INDEX idx_user_roles_role ON user_roles(role_id);
CREATE INDEX idx_user_roles_department ON user_roles(department_id);
CREATE INDEX idx_user_roles_position ON user_roles(position_id);
CREATE UNIQUE INDEX idx_user_roles_unique ON user_roles(user_id, organization_id);
```

#### 部门表 (departments)

```sql
CREATE TABLE departments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    parent_id UUID REFERENCES departments(id) ON DELETE SET NULL,
    manager_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT departments_name_unique_in_org UNIQUE (organization_id, name, parent_id)
);

-- 索引
CREATE INDEX idx_departments_organization ON departments(organization_id);
CREATE INDEX idx_departments_parent ON departments(parent_id);
CREATE INDEX idx_departments_manager ON departments(manager_id);
```

#### 职位表 (positions)

```sql
CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    title VARCHAR(100) NOT NULL,
    description TEXT,
    level INTEGER DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT positions_title_unique_in_org UNIQUE (organization_id, title)
);

-- 索引
CREATE INDEX idx_positions_organization ON positions(organization_id);
CREATE INDEX idx_positions_level ON positions(level);
```

#### 代理关系表 (user_proxies)

```sql
CREATE TABLE user_proxies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    principal_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    permissions JSONB DEFAULT '{}',
    starts_at TIMESTAMP WITH TIME ZONE NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT user_proxies_principal_agent_different CHECK (principal_id != agent_id),
    CONSTRAINT user_proxies_time_check CHECK (starts_at < expires_at OR expires_at IS NULL),
    CONSTRAINT user_proxies_unique UNIQUE (principal_id, agent_id, organization_id)
);

-- 索引
CREATE INDEX idx_user_proxies_principal ON user_proxies(principal_id);
CREATE INDEX idx_user_proxies_agent ON user_proxies(agent_id);
CREATE INDEX idx_user_proxies_organization ON user_proxies(organization_id);
CREATE INDEX idx_user_proxies_active ON user_proxies(is_active, expires_at);
CREATE UNIQUE INDEX idx_user_proxies_unique ON user_proxies(principal_id, agent_id, organization_id);
```

### 在线时间统计表

```sql
CREATE TABLE online_time_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    total_seconds INTEGER NOT NULL DEFAULT 0,
    sessions_count INTEGER NOT NULL DEFAULT 0,
    first_session_at TIMESTAMP WITH TIME ZONE,
    last_session_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT online_time_stats_unique UNIQUE (user_id, date)
);

-- 索引
CREATE INDEX idx_online_time_stats_user ON online_time_stats(user_id);
CREATE INDEX idx_online_time_stats_date ON online_time_stats(date);
CREATE UNIQUE INDEX idx_online_time_stats_unique ON online_time_stats(user_id, date);
```

### 系统表

#### 会话表 (sessions)

```sql
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT,
    is_active BOOLEAN DEFAULT true,
    
    -- 约束
    CONSTRAINT sessions_expires_check CHECK (expires_at > created_at)
);

-- 索引
CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_token ON sessions(token_hash);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX idx_sessions_active ON sessions(is_active, expires_at);
```

#### 通知表 (notifications)

```sql
CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type VARCHAR(50) NOT NULL,
    title VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    data JSONB DEFAULT '{}',
    is_read BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    read_at TIMESTAMP WITH TIME ZONE,
    
    -- 约束
    CONSTRAINT notifications_type_check CHECK (
        type IN ('new_message', 'room_invitation', 'user_joined', 'user_left', 'system_alert')
    )
);

-- 索引
CREATE INDEX idx_notifications_user ON notifications(user_id);
CREATE INDEX idx_notifications_type ON notifications(type);
CREATE INDEX idx_notifications_read ON notifications(is_read);
CREATE INDEX idx_notifications_created_at ON notifications(created_at DESC);
```

#### 文件存储表 (file_uploads)

```sql
CREATE TABLE file_uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    filename VARCHAR(255) NOT NULL,
    original_filename VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    storage_path VARCHAR(500) NOT NULL,
    storage_type VARCHAR(20) NOT NULL DEFAULT 'local',
    checksum VARCHAR(64),
    is_public BOOLEAN DEFAULT false,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    -- 约束
    CONSTRAINT file_uploads_storage_type_check CHECK (
        storage_type IN ('local', 's3', 'minio', 'azure')
    )
);

-- 索引
CREATE INDEX idx_file_uploads_user ON file_uploads(user_id);
CREATE INDEX idx_file_uploads_public ON file_uploads(is_public);
CREATE INDEX idx_file_uploads_expires_at ON file_uploads(expires_at);
CREATE INDEX idx_file_uploads_created_at ON file_uploads(created_at DESC);
```

## 📊 Kafka主题设计

### 主题配置

```yaml
# Kafka主题配置
topics:
  chat-events:
    partitions: 10
    replication-factor: 3
    retention-ms: 604800000  # 7 days
    cleanup-policy: delete
    compression-type: lz4
    
  user-events:
    partitions: 5
    replication-factor: 3
    retention-ms: 2592000000  # 30 days
    cleanup-policy: delete
    compression-type: lz4
    
  system-events:
    partitions: 3
    replication-factor: 3
    retention-ms: 5184000000  # 60 days
    cleanup-policy: delete
    compression-type: lz4
    
  notification-events:
    partitions: 5
    replication-factor: 3
    retention-ms: 604800000  # 7 days
    cleanup-policy: delete
    compression-type: lz4
    
  search-events:
    partitions: 3
    replication-factor: 3
    retention-ms: 86400000   # 1 day
    cleanup-policy: delete
    compression-type: lz4
```

### 消息格式

#### 聊天事件消息格式

```json
{
  "event_id": "uuid",
  "event_type": "message_sent",
  "event_version": "1.0",
  "timestamp": "2024-01-15T10:30:00Z",
  "producer": "chatroom-api-1",
  "data": {
    "message": {
      "id": "uuid",
      "room_id": "uuid",
      "user_id": "uuid",
      "content": "Hello World!",
      "message_type": "text",
      "created_at": "2024-01-15T10:30:00Z"
    },
    "room_metadata": {
      "name": "General",
      "is_private": false,
      "member_count": 42
    }
  },
  "metadata": {
    "source_ip": "192.168.1.100",
    "user_agent": "Mozilla/5.0...",
    "trace_id": "trace-123"
  }
}
```

#### 用户事件消息格式

```json
{
  "event_id": "uuid",
  "event_type": "user_logged_in",
  "event_version": "1.0",
  "timestamp": "2024-01-15T10:30:00Z",
  "producer": "chatroom-api-1",
  "data": {
    "user_id": "uuid",
    "username": "john_doe",
    "email": "john@example.com",
    "session_id": "uuid",
    "ip_address": "192.168.1.100"
  },
  "metadata": {
    "device_info": {
      "type": "desktop",
      "os": "Windows 10",
      "browser": "Chrome"
    },
    "location": {
      "country": "US",
      "city": "New York"
    }
  }
}
```

## 🚀 Redis缓存结构

### 缓存键命名规范

```
# 用户相关缓存
user:{user_id}:profile                    # 用户个人信息
user:{user_id}:session                   # 用户会话信息
user:{user_id}:permissions              # 用户权限
user:{user_id}:online_status             # 用户在线状态
user:{user_id}:unread_count              # 未读消息计数

# 聊天室相关缓存
room:{room_id}:info                      # 房间信息
room:{room_id}:members                   # 房间成员列表
room:{room_id}:online_count              # 在线成员数量
room:{room_id}:recent_messages          # 最近消息
room:{room_id}:settings                  # 房间设置

# 消息相关缓存
message:{message_id}:content             # 消息内容
room:{room_id}:messages:{page}           # 分页消息
user:{user_id}:messages:{room_id}:{page}  # 用户在房间的消息

# 系统相关缓存
system:feature_flags                     # 功能开关
system:rate_limits:{ip}                  # IP速率限制
system:metrics                           # 系统指标
system:maintenance                       # 维护模式状态

# 组织相关缓存
org:{org_id}:info                        # 组织信息
org:{org_id}:members                     # 组织成员
org:{org_id}:roles                       # 组织角色
org:{org_id}:permissions:{user_id}      # 用户在组织的权限

# 临时缓存
temp:{temp_id}:data                      # 临时数据
temp:verification:{email}                # 邮箱验证码
temp:password_reset:{user_id}            # 密码重置令牌
```

### 缓存数据结构示例

```json
# 用户个人信息缓存
{
  "id": "uuid",
  "username": "john_doe",
  "email": "john@example.com",
  "avatar_url": "https://example.com/avatar.jpg",
  "status": "active",
  "last_active_at": "2024-01-15T10:30:00Z",
  "created_at": "2024-01-01T00:00:00Z",
  "cached_at": "2024-01-15T10:30:00Z"
}

# 房间信息缓存
{
  "id": "uuid",
  "name": "General",
  "description": "General discussion room",
  "owner_id": "uuid",
  "is_private": false,
  "member_count": 42,
  "online_count": 15,
  "created_at": "2024-01-01T00:00:00Z",
  "cached_at": "2024-01-15T10:30:00Z"
}

# 最近消息缓存
{
  "messages": [
    {
      "id": "uuid",
      "user_id": "uuid",
      "username": "john_doe",
      "content": "Hello World!",
      "message_type": "text",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "total_count": 150,
  "page": 1,
  "page_size": 50,
  "cached_at": "2024-01-15T10:30:00Z"
}
```

### 缓存策略

```rust
// 缓存过期时间配置
pub struct CacheConfig {
    // 用户相关缓存
    pub user_profile_ttl: Duration,        // 30分钟
    pub user_session_ttl: Duration,        // 24小时
    pub user_permissions_ttl: Duration,     // 15分钟
    pub user_online_status_ttl: Duration,  // 5分钟
    
    // 聊天室相关缓存
    pub room_info_ttl: Duration,           // 1小时
    pub room_members_ttl: Duration,        // 15分钟
    pub room_messages_ttl: Duration,        // 5分钟
    
    // 系统相关缓存
    pub system_feature_flags_ttl: Duration, // 10分钟
    pub system_rate_limits_ttl: Duration,   // 1分钟
    
    // 临时缓存
    pub temp_data_ttl: Duration,           // 5分钟
    pub verification_code_ttl: Duration,    // 10分钟
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            user_profile_ttl: Duration::minutes(30),
            user_session_ttl: Duration::hours(24),
            user_permissions_ttl: Duration::minutes(15),
            user_online_status_ttl: Duration::minutes(5),
            room_info_ttl: Duration::hours(1),
            room_members_ttl: Duration::minutes(15),
            room_messages_ttl: Duration::minutes(5),
            system_feature_flags_ttl: Duration::minutes(10),
            system_rate_limits_ttl: Duration::minutes(1),
            temp_data_ttl: Duration::minutes(5),
            verification_code_ttl: Duration::minutes(10),
        }
    }
}
```

## 🔍 索引优化策略

### 数据库索引优化

```sql
-- 1. 用户相关查询优化
-- 按用户名搜索
CREATE INDEX CONCURRENTLY idx_users_username_search ON users 
WHERE username LIKE '%search_term%';

-- 按邮箱搜索
CREATE INDEX CONCURRENTLY idx_users_email_search ON users 
WHERE email LIKE '%search_term%';

-- 按状态和活跃时间筛选
CREATE INDEX CONCURRENTLY idx_users_status_active ON users(status, last_active_at DESC)
WHERE status = 'active';

-- 2. 聊天室相关查询优化
-- 按房间名称搜索
CREATE INDEX CONCURRENTLY idx_rooms_name_search ON chat_rooms 
WHERE name LIKE '%search_term%';

-- 按拥有者和房间类型筛选
CREATE INDEX CONCURRENTLY idx_rooms_owner_type ON chat_rooms(owner_id, is_private, created_at DESC);

-- 3. 消息相关查询优化
-- 按房间和时间范围查询
CREATE INDEX CONCURRENTLY idx_messages_room_time ON messages(room_id, created_at DESC)
WHERE is_deleted = false;

-- 按用户和时间范围查询
CREATE INDEX CONCURRENTLY idx_messages_user_time ON messages(user_id, created_at DESC)
WHERE is_deleted = false;

-- 全文搜索优化
CREATE INDEX CONCURRENTLY idx_messages_full_text ON messages 
USING gin(to_tsvector('english', content))
WHERE is_deleted = false AND message_type = 'text';

-- 4. 房间成员查询优化
-- 按房间和角色筛选
CREATE INDEX CONCURRENTLY idx_room_members_room_role ON room_members(room_id, role)
WHERE is_muted = false AND notifications_enabled = true;

-- 5. 组织相关查询优化
-- 按组织名称搜索
CREATE INDEX CONCURRENTLY idx_orgs_name_search ON organizations 
WHERE name LIKE '%search_term%' AND is_active = true;

-- 按用户和组织查询角色
CREATE INDEX CONCURRENTLY idx_user_roles_org_user ON user_roles(organization_id, user_id, is_active)
WHERE is_active = true;
```

### 分区策略

```sql
-- 消息表按时间分区
CREATE TABLE messages (
    -- 表结构同上
) PARTITION BY RANGE (created_at);

-- 创建分区
CREATE TABLE messages_2024_01 PARTITION OF messages
FOR VALUES FROM ('2024-01-01') TO ('2024-02-01');

CREATE TABLE messages_2024_02 PARTITION OF messages
FOR VALUES FROM ('2024-02-01') TO ('2024-03-01');

-- 自动创建下个月分区的函数
CREATE OR REPLACE FUNCTION create_monthly_partition()
RETURNS void AS $$
DECLARE
    partition_name TEXT;
    start_date TEXT;
    end_date TEXT;
BEGIN
    partition_name := 'messages_' || to_char(CURRENT_DATE + INTERVAL '1 month', 'YYYY_MM');
    start_date := to_char(CURRENT_DATE + INTERVAL '1 month', 'YYYY-MM-01');
    end_date := to_char(CURRENT_DATE + INTERVAL '2 month', 'YYYY-MM-01');
    
    EXECUTE format('
        CREATE TABLE IF NOT EXISTS %I PARTITION OF messages
        FOR VALUES FROM (%L) TO (%L)
    ', partition_name, start_date, end_date);
END;
$$ LANGUAGE plpgsql;

-- 设置每月执行一次
CREATE EXTENSION IF NOT EXISTS pg_cron;
SELECT cron.schedule('0 0 1 * *', $$SELECT create_monthly_partition()$$);
```

### 查询优化建议

```sql
-- 1. 使用EXPLAIN分析查询计划
EXPLAIN ANALYZE SELECT * FROM messages 
WHERE room_id = 'uuid' AND created_at > NOW() - INTERVAL '7 days'
ORDER BY created_at DESC
LIMIT 50;

-- 2. 常用查询优化模式
-- 优化前
SELECT m.*, u.username 
FROM messages m 
JOIN users u ON m.user_id = u.id 
WHERE m.room_id = 'uuid' 
ORDER BY m.created_at DESC 
LIMIT 50;

-- 优化后（使用覆盖索引）
SELECT m.*, u.username 
FROM messages m 
JOIN users u ON m.user_id = u.id 
WHERE m.room_id = 'uuid' 
ORDER BY m.created_at DESC 
LIMIT 50;

-- 3. 批量操作优化
-- 优化前
UPDATE messages SET is_deleted = true 
WHERE user_id = 'uuid' AND created_at < NOW() - INTERVAL '30 days';

-- 优化后（使用批量更新）
UPDATE messages SET is_deleted = true 
WHERE user_id = 'uuid' AND created_at < NOW() - INTERVAL '30 days';

-- 4. 分页查询优化
-- 优化前
SELECT * FROM messages 
WHERE room_id = 'uuid' 
ORDER BY created_at DESC 
LIMIT 50 OFFSET 1000;

-- 优化后（使用游标分页）
SELECT * FROM messages 
WHERE room_id = 'uuid' AND created_at < '2024-01-15T10:30:00Z'
ORDER BY created_at DESC 
LIMIT 50;
```

## 📊 数据统计表

### 每日统计表

```sql
CREATE TABLE daily_stats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stat_date DATE NOT NULL,
    total_users INTEGER NOT NULL DEFAULT 0,
    active_users INTEGER NOT NULL DEFAULT 0,
    new_users INTEGER NOT NULL DEFAULT 0,
    total_rooms INTEGER NOT NULL DEFAULT 0,
    new_rooms INTEGER NOT NULL DEFAULT 0,
    total_messages INTEGER NOT NULL DEFAULT 0,
    new_messages INTEGER NOT NULL DEFAULT 0,
    storage_used_bytes BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    CONSTRAINT daily_stats_unique UNIQUE (stat_date)
);

-- 索引
CREATE INDEX idx_daily_stats_date ON daily_stats(stat_date);
```

### 系统监控表

```sql
CREATE TABLE system_metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    tags JSONB DEFAULT '{}',
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    server_id VARCHAR(100),
    
    CONSTRAINT system_metrics_name_check CHECK (
        metric_name IN (
            'cpu_usage', 'memory_usage', 'disk_usage', 'network_io',
            'active_connections', 'message_rate', 'error_rate', 'response_time'
        )
    )
);

-- 索引
CREATE INDEX idx_system_metrics_name_time ON system_metrics(metric_name, timestamp DESC);
CREATE INDEX idx_system_metrics_server ON system_metrics(server_id, timestamp DESC);
```

---

**下一步**: 阅读[07-error-handling-and-testing.md](./07-error-handling-and-testing.md)了解错误处理和测试策略。
