-- 消息传递追踪表
-- 用于追踪消息的发送和送达状态
CREATE TABLE IF NOT EXISTS message_deliveries (
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ,

    PRIMARY KEY (message_id, user_id)
);

-- 优化查询性能的索引
CREATE INDEX IF NOT EXISTS idx_message_deliveries_user_undelivered
ON message_deliveries(user_id) WHERE delivered_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_message_deliveries_cleanup
ON message_deliveries(delivered_at) WHERE delivered_at IS NOT NULL;