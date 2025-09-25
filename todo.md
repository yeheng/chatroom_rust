#### **任务一（修正版）：用 `serde` 属性替换 DTO 层**

* **修改/功能：** 移除所有 DTO 结构体和它们的 `From` 实现，改用 `serde` 属性直接在领域实体上控制序列化行为。
* **逻辑/原理：** 利用工具本身的能力来解决问题，而不是自己发明一套笨拙的轮子。在事实的唯一来源（领域实体）上声明 API 视图，确保两者永远同步。
* **实现步骤：**
    1. 在你的领域实体（`User`, `ChatRoom` 等）上，找到所有不应该暴露给客户端的字段，比如 `password`、`is_deleted` 等。
    2. 在这些字段上添加 `#[serde(skip_serializing)]` 属性。
    3. 给所有需要作为 API 响应的领域实体加上 `#[derive(serde::Serialize)]`。
    4. **删除 `crates/application/src/dto.rs` 文件和所有 DTO。**
    5. 修改所有应用服务（`user_service.rs`, `chat_service.rs`），让它们直接返回领域实体，例如 `Result<User, ...>`。
    6. 修改所有 Web API 处理器（`routes.rs`），让它们直接序列化领域实体为 JSON。
* **验收标准：**
  * DTO 文件和结构体被彻底删除。
  * API 响应的 JSON 格式和之前完全一样，敏感字段（如 `password`）被成功隐藏。
  * 代码量大幅减少，维护成本降低。
  * 当你给领域实体添加一个新字段时，除非你明确标记它为 `skip_serializing`，否则它会自动出现在 API 响应中，这才是合理的默认行为。

#### **任务二：实现理智的枚举映射**

* **修改/功能：** 重构 `infrastructure/repository.rs` 中手动的字符串-枚举映射。
* **逻辑/原理：** 让编译器和数据库驱动来管理数据表示。避免使用脆弱的、可能与领域模型脱节的手动代码。
* **实现步骤：**
    1. 在你的 SQL 迁移文件（`0001_init.sql`）中为 `user_status` 和 `room_role` 创建对应的 `TYPE`。例如 `CREATE TYPE user_status AS ENUM ('active', 'inactive', 'suspended');`。更新你的 `users` 表来使用这个类型。
    2. 在你的领域枚举定义中（例如 `domain/src/user.rs`），加上 `#[derive(sqlx::Type)]` 和 `#[sqlx(type_name = "user_status")]`。
    3. 从 `infrastructure/repository.rs` 中删除 `status_as_str`, `status_from_str`, `role_as_str`, `role_from_str` 这些辅助函数。
    4. 更新 `sqlx` 查询，直接绑定枚举，并更新 `TryFrom<*Record>` 实现来直接读取它们。`sqlx` 会处理好映射。
* **测试用例：**
  * `infrastructure/tests/pg_repository.rs` 中的集成测试必须通过。
  * 创建一个用户，改变他的状态，获取他，并验证状态是正确的。
* **验收标准：**
  * 手动的枚举映射函数消失了。
  * 数据库 schema 使用了正确的 ENUM 类型。
  * 代码更简单、更安全。

---

#### **任务三：扔掉那个玩具广播器，换用Redis**

* **修改/功能：** 用 Redis Pub/Sub 实现替换 `LocalMessageBroadcaster`。
* **逻辑/原理：** 用正确的工具干正确的活。聊天系统本质上是个分布式问题。一个务实、简单的分布式消息总线，比如 Redis Pub/Sub，是正确的第一步。
* **实现步骤：**
    1. 将 `redis` crate 添加到 `infrastructure/Cargo.toml` 的依赖中。
    2. 在 `infrastructure/src/broadcast.rs` 中创建一个新的 `RedisMessageBroadcaster` 结构体。它将持有一个 Redis 连接池。
    3. 为它实现 `MessageBroadcaster` trait。`broadcast` 方法将把序列化后的 `MessageBroadcast` 负载 `PUBLISH` 到一个 Redis 频道（例如 `chat-room:{room_id}`）。
    4. 更新你的 `infrastructure/builder.rs` 和配置，以连接到 Redis。
    5. 在 `web-api/src/routes.rs` 中，`websocket_handler` 必须大改：
        a.  当一个连接建立时，获取一个 Redis 连接。
        b.  创建一个 `PubSub` 连接并 `SUBSCRIBE` 到相应房间的频道（例如 `chat-room:{room_id}`）。
        c.  使用 `tokio::select!` 循环来并发地：
            i.  等待来自 Redis `PubSub` 流的消息。当消息到达时，反序列化它并发送给 WebSocket 客户端。
            ii. 等待来自 WebSocket 客户端的消息（为未来的功能，如“正在输入”提示做准备）。
            iii. 优雅地处理任何一方的断连。
* **测试用例：**
  * 现有的 `ws_flow.rs` E2E 测试需要更新。它现在必须启动一个 Redis 容器（使用 `testcontainers`）。
  * 创建一个新的 E2E 测试：
        1. 模拟两个不同的 WebSocket 客户端连接到同一个房间。
        2. 客户端1 通过 HTTP API 发送一条消息。
        3. 断言客户端1和客户端2都通过他们的 WebSocket 连接收到了这条消息。
* **验收标准：**
  * `LocalMessageBroadcaster` 被删除。
  * 应用程序正确地使用 Redis 在不同客户端之间广播消息。
  * 系统现在可以水平扩展了（你可以运行多个服务器实例）。

### **任务四：真正的认证和授权 (Authentication & Authorization)**

* **【功能】**：用户登录后，系统必须知道他是谁，并且他接下来的所有操作都必须经过身份验证。
* **【为什么现在就需要】**：你现在的 API 是个笑话。`join_room` 居然能在 payload 里指定 `user_id`？这意味着我知道了你的用户 ID，就能冒充你加入任何房间。这不是聊天室，这是个“请随意冒充他人”的模拟器。**这是你现在最优先要修复的安全漏洞，没有之一。**
* **【Linus式实现方案】**：
    1. **用 JWT (JSON Web Tokens)。** 别自己发明轮子。它够简单，够实用。
    2. **登录 API (`/auth/login`)**：验证成功后，不要只返回用户信息。生成一个 JWT，把 `user_id` 和一个过期时间 (`exp`) 放进去。把这个 token 返回给客户端。
    3. **Axum 中间件 (Middleware)**：写一个 Axum `extractor`。这个 extractor 的工作就是从请求头 `Authorization: Bearer <token>` 里解析出 JWT，验证它，然后把里面的 `user_id` 提取出来。如果 token 无效或过期，直接拒绝请求。
    4. **改造所有需要登录的 API**：所有需要身份验证的路由，比如发消息、加入房间，都必须使用这个 extractor。API 函数的签名应该像这样：`async fn send_message(user: AuthenticatedUser, ...)`，其中 `AuthenticatedUser` 就是你的 extractor，它里面包含了验证过的 `user_id`。
    5. **干掉所有从 payload 获取用户身份的逻辑**：比如 `join_room` 和 `send_message`，发送者 ID 必须从 token 里来，绝不能从请求体里读。这样就从根本上消除了身份冒充的可能。
* **【要避免的愚蠢做法】**：
  * **别用 Session。** 那玩意儿需要在服务端存状态，不好扩展。JWT 是无状态的。
  * **别在 JWT 里放太多信息。** `user_id` 就够了。别把用户的权限、个人资料全塞进去。

---

### **任务五：让“角色”真正起作用 (Permissions)**

* **【功能】**：数据库里的 `role` 字段 (`owner`, `admin`, `member`) 不能只是个摆设。它必须决定用户能做什么，不能做什么。
* **【为什么现在就需要】**：没有权限控制，你的“所有者”和“成员”就没有任何区别。这不符合现实。
* **【Linus式实现方案】**：
    1. **从最简单的规则开始**：
        * 只有 `owner` 可以删除房间。
        * 只有 `owner` 和 `admin` 可以踢人。
        * 只有 `owner` 和 `admin` 可以修改房间信息（比如改名）。
    2. **在应用服务层做检查**：这是放权限逻辑最合适的地方。当一个请求进来时（比如“踢掉用户B”），服务层首先要：
        a.  通过你的 JWT 中间件，拿到**操作者A**的 `user_id`。
        b.  从数据库里查出操作者A在这个房间里的 `RoomMember` 记录，看他的 `role` 是什么。
        c.  根据规则判断他有没有权限执行这个操作。有就继续，没有就直接返回错误。
    3. **修正那个愚蠢的 `join_room` API**：一个用户不应该自己“加入”一个房间还带着自己的 ID。应该是房间的 `owner` 或 `admin` **邀请**他加入。把 API改成 `POST /api/v1/rooms/{room_id}/members`，请求体里是**被邀请者**的 `user_id` 或 email。然后检查**发起邀请的人**（从 JWT 里拿）有没有权限。
* **【要避免的愚蠢做法】**：
  * **别搞一套复杂的 RBAC（基于角色的访问控制）框架。** 你现在用不上。就用简单的 `if/match` 判断就足够了。在你需要之前，别增加复杂性。

---

### **任务六：在线状态 (Presence)**

* **【功能】**：用户需要知道一个房间里当前有哪些人在线。
* **【为什么现在就需要】**：没有在线列表的聊天室，感觉就像在跟一堆机器人说话。这是最基本的“实时”体验。
* **【Linus式实现方案】**：
    1. **继续用 Redis。** 它不只是个 Pub/Sub 工具。
    2. **用 Redis 的 `Set` 数据结构**：`Set` 是无序、唯一的集合，完美符合这个场景。
    3. **逻辑实现**：
        a.  当一个用户的 WebSocket **成功连接**时，执行 `SADD room:presence:<room_id> <user_id>`。然后通过 Pub/Sub 广播一条 `"user_joined"` 事件，让房间里所有客户端都知道。
        b.  当一个用户的 WebSocket **断开连接**时（不管是正常关闭还是异常掉线），执行 `SREM room:presence:<room_id> <user_id>`。然后广播一条 `"user_left"` 事件。
        c.  创建一个新的 API 接口 `GET /api/v1/rooms/{room_id}/members/online`。它的实现就是简单地调用 `SMEMBERS room:presence:<room_id>`，把在线用户 ID 列表返回给客户端。
* **【要避免的愚蠢做法】**：
  * **别在你的应用服务器内存里维护在线列表！** 这是最蠢的做法。如果你的服务重启，或者你水平扩展到多台服务器，内存里的状态就全乱了。Redis 这种外部、持久化的状态存储才是唯一正确的选择。
