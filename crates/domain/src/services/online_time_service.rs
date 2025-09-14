//! 在线时长统计服务接口定义
//!
//! 提供用户在线时长统计相关的业务逻辑服务接口，包括会话管理、心跳更新、统计查询等

use crate::entities::online_statistics::{DailyOnlineStatistics, UserSession};
use crate::errors::DomainResult;
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 在线时长统计服务接口
#[async_trait]
pub trait OnlineTimeService: Send + Sync {
    /// 开始用户会话
    async fn start_session(
        &self,
        user_id: Uuid,
        session_id: String,
        server_instance: String,
    ) -> DomainResult<()>;

    /// 更新心跳
    async fn update_heartbeat(&self, session_id: String) -> DomainResult<()>;

    /// 结束会话
    async fn end_session(&self, session_id: String, disconnect_reason: String) -> DomainResult<()>;

    /// 获取用户统计信息
    async fn get_user_statistics(
        &self,
        user_id: Uuid,
        date_range: DateRange,
    ) -> DomainResult<Vec<DailyOnlineStatistics>>;

    /// 获取活跃会话
    async fn get_active_sessions(&self, user_id: Uuid) -> DomainResult<Vec<UserSession>>;

    /// 获取用户总在线时长
    async fn get_total_online_time(
        &self,
        user_id: Uuid,
        date_range: DateRange,
    ) -> DomainResult<i64>;
}

/// 日期范围
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DateRange {
    /// 开始日期
    pub start_date: NaiveDate,
    /// 结束日期
    pub end_date: NaiveDate,
}

impl DateRange {
    /// 创建新的日期范围
    pub fn new(start_date: NaiveDate, end_date: NaiveDate) -> DomainResult<Self> {
        if end_date < start_date {
            return Err(crate::errors::DomainError::validation_error(
                "date_range",
                "结束日期不能早于开始日期",
            ));
        }

        Ok(Self {
            start_date,
            end_date,
        })
    }

    /// 创建今天的日期范围
    pub fn today() -> Self {
        let today = chrono::Utc::now().date_naive();
        Self {
            start_date: today,
            end_date: today,
        }
    }

    /// 创建本周的日期范围
    pub fn this_week() -> Self {
        let today = chrono::Utc::now().date_naive();
        let days_since_monday = today.weekday().num_days_from_monday();
        let monday = today - chrono::Duration::days(days_since_monday as i64);
        let sunday = monday + chrono::Duration::days(6);

        Self {
            start_date: monday,
            end_date: sunday,
        }
    }

    /// 创建本月的日期范围
    pub fn this_month() -> Self {
        let today = chrono::Utc::now().date_naive();
        let first_day =
            NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("有效的月份第一天");
        let last_day = if today.month() == 12 {
            NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).expect("有效的下个月第一天")
                - chrono::Duration::days(1)
        } else {
            NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).expect("有效的下个月第一天")
                - chrono::Duration::days(1)
        };

        Self {
            start_date: first_day,
            end_date: last_day,
        }
    }

    /// 创建最近N天的日期范围
    pub fn last_n_days(n: u32) -> Self {
        let today = chrono::Utc::now().date_naive();
        let start_date = today - chrono::Duration::days((n - 1) as i64);

        Self {
            start_date,
            end_date: today,
        }
    }

    /// 检查日期是否在范围内
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start_date && date <= self.end_date
    }

    /// 计算日期范围的天数
    pub fn days(&self) -> i64 {
        (self.end_date - self.start_date).num_days() + 1
    }

    /// 获取日期范围内的所有日期
    pub fn dates(&self) -> Vec<NaiveDate> {
        let mut dates = Vec::new();
        let mut current = self.start_date;

        while current <= self.end_date {
            dates.push(current);
            current += chrono::Duration::days(1);
        }

        dates
    }

    /// 验证日期范围的合理性
    pub fn validate(&self) -> DomainResult<()> {
        if self.end_date < self.start_date {
            return Err(crate::errors::DomainError::validation_error(
                "date_range",
                "结束日期不能早于开始日期",
            ));
        }

        // 限制查询范围不超过1年
        let max_days = 365;
        if self.days() > max_days {
            return Err(crate::errors::DomainError::validation_error(
                "date_range",
                format!("日期范围不能超过{}天", max_days),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_date_range_creation() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();

        let range = DateRange::new(start, end).unwrap();
        assert_eq!(range.start_date, start);
        assert_eq!(range.end_date, end);
    }

    #[test]
    fn test_date_range_invalid_creation() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

        let result = DateRange::new(start, end);
        assert!(result.is_err());
    }

    #[test]
    fn test_date_range_today() {
        let range = DateRange::today();
        let today = chrono::Utc::now().date_naive();

        assert_eq!(range.start_date, today);
        assert_eq!(range.end_date, today);
        assert_eq!(range.days(), 1);
    }

    #[test]
    fn test_date_range_this_week() {
        let range = DateRange::this_week();
        assert_eq!(range.days(), 7);

        // 验证开始日期是周一
        assert_eq!(range.start_date.weekday().num_days_from_monday(), 0);

        // 验证结束日期是周日
        assert_eq!(range.end_date.weekday().num_days_from_monday(), 6);
    }

    #[test]
    fn test_date_range_this_month() {
        let range = DateRange::this_month();
        let today = chrono::Utc::now().date_naive();

        // 验证开始日期是月初
        assert_eq!(range.start_date.day(), 1);
        assert_eq!(range.start_date.month(), today.month());
        assert_eq!(range.start_date.year(), today.year());

        // 验证结束日期是月末
        assert_eq!(range.end_date.month(), today.month());
        assert_eq!(range.end_date.year(), today.year());
    }

    #[test]
    fn test_date_range_last_n_days() {
        let range = DateRange::last_n_days(7);
        let today = chrono::Utc::now().date_naive();

        assert_eq!(range.end_date, today);
        assert_eq!(range.days(), 7);
        assert_eq!(range.start_date, today - chrono::Duration::days(6));
    }

    #[test]
    fn test_date_range_contains() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        let range = DateRange::new(start, end).unwrap();

        // 范围内的日期
        assert!(range.contains(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
        assert!(range.contains(start));
        assert!(range.contains(end));

        // 范围外的日期
        assert!(!range.contains(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()));
        assert!(!range.contains(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap()));
    }

    #[test]
    fn test_date_range_days() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        let range = DateRange::new(start, end).unwrap();

        assert_eq!(range.days(), 31);

        // 单日范围
        let single_day_range = DateRange::new(start, start).unwrap();
        assert_eq!(single_day_range.days(), 1);
    }

    #[test]
    fn test_date_range_dates() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 3).unwrap();
        let range = DateRange::new(start, end).unwrap();

        let dates = range.dates();
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[0], start);
        assert_eq!(dates[1], NaiveDate::from_ymd_opt(2024, 1, 2).unwrap());
        assert_eq!(dates[2], end);
    }

    #[test]
    fn test_date_range_validation() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        let valid_range = DateRange::new(start, end).unwrap();

        // 有效范围
        assert!(valid_range.validate().is_ok());

        // 超过一年的范围
        let too_long_end = start + chrono::Duration::days(400);
        let invalid_range = DateRange::new(start, too_long_end).unwrap();
        assert!(invalid_range.validate().is_err());

        // 倒序日期
        let reversed_range = DateRange {
            start_date: end,
            end_date: start,
        };
        assert!(reversed_range.validate().is_err());
    }

    #[test]
    fn test_date_range_serialization() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        let range = DateRange::new(start, end).unwrap();

        // 测试序列化
        let json = serde_json::to_string(&range).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: DateRange = serde_json::from_str(&json).unwrap();
        assert_eq!(range, deserialized);
    }
}
