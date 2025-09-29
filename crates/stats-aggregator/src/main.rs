use anyhow::Result;
use chrono::{Duration, Utc};
use config::AppConfig;
use infrastructure::repository::create_pg_pool;
use infrastructure::stats_aggregation::{StatsAggregationService, TimeGranularity};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::signal;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};

/// 统计聚合服务
///
/// 负责定期从 presence_events 表计算聚合统计，
/// 并将结果保存到 stats_aggregated 表供报表查询使用。
pub struct StatsAggregator {
    aggregation_service: StatsAggregationService,
}

impl StatsAggregator {
    /// 创建新的统计聚合服务
    pub fn new(pool: PgPool) -> Self {
        let aggregation_service = StatsAggregationService::new(pool);
        Self {
            aggregation_service,
        }
    }

    /// 执行小时级统计聚合
    pub async fn aggregate_hourly_stats(&self) -> Result<()> {
        info!("开始执行小时级统计聚合");

        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(1);

        let count = self
            .aggregation_service
            .run_aggregation_pipeline(TimeGranularity::Hour, start_time, end_time)
            .await?;

        info!("小时级统计聚合完成，处理了 {} 条记录", count);
        Ok(())
    }

    /// 执行日级统计聚合
    pub async fn aggregate_daily_stats(&self) -> Result<()> {
        info!("开始执行日级统计聚合");

        let end_time = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let start_time = end_time - Duration::days(1);

        let count = self
            .aggregation_service
            .run_aggregation_pipeline(TimeGranularity::Day, start_time, end_time)
            .await?;

        info!("日级统计聚合完成，处理了 {} 条记录", count);
        Ok(())
    }

    /// 执行周级统计聚合
    pub async fn aggregate_weekly_stats(&self) -> Result<()> {
        info!("开始执行周级统计聚合");

        let end_time = Utc::now();
        let start_time = end_time - Duration::weeks(1);

        let count = self
            .aggregation_service
            .run_aggregation_pipeline(TimeGranularity::Week, start_time, end_time)
            .await?;

        info!("周级统计聚合完成，处理了 {} 条记录", count);
        Ok(())
    }

    /// 执行月级统计聚合
    pub async fn aggregate_monthly_stats(&self) -> Result<()> {
        info!("开始执行月级统计聚合");

        let end_time = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let start_time = end_time - Duration::days(30); // 近似30天

        let count = self
            .aggregation_service
            .run_aggregation_pipeline(TimeGranularity::Month, start_time, end_time)
            .await?;

        info!("月级统计聚合完成，处理了 {} 条记录", count);
        Ok(())
    }

    /// 执行数据清理和分区管理
    pub async fn cleanup_expired_data(&self) -> Result<()> {
        info!("开始数据清理和分区管理");

        // 1. 创建下个月的分区（预创建）
        let next_month = Utc::now() + Duration::days(30);
        match self
            .aggregation_service
            .create_partition_if_not_exists(next_month)
            .await
        {
            Ok(message) => info!("分区管理: {}", message),
            Err(e) => error!("创建分区失败: {}", e),
        }

        // 2. 清理过期的原始事件分区
        match self.aggregation_service.cleanup_expired_partitions().await {
            Ok(deleted_partitions) => {
                let total_rows: i64 = deleted_partitions.iter().map(|(_, count)| count).sum();
                info!(
                    "清理过期分区完成，删除了 {} 个分区表，共 {} 条记录",
                    deleted_partitions.len(),
                    total_rows
                );
            }
            Err(e) => error!("清理过期分区失败: {}", e),
        }

        // 3. 清理过期聚合数据
        match self
            .aggregation_service
            .cleanup_expired_aggregated_data()
            .await
        {
            Ok(deleted_count) => {
                info!("清理过期聚合数据完成，删除了 {} 条记录", deleted_count);
            }
            Err(e) => error!("清理过期聚合数据失败: {}", e),
        }

        info!("数据清理和分区管理完成");
        Ok(())
    }

    /// 运行统计聚合服务主循环
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("启动统计聚合服务");

        // 创建定时任务调度器
        let mut scheduler = JobScheduler::new().await?;

        // 小时级统计 - 每小时第5分钟执行
        let aggregator = Arc::new(self);
        let hourly_aggregator = aggregator.clone();
        scheduler
            .add(Job::new_async("0 5 * * * *", move |_uuid, _l| {
                let agg = hourly_aggregator.clone();
                Box::pin(async move {
                    if let Err(e) = agg.aggregate_hourly_stats().await {
                        error!("小时级统计聚合失败: {}", e);
                    }
                })
            })?)
            .await?;

        // 日级统计 - 每天凌晨1点执行
        let daily_aggregator = aggregator.clone();
        scheduler
            .add(Job::new_async("0 0 1 * * *", move |_uuid, _l| {
                let agg = daily_aggregator.clone();
                Box::pin(async move {
                    if let Err(e) = agg.aggregate_daily_stats().await {
                        error!("日级统计聚合失败: {}", e);
                    }
                })
            })?)
            .await?;

        // 周级统计 - 每周一凌晨2点执行
        let weekly_aggregator = aggregator.clone();
        scheduler
            .add(Job::new_async("0 0 2 * * 1", move |_uuid, _l| {
                let agg = weekly_aggregator.clone();
                Box::pin(async move {
                    if let Err(e) = agg.aggregate_weekly_stats().await {
                        error!("周级统计聚合失败: {}", e);
                    }
                })
            })?)
            .await?;

        // 月级统计 - 每月1号凌晨3点执行
        let monthly_aggregator = aggregator.clone();
        scheduler
            .add(Job::new_async("0 0 3 1 * *", move |_uuid, _l| {
                let agg = monthly_aggregator.clone();
                Box::pin(async move {
                    if let Err(e) = agg.aggregate_monthly_stats().await {
                        error!("月级统计聚合失败: {}", e);
                    }
                })
            })?)
            .await?;

        // 数据清理 - 每天凌晨4点执行
        let cleanup_aggregator = aggregator.clone();
        scheduler
            .add(Job::new_async("0 0 4 * * *", move |_uuid, _l| {
                let agg = cleanup_aggregator.clone();
                Box::pin(async move {
                    if let Err(e) = agg.cleanup_expired_data().await {
                        error!("数据清理失败: {}", e);
                    }
                })
            })?)
            .await?;

        info!("定时任务已设置完成");
        info!("小时级统计: 每小时第5分钟执行");
        info!("日级统计: 每天凌晨1点执行");
        info!("周级统计: 每周一凌晨2点执行");
        info!("月级统计: 每月1号凌晨3点执行");
        info!("数据清理: 每天凌晨4点执行");

        // 启动调度器
        scheduler.start().await?;

        // 等待关闭信号
        let mut sig_term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("接收到 Ctrl+C 信号，开始优雅停机...");
            }
            _ = sig_term.recv() => {
                info!("接收到终止信号，开始优雅停机...");
            }
        }

        // 停止调度器
        scheduler.shutdown().await?;
        info!("统计聚合服务已停止");

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("启动统计聚合服务");

    // 加载配置
    let config = AppConfig::from_env_with_defaults();

    // 创建数据库连接池
    let db_pool = create_pg_pool(&config.database.url, config.database.max_connections).await?;

    // 创建统计聚合服务
    let aggregator = StatsAggregator::new(db_pool);

    // 运行主循环
    Arc::new(aggregator).run().await?;

    Ok(())
}
