# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个基于 Rust 的高性能聊天室后台系统，采用清洁架构（Clean Architecture）设计模式。系统使用 tokio 异步运行时、axum web 框架、Kafka 消息队列、PostgreSQL 数据库和 Redis 缓存。

系统实现了 **Feature Flag 架构**，支持企业级功能的渐进式启用，包括组织管理、权限系统、代理关系、机器人消息和在线统计等高级功能。
[@docs/README.md](./docs/README.md)

## 核心架构

项目采用分层架构，从内到外分为：

1. **Domain 层** (`crates/domain/`) - 核心业务逻辑和实体
2. **Application 层** (`crates/application/`) - 用例和应用服务  
3. **Infrastructure 层** (`crates/infrastructure/`) - 外部依赖和技术实现
4. **Web API 层** (`crates/web-api/`) - HTTP/WebSocket 接口
5. **Main 层** (`crates/main/`) - 应用程序入口

### 依赖关系

外层依赖内层，内层不依赖外层：

- Web API → Application → Domain
- Infrastructure → Domain  
- Main → 所有层

## 开发命令

### 构建和运行

```bash
# 构建整个项目
cargo build

# 构建特定 crate
cargo build -p domain
cargo build -p application
cargo build -p infrastructure
cargo build -p web-api
cargo build -p main

# 运行主应用
cargo run -p main

# 开发模式运行（自动重载）
cargo watch -x run -p main
```

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定 crate 的测试
cargo test -p domain
cargo test -p application

# 运行端到端测试和性能测试
./scripts/run-e2e-tests.sh

# 运行带测试功能的测试
cargo test --features testing

# 运行集成测试
cargo test --test integration

# 仅运行端到端测试
cargo test -p tests e2e_tests -- --test-threads=1 --nocapture

# 仅运行性能测试
cargo test -p tests performance_tests -- --test-threads=1 --nocapture
```

### 代码质量

```bash
# 格式化代码
cargo fmt

# 检查代码风格
cargo fmt --check

# 静态分析
cargo clippy

# 运行 clippy 并修复建议
cargo clippy --fix
```

### 文档

```bash
# 生成文档
cargo doc --no-deps

# 在浏览器中打开文档
cargo doc --no-deps --open
```

## 关键技术组件

### 领域模型 (Domain)

- **Entities**: `User`, `ChatRoom`, `Message` 核心实体
- **Enterprise Entities**: `Organization`, `Role`, `Permission`, `Bot`, `ProxyRelationship` 企业级实体
- **Value Objects**: 用户状态、消息类型等
- **Domain Services**: 业务规则验证
- **Feature Flags**: 动态功能开关系统
- **Errors**: 领域特定错误类型

### 应用服务 (Application)

- **Command Handlers**: 处理写操作（创建房间、发送消息等）
- **Query Handlers**: 处理读操作（查询消息历史等）
- **Application Services**: 协调领域对象和基础设施
- **Enterprise Services**: 组织管理、权限控制、代理系统等企业级服务
- **Feature Flag Service**: 功能开关管理

### 基础设施 (Infrastructure)

- **Kafka**: 消息队列，用于事件驱动架构
- **Redis**: 缓存和 Pub/Sub，用于实时消息分发
- **PostgreSQL**: 数据持久化
- **WebSocket**: 实时通信

### Web API (Web API)

- **REST API**: 用户认证、房间管理等
- **WebSocket**: 实时消息通信
- **JWT**: 无状态认证

## 错误处理

使用 `thiserror` 定义分层错误类型：

- **Domain Error**: 业务逻辑错误
- **Application Error**: 应用层错误  
- **Infrastructure Error**: 基础设施错误

## 开发注意事项

### 特性标志 (Features)

- `testing`: 启用测试相关功能（mock 等）
- 默认特性为空，生产环境不需要额外特性

### 异步模式

- 全面使用 `async/await`
- 使用 `tokio` 作为运行时
- 避免阻塞操作

### 配置管理

### 环境变量配置

- 使用 `figment` 进行配置管理
- 支持环境变量和配置文件
- 敏感信息通过环境变量传递

### Feature Flag配置

系统支持通过环境变量动态启用企业级功能：

```bash
# 启用组织管理
export ENABLE_ORGANIZATIONS=true

# 启用用户角色和权限系统
export ENABLE_USER_ROLES=true

# 启用代理系统
export ENABLE_PROXY_SYSTEM=true

# 启用机器人消息
export ENABLE_BOT_MESSAGES=true

# 启用在线时长统计
export ENABLE_ONLINE_STATISTICS=true
```

### 企业级功能

#### 组织管理 (Feature Flag: enable_organizations)
- 支持层级组织结构（最多5层）
- 部门和职位管理
- 用户组织关联

#### 用户角色和权限系统 (Feature Flag: enable_user_roles)
- 细粒度权限控制
- 系统、组织、自定义角色
- 权限继承和动态分配

#### 代理关系管理 (Feature Flag: enable_proxy_system)
- 支持临时和长期代理
- 权限委托和活动记录
- 代理操作审计

#### 机器人消息系统 (Feature Flag: enable_bot_messages)
- 多种机器人类型（系统、聊天、通知等）
- 触发器和自动化流程
- 消息限流和权限控制

#### 用户在线统计 (Feature Flag: enable_online_statistics)
- 会话管理和时长统计
- 每日/月度活动报告
- 设备使用分析

### 日志记录

- 使用 `tracing` 进行结构化日志
- 支持日志级别过滤
- 集成 OpenTelemetry 追踪

## 部署架构

系统设计支持：

- 水平扩展（多实例部署）
- 故障转移和负载均衡
- 基于 Redis 的跨实例通信
- Kafka 消息持久化和重放

## 开发规范

### 代码风格

- 遵循 Rust 官方风格指南
- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 进行静态分析

### 测试策略

- 单元测试覆盖核心业务逻辑
- 集成测试覆盖组件交互
- 使用 `mockall` 进行依赖模拟

### 文档维护

- 为公共 API 添加文档注释
- 保持设计文档与代码同步
- 记录架构决策和设计考量
