//! 性能测试模块
//! 
//! 包含各种性能测试场景，包括API性能、WebSocket性能、数据库性能等

use anyhow::Result;
use std::time::{Duration, Instant};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{TestEnvironment, TestDataFactory, WebSocketTestClient, TestUser, TestChatRoom};
use crate::test_utils::{
    PerformanceMetrics, PerformanceReport, PerformanceRequirements, 
    ErrorRateTracker, ConcurrencyTestUtils, TestAssertions
};

/// API性能测试套件
pub struct ApiPerformanceTests {
    env: Arc<TestEnvironment>,
    factory: Arc<TestDataFactory>,
}

impl ApiPerformanceTests {
    pub fn new(env: Arc<TestEnvironment>) -> Self {
        let factory = Arc::new(TestDataFactory::new(env.clone()));
        Self { env, factory }
    }

    /// 测试用户认证API性能
    pub async fn test_auth_api_performance(&self) -> Result<PerformanceReport> {
        info!("开始用户认证API性能测试");
        
        // 创建测试用户
        let user = self.factory.create_test_user("perf_user").await?;
        let client = reqwest::Client::new();
        let base_url = self.env.app_base_url();
        
        let mut durations = Vec::new();
        let mut error_tracker = ErrorRateTracker::new();
        
        // 执行100次登录请求
        for _ in 0..100 {
            let start = Instant::now();
            
            let response = client
                .post(&format!("{}/api/auth/login", base_url))
                .json(&serde_json::json!({
                    "username": user.username,
                    "password": "test_password"
                }))
                .send()
                .await;
            
            let duration = start.elapsed();
            durations.push(duration);
            
            match response {
                Ok(resp) if resp.status().is_success() => {
                    error_tracker.record_success();
                }
                _ => {
                    error_tracker.record_failure();
                }
            }
        }
        
        // 验证错误率
        error_tracker.assert_error_rate_below(1.0)?; // 错误率应低于1%
        error_tracker.print_summary();
        
        let report = PerformanceMetrics::generate_report("用户认证API", &durations);
        report.print();
        
        // 验证性能要求
        report.assert_requirements(&PerformanceRequirements::api_requirements())?;
        
        Ok(report)
    }

    /// 测试发送消息API性能
    pub async fn test_send_message_api_performance(&self) -> Result<PerformanceReport> {
        info!("开始发送消息API性能测试");
        
        // 准备测试数据
        let user = self.factory.create_test_user("msg_perf_user").await?;
        let room = self.factory.create_test_room("Performance Test Room").await?;
        let token = self.factory.login_user(&user).await?;
        
        self.factory.join_room(&user, &room, None).await?;
        
        let client = reqwest::Client::new();
        let base_url = self.env.app_base_url();
        
        let mut durations = Vec::new();
        let mut error_tracker = ErrorRateTracker::new();
        
        // 发送100条消息
        for i in 0..100 {
            let start = Instant::now();
            
            let response = client
                .post(&format!("{}/api/v1/rooms/{}/messages", base_url, room.id))
                .header("Authorization", format!("Bearer {}", token))
                .json(&serde_json::json!({
                    "content": format!("性能测试消息 {}", i + 1),
                    "message_type": "text"
                }))
                .send()
                .await;
            
            let duration = start.elapsed();
            durations.push(duration);
            
            match response {
                Ok(resp) if resp.status().is_success() => {
                    error_tracker.record_success();
                }
                _ => {
                    error_tracker.record_failure();
                }
            }
        }
        
        error_tracker.assert_error_rate_below(1.0)?;
        error_tracker.print_summary();
        
        let report = PerformanceMetrics::generate_report("发送消息API", &durations);
        report.print();
        report.assert_requirements(&PerformanceRequirements::api_requirements())?;
        
        Ok(report)
    }

    /// 测试并发API性能
    pub async fn test_concurrent_api_performance(&self) -> Result<PerformanceReport> {
        info!("开始并发API性能测试");
        
        // 准备测试数据
        let users = self.factory.create_multiple_users(10).await?;
        let room = self.factory.create_test_room("Concurrent Test Room").await?;
        
        // 获取tokens
        let mut user_tokens = Vec::new();
        for user in &users {
            let token = self.factory.login_user(user).await?;
            self.factory.join_room(user, &room, None).await?;
            user_tokens.push((user.clone(), token));
        }
        
        let client = Arc::new(reqwest::Client::new());
        let base_url = self.env.app_base_url();
        let room_id = room.id;
        
        // 并发发送消息测试
        let (results, report) = ConcurrencyTestUtils::measure_concurrent_performance(
            "并发发送消息",
            100, // 总操作数
            10,  // 并发数
            |i| {
                let client = client.clone();
                let base_url = base_url.clone();
                let user_token = &user_tokens[i % user_tokens.len()];
                let token = user_token.1.clone();
                
                async move {
                    client
                        .post(&format!("{}/api/v1/rooms/{}/messages", base_url, room_id))
                        .header("Authorization", format!("Bearer {}", token))
                        .json(&serde_json::json!({
                            "content": format!("并发测试消息 {}", i + 1),
                            "message_type": "text"
                        }))
                        .send()
                        .await?
                        .error_for_status()?;
                    
                    Result::<()>::Ok(())
                }
            }
        ).await?;
        
        // 验证成功率
        TestAssertions::assert_success_rate_above(results.len(), 100, 95.0)?;
        
        report.print();
        report.assert_requirements(&PerformanceRequirements::api_requirements())?;
        
        Ok(report)
    }
}

/// WebSocket性能测试套件
pub struct WebSocketPerformanceTests {
    env: Arc<TestEnvironment>,
    factory: Arc<TestDataFactory>,
}

impl WebSocketPerformanceTests {
    pub fn new(env: Arc<TestEnvironment>) -> Self {
        let factory = Arc::new(TestDataFactory::new(env.clone()));
        Self { env, factory }
    }

    /// 测试WebSocket消息延迟
    pub async fn test_websocket_message_latency(&self) -> Result<PerformanceReport> {
        info!("开始WebSocket消息延迟测试");
        
        // 创建两个用户
        let user1 = self.factory.create_test_user("ws_user1").await?;
        let user2 = self.factory.create_test_user("ws_user2").await?;
        
        let token1 = self.factory.login_user(&user1).await?;
        let token2 = self.factory.login_user(&user2).await?;
        
        // 创建聊天室
        let room = self.factory.create_test_room("WebSocket Latency Test").await?;
        self.factory.join_room(&user1, &room, None).await?;
        self.factory.join_room(&user2, &room, None).await?;
        
        // 建立WebSocket连接
        let mut ws1 = WebSocketTestClient::connect(&self.env, &user1, &token1).await?;
        let mut ws2 = WebSocketTestClient::connect(&self.env, &user2, &token2).await?;
        
        ws1.join_room(room.id).await?;
        ws2.join_room(room.id).await?;
        
        // 等待加入确认
        tokio::time::sleep(Duration::from_millis(100)).await;
        ws2.clear_message_buffer().await;
        
        let mut durations = Vec::new();
        let mut error_tracker = ErrorRateTracker::new();
        
        // 测试消息延迟
        for i in 0..50 {
            let start = Instant::now();
            
            // user1发送消息
            let content = format!("延迟测试消息 {}", i + 1);
            if let Err(e) = ws1.send_chat_message(room.id, &content, domain::MessageType::Text).await {
                warn!("发送消息失败: {}", e);
                error_tracker.record_failure();
                continue;
            }
            
            // user2接收消息
            match ws2.receive_message_timeout(Duration::from_secs(5)).await? {
                Some(frame) => {
                    let duration = start.elapsed();
                    durations.push(duration);
                    error_tracker.record_success();
                    
                    // 验证消息内容
                    if let Some(received_content) = crate::websocket_client::MessageMatcher::extract_message_content(&frame) {
                        if received_content != content {
                            warn!("消息内容不匹配: 期望 '{}', 实际 '{}'", content, received_content);
                        }
                    }
                }
                None => {
                    warn!("未收到消息 {}", i + 1);
                    error_tracker.record_failure();
                }
            }
            
            // 避免过于频繁的发送
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        error_tracker.assert_error_rate_below(2.0)?; // WebSocket可能有更高的错误率
        error_tracker.print_summary();
        
        let report = PerformanceMetrics::generate_report("WebSocket消息延迟", &durations);
        report.print();
        report.assert_requirements(&PerformanceRequirements::websocket_requirements())?;
        
        Ok(report)
    }

    /// 测试并发WebSocket连接性能
    pub async fn test_concurrent_websocket_connections(&self) -> Result<PerformanceReport> {
        info!("开始并发WebSocket连接性能测试");
        
        let connection_count = 50;
        let room = self.factory.create_test_room("Concurrent WebSocket Test").await?;
        
        // 创建用户和获取tokens
        let mut user_tokens = Vec::new();
        for i in 0..connection_count {
            let user = self.factory.create_test_user(&format!("ws_conc_user_{}", i)).await?;
            let token = self.factory.login_user(&user).await?;
            self.factory.join_room(&user, &room, None).await?;
            user_tokens.push((user, token));
        }
        
        // 并发建立WebSocket连接
        let (clients, report) = ConcurrencyTestUtils::measure_concurrent_performance(
            "WebSocket连接建立",
            connection_count,
            10, // 限制并发数以避免过载
            |i| {
                let env = self.env.clone();
                let (user, token) = user_tokens[i].clone();
                
                async move {
                    WebSocketTestClient::connect(&env, &user, &token).await
                }
            }
        ).await?;
        
        info!("成功建立 {} 个WebSocket连接", clients.len());
        
        // 测试并发消息发送
        let message_count = 5;
        let start_time = Instant::now();
        
        let send_tasks: Vec<_> = clients
            .iter()
            .enumerate()
            .map(|(i, client)| {
                let room_id = room.id;
                async move {
                    for j in 0..message_count {
                        let content = format!("并发消息 {} 来自连接 {}", j + 1, i + 1);
                        client.send_chat_message(room_id, &content, domain::MessageType::Text).await?;
                    }
                    Result::<()>::Ok(())
                }
            })
            .collect();
        
        let send_results = futures::future::try_join_all(send_tasks).await?;
        let send_duration = start_time.elapsed();
        
        info!("所有连接发送 {} 条消息耗时: {:?}", 
              connection_count * message_count, send_duration);
        
        // 验证成功率
        TestAssertions::assert_success_rate_above(
            clients.len(), 
            connection_count, 
            90.0
        )?;
        
        report.print();
        
        Ok(report)
    }
}

/// 数据库性能测试套件
pub struct DatabasePerformanceTests {
    env: Arc<TestEnvironment>,
}

impl DatabasePerformanceTests {
    pub fn new(env: Arc<TestEnvironment>) -> Self {
        Self { env }
    }

    /// 测试数据库批量插入性能
    pub async fn test_batch_insert_performance(&self) -> Result<PerformanceReport> {
        info!("开始数据库批量插入性能测试");
        
        let pool = self.env.get_database_pool().await?;
        let insert_count = 1000;
        
        let (results, report) = ConcurrencyTestUtils::measure_concurrent_performance(
            "数据库批量插入",
            insert_count,
            20, // 数据库连接池限制并发数
            |i| {
                let pool = pool.clone();
                async move {
                    sqlx::query!(
                        r#"
                        INSERT INTO messages (id, room_id, user_id, content, message_type, created_at)
                        VALUES ($1, $2, $3, $4, 'text', NOW())
                        "#,
                        Uuid::new_v4(),
                        Uuid::new_v4(),
                        Uuid::new_v4(),
                        format!("批量插入测试消息 {}", i + 1)
                    )
                    .execute(&pool.0)
                    .await?;
                    
                    Result::<()>::Ok(())
                }
            }
        ).await?;
        
        TestAssertions::assert_success_rate_above(results.len(), insert_count, 99.0)?;
        
        report.print();
        report.assert_requirements(&PerformanceRequirements::database_requirements())?;
        
        Ok(report)
    }

    /// 测试数据库查询性能
    pub async fn test_query_performance(&self) -> Result<PerformanceReport> {
        info!("开始数据库查询性能测试");
        
        let pool = self.env.get_database_pool().await?;
        let query_count = 500;
        
        // 首先插入一些测试数据
        for i in 0..100 {
            sqlx::query!(
                r#"
                INSERT INTO messages (id, room_id, user_id, content, message_type, created_at)
                VALUES ($1, $2, $3, $4, 'text', NOW())
                "#,
                Uuid::new_v4(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                format!("查询测试数据 {}", i + 1)
            )
            .execute(&pool.0)
            .await?;
        }
        
        let (results, report) = ConcurrencyTestUtils::measure_concurrent_performance(
            "数据库查询",
            query_count,
            30, // 查询并发数可以更高
            |_| {
                let pool = pool.clone();
                async move {
                    let _rows = sqlx::query!(
                        "SELECT COUNT(*) as count FROM messages WHERE message_type = 'text'"
                    )
                    .fetch_one(&pool.0)
                    .await?;
                    
                    Result::<()>::Ok(())
                }
            }
        ).await?;
        
        TestAssertions::assert_success_rate_above(results.len(), query_count, 99.5)?;
        
        report.print();
        report.assert_requirements(&PerformanceRequirements::database_requirements())?;
        
        Ok(report)
    }
}

/// 综合性能测试套件
pub struct IntegratedPerformanceTests {
    env: Arc<TestEnvironment>,
    factory: Arc<TestDataFactory>,
}

impl IntegratedPerformanceTests {
    pub fn new(env: Arc<TestEnvironment>) -> Self {
        let factory = Arc::new(TestDataFactory::new(env.clone()));
        Self { env, factory }
    }

    /// 综合负载测试
    pub async fn test_full_system_load(&self) -> Result<Vec<PerformanceReport>> {
        info!("开始系统综合负载测试");
        
        let mut reports = Vec::new();
        
        // API性能测试
        let api_tests = ApiPerformanceTests::new(self.env.clone());
        reports.push(api_tests.test_concurrent_api_performance().await?);
        
        // WebSocket性能测试
        let ws_tests = WebSocketPerformanceTests::new(self.env.clone());
        reports.push(ws_tests.test_websocket_message_latency().await?);
        
        // 数据库性能测试
        let db_tests = DatabasePerformanceTests::new(self.env.clone());
        reports.push(db_tests.test_batch_insert_performance().await?);
        reports.push(db_tests.test_query_performance().await?);
        
        info!("系统综合负载测试完成");
        
        Ok(reports)
    }
}

/// 性能测试工具函数
pub async fn run_all_performance_tests(env: Arc<TestEnvironment>) -> Result<Vec<PerformanceReport>> {
    let mut all_reports = Vec::new();
    
    info!("开始运行所有性能测试...");
    
    // API性能测试
    let api_tests = ApiPerformanceTests::new(env.clone());
    all_reports.push(api_tests.test_auth_api_performance().await?);
    all_reports.push(api_tests.test_send_message_api_performance().await?);
    all_reports.push(api_tests.test_concurrent_api_performance().await?);
    
    // WebSocket性能测试
    let ws_tests = WebSocketPerformanceTests::new(env.clone());
    all_reports.push(ws_tests.test_websocket_message_latency().await?);
    all_reports.push(ws_tests.test_concurrent_websocket_connections().await?);
    
    // 数据库性能测试
    let db_tests = DatabasePerformanceTests::new(env.clone());
    all_reports.push(db_tests.test_batch_insert_performance().await?);
    all_reports.push(db_tests.test_query_performance().await?);
    
    // 综合测试
    let integrated_tests = IntegratedPerformanceTests::new(env.clone());
    let mut integrated_reports = integrated_tests.test_full_system_load().await?;
    all_reports.append(&mut integrated_reports);
    
    info!("所有性能测试完成，共生成 {} 个报告", all_reports.len());
    
    Ok(all_reports)
}