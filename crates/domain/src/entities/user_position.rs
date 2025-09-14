//! 用户职位关系实体定义
//!
//! 包含用户与职位关系的核心信息和相关操作。

use crate::errors::{DomainError, DomainResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 用户职位关系实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPosition {
    /// 用户ID
    pub user_id: Uuid,
    /// 职位ID
    pub position_id: Uuid,
    /// 部门ID
    pub department_id: Uuid,
    /// 是否为主要职位
    pub is_primary: bool,
    /// 开始日期
    pub start_date: DateTime<Utc>,
    /// 结束日期（None表示仍在职）
    pub end_date: Option<DateTime<Utc>>,
}

impl UserPosition {
    /// 创建新的用户职位关系
    pub fn new(user_id: Uuid, position_id: Uuid, department_id: Uuid, is_primary: bool) -> Self {
        Self {
            user_id,
            position_id,
            department_id,
            is_primary,
            start_date: Utc::now(),
            end_date: None,
        }
    }

    /// 创建具有指定时间的用户职位关系（用于从数据库加载）
    pub fn with_dates(
        user_id: Uuid,
        position_id: Uuid,
        department_id: Uuid,
        is_primary: bool,
        start_date: DateTime<Utc>,
        end_date: Option<DateTime<Utc>>,
    ) -> DomainResult<Self> {
        // 验证日期逻辑
        if let Some(end_date) = end_date {
            if end_date <= start_date {
                return Err(DomainError::validation_error(
                    "end_date",
                    "结束日期必须晚于开始日期",
                ));
            }
        }

        Ok(Self {
            user_id,
            position_id,
            department_id,
            is_primary,
            start_date,
            end_date,
        })
    }

    /// 设置为主要职位
    pub fn set_primary(&mut self) {
        self.is_primary = true;
    }

    /// 设置为非主要职位
    pub fn set_secondary(&mut self) {
        self.is_primary = false;
    }

    /// 结束职位任期
    pub fn end_position(&mut self) -> DomainResult<()> {
        if self.is_ended() {
            return Err(DomainError::business_rule_violation("职位关系已结束"));
        }

        let now = Utc::now();
        if now <= self.start_date {
            return Err(DomainError::business_rule_violation(
                "不能在开始日期之前结束职位",
            ));
        }

        self.end_date = Some(now);
        Ok(())
    }

    /// 结束职位任期（指定结束日期）
    pub fn end_position_at(&mut self, end_date: DateTime<Utc>) -> DomainResult<()> {
        if self.is_ended() {
            return Err(DomainError::business_rule_violation("职位关系已结束"));
        }

        if end_date <= self.start_date {
            return Err(DomainError::validation_error(
                "end_date",
                "结束日期必须晚于开始日期",
            ));
        }

        self.end_date = Some(end_date);
        Ok(())
    }

    /// 重新激活职位（取消结束日期）
    pub fn reactivate(&mut self) -> DomainResult<()> {
        if !self.is_ended() {
            return Err(DomainError::business_rule_violation("职位关系尚未结束"));
        }

        self.end_date = None;
        Ok(())
    }

    /// 检查职位关系是否已结束
    pub fn is_ended(&self) -> bool {
        self.end_date.is_some()
    }

    /// 检查职位关系是否仍然有效
    pub fn is_active(&self) -> bool {
        match self.end_date {
            Some(end_date) => Utc::now() < end_date,
            None => true,
        }
    }

    /// 检查是否为主要职位
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// 检查职位关系是否在指定时间范围内有效
    pub fn is_active_at(&self, date: DateTime<Utc>) -> bool {
        date >= self.start_date && self.end_date.is_none_or(|end| date < end)
    }

    /// 获取任职期长度（天数）
    pub fn duration_days(&self) -> i64 {
        let end_time = self.end_date.unwrap_or_else(Utc::now);
        (end_time - self.start_date).num_days()
    }

    /// 检查两个职位关系是否有时间冲突（对于同一用户的不同主要职位）
    pub fn conflicts_with(&self, other: &UserPosition) -> bool {
        // 只检查同一用户的主要职位
        if self.user_id != other.user_id || !self.is_primary || !other.is_primary {
            return false;
        }

        // 检查时间段是否重叠
        let self_end = self.end_date.unwrap_or(DateTime::<Utc>::MAX_UTC);
        let other_end = other.end_date.unwrap_or(DateTime::<Utc>::MAX_UTC);

        self.start_date < other_end && other.start_date < self_end
    }

    /// 更新部门（职位转移）
    pub fn transfer_to_department(&mut self, new_department_id: Uuid) -> DomainResult<()> {
        if self.is_ended() {
            return Err(DomainError::business_rule_violation(
                "已结束的职位关系不能转移部门",
            ));
        }

        self.department_id = new_department_id;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_user_position_creation() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let user_position = UserPosition::new(user_id, position_id, department_id, true);

        assert_eq!(user_position.user_id, user_id);
        assert_eq!(user_position.position_id, position_id);
        assert_eq!(user_position.department_id, department_id);
        assert!(user_position.is_primary());
        assert!(user_position.is_active());
        assert!(!user_position.is_ended());
    }

    #[test]
    fn test_user_position_with_invalid_dates() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let start_date = Utc::now();
        let end_date = start_date - Duration::days(1); // 结束日期早于开始日期

        let result = UserPosition::with_dates(
            user_id,
            position_id,
            department_id,
            true,
            start_date,
            Some(end_date),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_set_primary_and_secondary() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let mut user_position = UserPosition::new(user_id, position_id, department_id, false);

        assert!(!user_position.is_primary());

        user_position.set_primary();
        assert!(user_position.is_primary());

        user_position.set_secondary();
        assert!(!user_position.is_primary());
    }

    #[test]
    fn test_end_position() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let mut user_position = UserPosition::new(user_id, position_id, department_id, true);

        // 等待一段时间确保开始日期在结束日期之前
        std::thread::sleep(std::time::Duration::from_millis(1));

        assert!(user_position.is_active());
        assert!(!user_position.is_ended());

        // 结束职位
        assert!(user_position.end_position().is_ok());
        assert!(user_position.is_ended());
        assert!(!user_position.is_active());

        // 不能重复结束
        assert!(user_position.end_position().is_err());
    }

    #[test]
    fn test_end_position_at() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let mut user_position = UserPosition::new(user_id, position_id, department_id, true);

        let future_date = Utc::now() + Duration::days(30);
        let past_date = user_position.start_date - Duration::days(1);

        // 指定未来日期结束
        assert!(user_position.end_position_at(future_date).is_ok());
        assert_eq!(user_position.end_date, Some(future_date));

        // 重新激活
        assert!(user_position.reactivate().is_ok());
        assert!(!user_position.is_ended());

        // 不能指定过去日期结束
        assert!(user_position.end_position_at(past_date).is_err());
    }

    #[test]
    fn test_is_active_at() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let start_date = Utc::now() - Duration::days(30);
        let end_date = Utc::now() + Duration::days(30);

        let user_position = UserPosition::with_dates(
            user_id,
            position_id,
            department_id,
            true,
            start_date,
            Some(end_date),
        )
        .unwrap();

        // 开始日期之前
        assert!(!user_position.is_active_at(start_date - Duration::days(1)));

        // 任职期间
        assert!(user_position.is_active_at(start_date + Duration::days(1)));
        assert!(user_position.is_active_at(Utc::now()));

        // 结束日期之后
        assert!(!user_position.is_active_at(end_date + Duration::days(1)));
    }

    #[test]
    fn test_duration_days() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let start_date = Utc::now() - Duration::days(100);
        let end_date = Utc::now() - Duration::days(30);

        let user_position = UserPosition::with_dates(
            user_id,
            position_id,
            department_id,
            true,
            start_date,
            Some(end_date),
        )
        .unwrap();

        assert_eq!(user_position.duration_days(), 70);

        // 未结束的职位
        let ongoing_position =
            UserPosition::with_dates(user_id, position_id, department_id, true, start_date, None)
                .unwrap();

        assert!(ongoing_position.duration_days() >= 100);
    }

    #[test]
    fn test_conflicts_with() {
        let user_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let start_date1 = Utc::now() - Duration::days(100);
        let end_date1 = Utc::now() - Duration::days(50);

        let start_date2 = Utc::now() - Duration::days(75);
        let end_date2 = Utc::now() - Duration::days(25);

        let position1 = UserPosition::with_dates(
            user_id,
            position_id,
            department_id,
            true, // 主要职位
            start_date1,
            Some(end_date1),
        )
        .unwrap();

        let position2 = UserPosition::with_dates(
            user_id,
            position_id,
            department_id,
            true, // 主要职位
            start_date2,
            Some(end_date2),
        )
        .unwrap();

        let position3 = UserPosition::with_dates(
            user_id,
            position_id,
            department_id,
            false, // 非主要职位
            start_date2,
            Some(end_date2),
        )
        .unwrap();

        let position4 = UserPosition::with_dates(
            other_user_id, // 不同用户
            position_id,
            department_id,
            true,
            start_date2,
            Some(end_date2),
        )
        .unwrap();

        // 同一用户的主要职位有时间冲突
        assert!(position1.conflicts_with(&position2));

        // 非主要职位不冲突
        assert!(!position1.conflicts_with(&position3));

        // 不同用户不冲突
        assert!(!position1.conflicts_with(&position4));
    }

    #[test]
    fn test_transfer_department() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();
        let new_department_id = Uuid::new_v4();

        let mut user_position = UserPosition::new(user_id, position_id, department_id, true);

        // 转移部门
        assert!(user_position
            .transfer_to_department(new_department_id)
            .is_ok());
        assert_eq!(user_position.department_id, new_department_id);

        // 等待一段时间确保开始日期在结束日期之前
        std::thread::sleep(std::time::Duration::from_millis(1));

        // 结束职位后不能转移
        assert!(user_position.end_position().is_ok());
        assert!(user_position.transfer_to_department(department_id).is_err());
    }

    #[test]
    fn test_reactivate() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let mut user_position = UserPosition::new(user_id, position_id, department_id, true);

        // 未结束的职位不能重新激活
        assert!(user_position.reactivate().is_err());

        // 结束后重新激活
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(user_position.end_position().is_ok());
        assert!(user_position.reactivate().is_ok());
        assert!(!user_position.is_ended());
        assert!(user_position.is_active());
    }

    #[test]
    fn test_user_position_serialization() {
        let user_id = Uuid::new_v4();
        let position_id = Uuid::new_v4();
        let department_id = Uuid::new_v4();

        let user_position = UserPosition::new(user_id, position_id, department_id, true);

        // 测试序列化
        let json = serde_json::to_string(&user_position).unwrap();
        assert!(!json.is_empty());

        // 测试反序列化
        let deserialized: UserPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(user_position, deserialized);
    }
}
