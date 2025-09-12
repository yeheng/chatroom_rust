# 端到端测试和性能测试

本文档说明如何运行聊天室系统的端到端测试和性能测试。

## 📋 测试覆盖范围

### 端到端测试 (E2E Tests)

- **用户认证流程测试**: 用户注册、登录、JWT token验证
- **聊天室生命周期测试**: 创建、加入、离开聊天室
- **实时消息测试**: WebSocket连接、消息发送和接收
- **并发用户测试**: 多用户同时在线和消息传递
- **错误处理测试**: 无效请求、未授权访问、速率限制

### 性能测试 (Performance Tests)

- **API性能测试**: 认证API、消息发送API的响应时间和吞吐量
- **WebSocket性能测试**: 消息延迟、连接建立时间
- **并发负载测试**: 高并发场景下的系统表现
- **数据库性能测试**: 批量插入、查询性能

## 🛠️ 环境要求

### 必需软件

- **Rust** (1.70+)
- **Docker** (用于测试容器)
- **PostgreSQL** (15+)
- **Redis** (7+)
- **Kafka** (可选，用于完整测试)

### 测试依赖

测试框架使用以下关键依赖：

- `testcontainers` - 容器化测试环境
- `reqwest` - HTTP客户端测试
- `tokio-tungstenite` - WebSocket客户端测试
- `wiremock` - 外部服务模拟

## 🚀 快速开始

### 1. 运行所有测试

使用提供的脚本运行完整测试套件：

```bash
# 确保Docker运行
docker --version

# 运行所有测试（包括E2E和性能测试）
./scripts/run-e2e-tests.sh

# 或者使用make（如果有Makefile）
make test-e2e
```

### 2. 分别运行测试

```bash
# 仅运行单元测试
cargo test --workspace --lib --bins

# 仅运行集成测试
cargo test -p tests integration_test jwt_integration

# 仅运行端到端测试
cargo test -p tests e2e_tests -- --test-threads=1 --nocapture

# 仅运行性能测试
cargo test -p tests performance_tests -- --test-threads=1 --nocapture
```

## 📊 性能基准要求

测试包含以下性能要求验证：

### API性能要求
- 平均响应时间: < 50ms
- P99响应时间: < 200ms
- 最小吞吐量: > 100 ops/s

### WebSocket性能要求
- 消息延迟P99: < 100ms
- 连接建立时间: < 1s
- 最小吞吐量: > 500 msg/s

### 数据库性能要求
- 批量插入平均时间: < 10ms
- 查询P99响应时间: < 50ms
- 最小吞吐量: > 1000 ops/s

### 系统级要求
- 错误率: < 0.1%
- 成功率: > 99.9%
- 并发用户支持: > 1000

## 🔧 测试配置

### 环境变量

```bash
# 数据库连接
export DATABASE_URL="postgres://test_user:test_password@localhost:5432/chatroom_test"

# Redis连接
export REDIS_URL="redis://localhost:6379"

# Kafka连接（可选）
export KAFKA_BROKERS="localhost:9092"

# 测试配置
export RUST_LOG="info"
export TEST_TIMEOUT="300"
export PERF_TEST_MODE="1"
```

### 测试容器配置

测试使用Docker容器提供隔离的测试环境：

```yaml
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: chatroom_test
      POSTGRES_USER: test_user
      POSTGRES_PASSWORD: test_password
    ports:
      - "5432:5432"

  redis:
    image: redis:7
    ports:
      - "6379:6379"
```

## 📈 测试报告

### 自动生成报告

测试完成后会自动生成以下报告：

- **性能报告**: `test-reports/performance-report.md`
- **覆盖率报告**: `test-reports/tarpaulin-report.html`
- **测试日志**: 控制台输出和文件日志

### 查看报告

```bash
# 查看性能报告
cat test-reports/performance-report.md

# 在浏览器中查看覆盖率报告
open test-reports/tarpaulin-report.html
```

### CI/CD集成

项目包含GitHub Actions工作流程（`.github/workflows/e2e-tests.yml`）：

- **自动运行**: 每次push和PR时自动运行
- **定时测试**: 每天凌晨2点运行性能测试
- **报告上传**: 自动上传测试结果和覆盖率报告
- **PR评论**: 在PR中自动评论性能测试结果

## 🐛 故障排除

### 常见问题

**1. Docker连接失败**
```bash
# 检查Docker状态
docker info

# 重启Docker服务
sudo systemctl restart docker  # Linux
# 或重启Docker Desktop (macOS/Windows)
```

**2. 端口冲突**
```bash
# 检查端口占用
lsof -i :5432  # PostgreSQL
lsof -i :6379  # Redis
lsof -i :9092  # Kafka

# 停止冲突的服务或修改测试配置
```

**3. 测试超时**
```bash
# 增加超时时间
export TEST_TIMEOUT=600  # 10分钟

# 或减少测试并发数
export TEST_CONCURRENCY=5
```

**4. 内存不足**
```bash
# 检查系统资源
free -h
docker stats

# 减少测试并发数或增加系统内存
```

### 调试测试

```bash
# 启用详细日志
export RUST_LOG=debug

# 单独运行失败的测试
cargo test -p tests test_specific_test_name -- --nocapture

# 保留测试容器用于调试
export NO_CLEANUP=1
```

## 📝 添加新测试

### 端到端测试

在 `crates/tests/src/e2e_tests.rs` 中添加新的测试函数：

```rust
#[tokio::test]
async fn test_new_feature_e2e() -> Result<()> {
    let test_env = TestEnvironment::new().await?;
    let app = test_env.start_app().await?;
    
    // 测试逻辑
    
    test_env.cleanup().await?;
    Ok(())
}
```

### 性能测试

在 `crates/tests/src/performance_tests.rs` 中添加新的性能测试：

```rust
impl ApiPerformanceTests {
    pub async fn test_new_api_performance(&self) -> Result<PerformanceReport> {
        // 性能测试逻辑
        
        let report = PerformanceMetrics::generate_report("新API", &durations);
        report.assert_requirements(&PerformanceRequirements::api_requirements())?;
        
        Ok(report)
    }
}
```

## 🔗 相关文档

- [架构设计文档](../docs/01-overview-and-architecture.md)
- [错误处理和测试策略](../docs/07-error-handling-and-testing.md)
- [WebSocket协议规范](../docs/08-websocket-message-protocol.md)
- [Task 11: 测试和部署](../docs/tasks/task-11-core-testing-deployment.md)