//! 测试工具集
//! 
//! 提供通用的测试辅助函数和断言工具

use anyhow::Result;
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// 性能指标计算工具
pub struct PerformanceMetrics;

impl PerformanceMetrics {
    /// 计算百分位数
    pub fn calculate_percentile(durations: &[Duration], percentile: f64) -> Duration {
        if durations.is_empty() {
            return Duration::from_millis(0);
        }
        
        let mut sorted = durations.to_vec();
        sorted.sort();
        
        let index = ((percentile / 100.0) * (sorted.len() as f64 - 1.0)) as usize;
        sorted[index.min(sorted.len() - 1)]
    }

    /// 计算平均值
    pub fn calculate_average(durations: &[Duration]) -> Duration {
        if durations.is_empty() {
            return Duration::from_millis(0);
        }
        
        let total: Duration = durations.iter().sum();
        total / durations.len() as u32
    }

    /// 计算标准差
    pub fn calculate_std_dev(durations: &[Duration], average: Duration) -> Duration {
        if durations.len() <= 1 {
            return Duration::from_millis(0);
        }
        
        let variance: f64 = durations
            .iter()
            .map(|d| {
                let diff = d.as_millis() as f64 - average.as_millis() as f64;
                diff * diff
            })
            .sum::<f64>() / (durations.len() - 1) as f64;
        
        Duration::from_millis(variance.sqrt() as u64)
    }

    /// 生成性能报告
    pub fn generate_report(operation: &str, durations: &[Duration]) -> PerformanceReport {
        if durations.is_empty() {
            return PerformanceReport {
                operation: operation.to_string(),
                total_operations: 0,
                min: Duration::from_millis(0),
                max: Duration::from_millis(0),
                average: Duration::from_millis(0),
                p50: Duration::from_millis(0),
                p95: Duration::from_millis(0),
                p99: Duration::from_millis(0),
                std_dev: Duration::from_millis(0),
                ops_per_second: 0.0,
            };
        }
        
        let min = *durations.iter().min().unwrap();
        let max = *durations.iter().max().unwrap();
        let average = Self::calculate_average(durations);
        let p50 = Self::calculate_percentile(durations, 50.0);
        let p95 = Self::calculate_percentile(durations, 95.0);
        let p99 = Self::calculate_percentile(durations, 99.0);
        let std_dev = Self::calculate_std_dev(durations, average);
        
        let total_time: Duration = durations.iter().sum();
        let ops_per_second = if total_time.as_millis() > 0 {
            (durations.len() as f64) / (total_time.as_millis() as f64 / 1000.0)
        } else {
            0.0
        };
        
        PerformanceReport {
            operation: operation.to_string(),
            total_operations: durations.len(),
            min,
            max,
            average,
            p50,
            p95,
            p99,
            std_dev,
            ops_per_second,
        }
    }
}

/// 性能报告
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    pub operation: String,
    pub total_operations: usize,
    pub min: Duration,
    pub max: Duration,
    pub average: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub std_dev: Duration,
    pub ops_per_second: f64,
}

impl PerformanceReport {
    /// 打印报告
    pub fn print(&self) {
        info!("=== {} 性能报告 ===", self.operation);
        info!("总操作数: {}", self.total_operations);
        info!("最小时间: {:?}", self.min);
        info!("最大时间: {:?}", self.max);
        info!("平均时间: {:?}", self.average);
        info!("P50: {:?}", self.p50);
        info!("P95: {:?}", self.p95);
        info!("P99: {:?}", self.p99);
        info!("标准差: {:?}", self.std_dev);
        info!("每秒操作数: {:.2}", self.ops_per_second);
        info!("===================================");
    }

    /// 验证性能要求
    pub fn assert_requirements(&self, requirements: &PerformanceRequirements) -> Result<()> {
        if let Some(max_avg) = requirements.max_average_duration {
            if self.average > max_avg {
                return Err(anyhow::anyhow!(
                    "{} 平均响应时间 {:?} 超过要求 {:?}",
                    self.operation, self.average, max_avg
                ));
            }
        }

        if let Some(max_p99) = requirements.max_p99_duration {
            if self.p99 > max_p99 {
                return Err(anyhow::anyhow!(
                    "{} P99响应时间 {:?} 超过要求 {:?}",
                    self.operation, self.p99, max_p99
                ));
            }
        }

        if let Some(min_ops) = requirements.min_ops_per_second {
            if self.ops_per_second < min_ops {
                return Err(anyhow::anyhow!(
                    "{} 每秒操作数 {:.2} 低于要求 {:.2}",
                    self.operation, self.ops_per_second, min_ops
                ));
            }
        }

        Ok(())
    }
}

/// 性能要求
#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    pub max_average_duration: Option<Duration>,
    pub max_p99_duration: Option<Duration>,
    pub min_ops_per_second: Option<f64>,
}

impl PerformanceRequirements {
    /// API性能要求
    pub fn api_requirements() -> Self {
        Self {
            max_average_duration: Some(Duration::from_millis(50)),
            max_p99_duration: Some(Duration::from_millis(200)),
            min_ops_per_second: Some(100.0),
        }
    }

    /// WebSocket性能要求
    pub fn websocket_requirements() -> Self {
        Self {
            max_average_duration: Some(Duration::from_millis(20)),
            max_p99_duration: Some(Duration::from_millis(100)),
            min_ops_per_second: Some(500.0),
        }
    }

    /// 数据库性能要求
    pub fn database_requirements() -> Self {
        Self {
            max_average_duration: Some(Duration::from_millis(10)),
            max_p99_duration: Some(Duration::from_millis(50)),
            min_ops_per_second: Some(1000.0),
        }
    }
}

/// 错误率统计工具
pub struct ErrorRateTracker {
    total_requests: usize,
    failed_requests: usize,
}

impl ErrorRateTracker {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            failed_requests: 0,
        }
    }

    pub fn record_success(&mut self) {
        self.total_requests += 1;
    }

    pub fn record_failure(&mut self) {
        self.total_requests += 1;
        self.failed_requests += 1;
    }

    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.failed_requests as f64) / (self.total_requests as f64) * 100.0
        }
    }

    pub fn success_rate(&self) -> f64 {
        100.0 - self.error_rate()
    }

    pub fn assert_error_rate_below(&self, max_error_rate: f64) -> Result<()> {
        let current_rate = self.error_rate();
        if current_rate > max_error_rate {
            return Err(anyhow::anyhow!(
                "错误率 {:.2}% 超过最大允许值 {:.2}%",
                current_rate, max_error_rate
            ));
        }
        Ok(())
    }

    pub fn print_summary(&self) {
        info!("=== 错误率统计 ===");
        info!("总请求数: {}", self.total_requests);
        info!("失败请求数: {}", self.failed_requests);
        info!("成功率: {:.2}%", self.success_rate());
        info!("错误率: {:.2}%", self.error_rate());
        info!("==================");
    }
}

/// 并发测试工具
pub struct ConcurrencyTestUtils;

impl ConcurrencyTestUtils {
    /// 执行并发操作并收集结果
    pub async fn run_concurrent_operations<F, Fut, T>(
        operation_count: usize,
        concurrency: usize,
        mut operation: F,
    ) -> Result<Vec<Result<T>>>
    where
        F: FnMut(usize) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
        T: Send + 'static,
    {
        use futures::stream::{FuturesUnordered, StreamExt};
        
        let mut futures = FuturesUnordered::new();
        let mut results = Vec::with_capacity(operation_count);
        
        // 启动初始并发任务
        for i in 0..concurrency.min(operation_count) {
            futures.push(Box::pin(operation(i)));
        }
        
        let mut next_operation = concurrency;
        
        // 处理完成的任务并启动新任务
        while let Some(result) = futures.next().await {
            results.push(result);
            
            // 如果还有操作待执行，启动新的任务
            if next_operation < operation_count {
                futures.push(Box::pin(operation(next_operation)));
                next_operation += 1;
            }
        }
        
        Ok(results)
    }

    /// 测量并发操作的性能
    pub async fn measure_concurrent_performance<F, Fut, T>(
        operation_name: &str,
        operation_count: usize,
        concurrency: usize,
        mut operation: F,
    ) -> Result<(Vec<T>, PerformanceReport)>
    where
        F: FnMut(usize) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
        T: Send + 'static,
    {
        info!("开始并发性能测试: {} ({}个操作, {}并发)", operation_name, operation_count, concurrency);
        
        let start_time = Instant::now();
        let mut durations = Vec::with_capacity(operation_count);
        
        let results = Self::run_concurrent_operations(
            operation_count,
            concurrency,
            |i| async move {
                let op_start = Instant::now();
                let result = operation(i).await;
                let duration = op_start.elapsed();
                (result, duration)
            }
        ).await?;
        
        let total_duration = start_time.elapsed();
        
        // 分离结果和时间统计
        let mut successful_results = Vec::new();
        let mut failed_count = 0;
        
        for result in results {
            match result {
                Ok((Ok(value), duration)) => {
                    successful_results.push(value);
                    durations.push(duration);
                }
                Ok((Err(_), duration)) => {
                    failed_count += 1;
                    durations.push(duration);
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }
        
        // 生成性能报告
        let report = PerformanceMetrics::generate_report(operation_name, &durations);
        
        info!("并发测试完成: 总耗时 {:?}, 成功 {}, 失败 {}", 
              total_duration, successful_results.len(), failed_count);
        
        if failed_count > 0 {
            warn!("有 {} 个操作失败", failed_count);
        }
        
        Ok((successful_results, report))
    }
}

/// 测试断言扩展
pub struct TestAssertions;

impl TestAssertions {
    /// 断言响应时间在要求范围内
    pub fn assert_response_time_within(duration: Duration, max_duration: Duration) -> Result<()> {
        if duration > max_duration {
            return Err(anyhow::anyhow!(
                "响应时间 {:?} 超过最大允许值 {:?}",
                duration, max_duration
            ));
        }
        Ok(())
    }

    /// 断言成功率高于指定值
    pub fn assert_success_rate_above(success_count: usize, total_count: usize, min_rate: f64) -> Result<()> {
        let actual_rate = (success_count as f64) / (total_count as f64) * 100.0;
        if actual_rate < min_rate {
            return Err(anyhow::anyhow!(
                "成功率 {:.2}% 低于要求 {:.2}%",
                actual_rate, min_rate
            ));
        }
        Ok(())
    }

    /// 断言值在指定范围内
    pub fn assert_within_range<T: PartialOrd + std::fmt::Debug>(
        value: T, 
        min: T, 
        max: T, 
        name: &str
    ) -> Result<()> {
        if value < min || value > max {
            return Err(anyhow::anyhow!(
                "{} 值 {:?} 不在允许范围 [{:?}, {:?}] 内",
                name, value, min, max
            ));
        }
        Ok(())
    }
}