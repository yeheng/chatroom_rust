# 精简架构设计文档

## 1. 背景与目标

这个项目的首要目标是交付一个能跑、可维护的聊天室后端 MVP，而不是写论文。架构必须遵守以下四条：

1. **先把用户路径跑通**：用户认证、加入房间、消息实时广播、历史消息查询。
2. **消灭特殊情况**：相同的数据用相同的路径处理，少写 if/else，少搞 feature flag。
3. **保持向后兼容**：未来扩展必须建立在当前模型之上，不得破坏现有接口和数据。
4. **真实可行**：所有示例代码、接口约定都能落地实现，不和领域层 API 打架。

## 2. 技术栈裁剪

只保留实现 MVP 必需的组件：

- **Runtime**: `tokio`
- **Web 框架**: `axum`
- **数据库**: `PostgreSQL` (通过 `sqlx`)
- **缓存/会话**: `Redis`（可选，针对短期缓存和限流，不是硬依赖）
- **认证**: `jwt` (签发/校验放在 Web API 层)
- **日志**: `tracing`

Kafka、复杂总线、异步事件风暴统统掐掉。未来真有跨进程广播或多集群需求，再单独设计事件层。

## 3. 分层结构 (MVP)

```
+-------------------+
|   Web API Layer   |  -> HTTP / WebSocket 入口
+-------------------+
          |
          v
+-------------------+
| Application Layer |  -> 用例协调器，薄服务
+-------------------+
          |
          v
+-------------------+
|   Domain Layer    |  -> 实体、值对象、领域服务
+-------------------+
          |
          v
+-------------------+
|Infrastructure Lyr |  -> Repository / Adapter
+-------------------+
```

### 3.1 Web API Layer

- Axum Router 负责 HTTP 路径、参数校验、响应序列化。
- WebSocket handler 只做一件事：认证用户 -> 建立房间连接 -> 委托给应用层广播。
- 不在这里做数据库调用，所有业务逻辑都通过应用层接口完成。

### 3.2 Application Layer

- 每个用例一个 service，例如 `ChatService::send_message`、`ChatService::fetch_history`。
- Service 只拿明确定义的 DTO，不直接暴露领域实体。
- Service 负责事务边界：组合多个 Repository 调用，必要时开启 db 事务。
- 没有 CommandBus/QueryBus，直接调用，保持调用链简单可追踪。

### 3.3 Domain Layer

- 实体只关心业务状态和校验，不依赖框架/加密库/序列化库。
- 需要密码哈希？在应用层处理，领域层接收 `PasswordHash` 值对象。
- 错误使用自定义 `DomainError`，不要抛 `anyhow` 字符串。
- 领域事件在 MVP 不启用，若未来需要，可扩展成同步回调。

### 3.4 Infrastructure Layer

- `sqlx` Repository 实现落在这里，对外暴露领域层定义的 trait。
- WebSocket 连接管理（房间 -> 连接集合）放在这里，用 `tokio::sync::RwLock` 控制共享状态。
- Redis 仅用于可选缓存/限流，属于适配器，多态注入。

## 4. 数据模型

MVP 必备的四张表：

1. `users`
2. `chat_rooms`
3. `room_members`
4. `messages`

### 4.1 users

```
id UUID PK
username TEXT UNIQUE
email TEXT UNIQUE
password_hash TEXT
status TEXT (active|inactive|suspended)
created_at TIMESTAMPTZ default now()
updated_at TIMESTAMPTZ default now()
```

索引：`(username)`, `(email)`, `(status)`。

### 4.2 chat_rooms

```
id UUID PK
name TEXT UNIQUE
owner_id UUID FK -> users
is_private BOOLEAN
password_hash TEXT NULLABLE (仅私有房间使用)
created_at TIMESTAMPTZ default now()
updated_at TIMESTAMPTZ default now()
```

索引：`(owner_id)`, `(is_private)`。

### 4.3 room_members

```
id UUID PK
room_id UUID FK -> chat_rooms
user_id UUID FK -> users
role TEXT (owner|admin|member)
joined_at TIMESTAMPTZ
last_read_message_id UUID NULLABLE
UNIQUE(room_id, user_id)
```

索引：`(room_id)`, `(user_id)`。

### 4.4 messages

```
id UUID PK
room_id UUID FK -> chat_rooms
user_id UUID FK -> users
content TEXT
message_type TEXT (text|image|file)
created_at TIMESTAMPTZ default now()
updated_at TIMESTAMPTZ NULLABLE
reply_to_message_id UUID NULLABLE FK -> messages
is_deleted BOOLEAN default false
```

索引：`(room_id, created_at DESC)`, `(user_id)`, `(reply_to_message_id)`。
全文检索、JSONB 元数据等全部延后。

## 5. WebSocket 广播模型

- WebSocket 连接表：`room_id -> HashSet<ConnectionId>`。
- 新消息流程：
  1. Web API 层收到消息，调用 `ChatService::send_message`。
  2. Service 校验成员 -> 调用领域实体构造消息 -> Repository 写入。
  3. Service 调用基础设施的 `WebSocketBroadcaster::broadcast(room_id, payload)`。
  4. 广播器遍历房间连接集合，异步写出消息。失败拆链接，避免脏状态。
- 没有 Kafka；同一进程内保证顺序和低延迟。多实例部署时再引入集中式广播（比如 Redis pub/sub），届时再写新文档。

## 6. 配置与可选特性

- 必填配置项：数据库连接、JWT secret、HTTP 端口。
- 可选配置项：Redis 连接信息、WebSocket 心跳间隔、日志级别。
- 所有 feature flag 默认关闭，且 MVP 不引入组织、机器人、代理等概念。
- 将潜在扩展写成单独 RFC：
  - `RFC-001: 多实例 WebSocket 广播`
  - `RFC-002: 组织与角色`
  - `RFC-003: 消息审计与检索`

## 7. 兼容性策略

- 所有数据库 schema 通过 migration 管理，禁止直接修改历史字段含义。
- 对外 API 保持稳定：URL、请求/响应字段一旦发布，后续兼容升级。
- WebSocket 消息格式固定为 `{ "type": "...", "payload": {...} }`，增加新字段必须向后兼容。

## 8. 下一步工作

1. 实现领域实体与 Repository trait，确认编译无第三方耦合。
2. 实现 `ChatService` / `UserService` MVP 版本（无总线、无事件风暴）。
3. 搭建 Axum Router，验证 HTTP + WebSocket 流程闭环。
4. 编写基础集成测试：注册 -> 登录 -> 创建房间 -> 加入 -> 发送消息 -> 拉取历史。
5. 待 MVP 跑通再讨论扩展需求，所有新特性先提交 RFC。
