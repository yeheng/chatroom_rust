use crate::value_objects::{MessageId, Timestamp, UserId};

/// 消息传递状态追踪
/// 对应数据库表：message_deliveries
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MessageDelivery {
    pub message_id: MessageId,
    pub user_id: UserId,
    pub sent_at: Timestamp,        // 消息发送时间
    pub delivered_at: Option<Timestamp>, // 消息送达确认时间
}

impl MessageDelivery {
    /// 创建新的消息传递记录（发送状态）
    pub fn new_sent(message_id: MessageId, user_id: UserId, sent_at: Timestamp) -> Self {
        Self {
            message_id,
            user_id,
            sent_at,
            delivered_at: None,
        }
    }

    /// 标记消息已送达
    pub fn mark_delivered(&mut self, delivered_at: Timestamp) {
        self.delivered_at = Some(delivered_at);
    }

    /// 检查消息是否已送达
    pub fn is_delivered(&self) -> bool {
        self.delivered_at.is_some()
    }

    /// 获取传递延迟（如果已送达）
    pub fn delivery_delay(&self) -> Option<std::time::Duration> {
        self.delivered_at.map(|delivered| {
            let sent_timestamp = self.sent_at.unix_timestamp_nanos() as u64;
            let delivered_timestamp = delivered.unix_timestamp_nanos() as u64;

            if delivered_timestamp >= sent_timestamp {
                std::time::Duration::from_nanos(delivered_timestamp - sent_timestamp)
            } else {
                // 时间倒流的异常情况，返回零延迟
                std::time::Duration::ZERO
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    #[test]
    fn test_message_delivery_lifecycle() {
        use uuid::Uuid;

        let message_id = MessageId::from(Uuid::new_v4());
        let user_id = UserId::from(Uuid::new_v4());
        let sent_time = OffsetDateTime::now_utc();

        // 创建发送记录
        let mut delivery = MessageDelivery::new_sent(message_id, user_id, sent_time);
        assert!(!delivery.is_delivered());
        assert!(delivery.delivery_delay().is_none());

        // 标记已送达
        let delivered_time = sent_time + time::Duration::milliseconds(100);
        delivery.mark_delivered(delivered_time);
        assert!(delivery.is_delivered());
        assert!(delivery.delivery_delay().is_some());

        // 检查延迟计算
        let delay = delivery.delivery_delay().unwrap();
        assert!(delay.as_millis() >= 100);
    }
}