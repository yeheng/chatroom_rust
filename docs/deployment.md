# Chatroom 生产环境部署指南

## 系统架构要求

本系统是一个分布式聊天室应用，依赖以下核心组件：

### 🚨 硬性要求：Redis 高可用性

**本系统在生产环境中必须部署高可用的 Redis**

#### 为什么 Redis 是必需的？
- **消息广播**：所有聊天消息通过 Redis Pub/Sub 广播给在线用户
- **用户在线状态**：实时跟踪和管理用户连接状态
- **消息限流**：防止用户发送过于频繁的消息
- **消息序列化**：确保消息在多实例间的顺序一致性

#### Redis 故障的影响
如果 Redis 不可用：
- ✅ 用户可以发送消息（消息会保存到数据库）
- ❌ 其他用户无法实时接收消息
- ❌ 用户在线状态功能失效
- ❌ 消息广播失败，API 返回 5xx 错误

#### 推荐的 Redis 部署方案

##### 1. Redis Sentinel（推荐）
```yaml
# 配置示例
broadcast:
  redis_url: "redis+sentinel://mymaster/redis-sentinel1:26379,redis-sentinel2:26379,redis-sentinel3:26379"
```

**优点**：
- 自动故障转移
- 高可用性
- 成熟的解决方案

**最低要求**：
- 3 个 Sentinel 实例
- 1 个 Master + 1 个 Slave

##### 2. Redis Cluster
```yaml
# 配置示例
broadcast:
  redis_url: "redis-cluster://redis-node1:6379,redis-node2:6379,redis-node3:6379,redis-node4:6379,redis-node5:6379,redis-node6:6379"
```

**优点**：
- 数据分片，支持大规模部署
- 自动故障转移
- 水平扩展

**最低要求**：
- 6 个节点（3 主 3 从）

##### 3. 云服务商托管 Redis
- **AWS ElastiCache for Redis**
- **Azure Cache for Redis**
- **Google Cloud Memorystore**

### 数据库要求

- PostgreSQL 12+
- 生产环境需要主从复制或高可用方案
- 建议配置定期备份

### 应用服务器要求

- 支持 Rust 运行时
- 建议至少 2 个实例以实现高可用
- 负载均衡器支持 WebSocket

## 部署配置

### 环境变量配置

```bash
# 数据库配置
DATABASE_URL=postgres://user:password@postgres-host:5432/chatroom

# Redis 配置
REDIS_URL=redis://redis-host:6379
BROADCAST_REDIS_URL=redis+sentinel://mymaster/sentinel1:26379,sentinel2:26379

# JWT 配置
JWT_SECRET=your-super-secret-jwt-key-at-least-32-characters

# 服务器配置
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
```

### 配置文件示例

生产环境配置文件 `config/production.yml`：

```yaml
database:
  url: ${DATABASE_URL}
  max_connections: 20

redis:
  url: ${REDIS_URL}
  max_connections: 20

broadcast:
  capacity: 1024
  # 使用 Sentinel 模式
  redis_url: ${BROADCAST_REDIS_URL}

jwt:
  secret: ${JWT_SECRET}
  expiration_hours: 24

server:
  host: ${SERVER_HOST}
  port: ${SERVER_PORT}
  bcrypt_cost: 12
```

## 健康检查和监控

### 关键指标监控

1. **Redis 连接状态**
2. **消息广播延迟**
3. **数据库连接池使用率**
4. **WebSocket 连接数**
5. **API 响应时间**

### 日志监控

关注以下错误模式：
- Redis 连接失败
- 消息广播失败
- 数据库连接超时
- WebSocket 连接异常

## 容灾和备份

### 数据备份策略
- PostgreSQL 每日全量备份
- Redis RDB 快照 + AOF 持久化
- 配置文件版本控制

### 故障恢复流程
1. Redis 故障：自动切换到备用节点
2. 数据库故障：切换到备用数据库
3. 应用服务器故障：负载均衡器自动剔除

## 性能优化

### Redis 优化
- 启用 TCP keepalive
- 配置合适的超时时间
- 监控内存使用率

### 数据库优化
- 配置适当的连接池大小
- 创建必要的索引
- 定期清理过期数据

## 安全考虑

- 所有网络通信使用 TLS
- Redis 配置密码认证
- 数据库连接使用 SSL
- 定期更新依赖包

---

**重要提醒**：本系统依赖 Redis 作为核心消息总线，任何降低 Redis 可用性的尝试（如使用本地缓存替代）都会破坏系统的分布式特性。请务必按照上述要求部署 Redis 高可用方案。