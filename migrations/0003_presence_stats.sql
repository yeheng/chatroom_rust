-- 在线统计功能 - 时序数据存储和聚合表结构
-- 基于文档 @docs/features/用户在线统计.md 的设计

-- 创建事件类型枚举
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'presence_event_type') THEN
        CREATE TYPE presence_event_type AS ENUM ('Connected', 'Disconnected', 'Heartbeat');
    END IF;
END$$;

-- 时间粒度枚举
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'time_granularity') THEN
        CREATE TYPE time_granularity AS ENUM ('Hour', 'Day', 'Week', 'Month', 'Year');
    END IF;
END$$;

-- 用户状态变化事件表（原始时序数据）
-- 分区表，按时间分区，支持高频写入
CREATE TABLE IF NOT EXISTS presence_events (
    event_id UUID NOT NULL DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    room_id UUID NOT NULL REFERENCES chat_rooms(id) ON DELETE CASCADE,
    event_type presence_event_type NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    session_id UUID NOT NULL,  -- 用于计算在线时长
    user_ip INET,              -- 用户IP地址（可选）
    user_agent TEXT,           -- 用户代理（可选）
    PRIMARY KEY (event_id, timestamp)
) PARTITION BY RANGE (timestamp);

-- 为原始事件表创建索引
CREATE INDEX IF NOT EXISTS idx_presence_events_room_time ON presence_events (room_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_presence_events_user_session ON presence_events (user_id, session_id);
CREATE INDEX IF NOT EXISTS idx_presence_events_timestamp ON presence_events (timestamp);
CREATE INDEX IF NOT EXISTS idx_presence_events_event_type ON presence_events (event_type);

-- 创建当前月份的分区表
DO $$
DECLARE
    current_month_start DATE;
    next_month_start DATE;
    partition_name TEXT;
BEGIN
    current_month_start := date_trunc('month', CURRENT_DATE);
    next_month_start := current_month_start + INTERVAL '1 month';
    partition_name := 'presence_events_' || to_char(current_month_start, 'YYYY_MM');

    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF presence_events
                    FOR VALUES FROM (%L) TO (%L)',
                   partition_name, current_month_start, next_month_start);

    -- 为分区表创建特定索引
    EXECUTE format('CREATE INDEX IF NOT EXISTS %I ON %I (room_id, timestamp)',
                   'idx_' || partition_name || '_room_time', partition_name);
END$$;

-- 聚合统计表（预计算的统计数据）
CREATE TABLE IF NOT EXISTS stats_aggregated (
    room_id UUID NOT NULL REFERENCES chat_rooms(id) ON DELETE CASCADE,
    time_bucket TIMESTAMPTZ NOT NULL,          -- 时间桶（小时/日/月/年的开始时间）
    granularity time_granularity NOT NULL,     -- 时间粒度
    peak_online_count BIGINT NOT NULL,         -- 峰值在线人数
    avg_online_count DOUBLE PRECISION NOT NULL, -- 平均在线人数
    total_connections BIGINT NOT NULL,         -- 总连接数
    unique_users BIGINT NOT NULL,              -- 去重用户数
    avg_session_duration DOUBLE PRECISION NOT NULL, -- 平均会话时长（秒）
    PRIMARY KEY (room_id, time_bucket, granularity)
);

-- 聚合统计表索引
CREATE INDEX IF NOT EXISTS idx_stats_aggregated_time_gran ON stats_aggregated (time_bucket, granularity);
CREATE INDEX IF NOT EXISTS idx_stats_aggregated_room_gran ON stats_aggregated (room_id, granularity);

-- 数据生命周期管理表
CREATE TABLE IF NOT EXISTS stats_data_retention (
    granularity time_granularity PRIMARY KEY,
    retention_days INTEGER NOT NULL,
    last_cleanup TIMESTAMPTZ,
    CONSTRAINT valid_retention CHECK (retention_days > 0 OR retention_days = -1)
);

-- 插入默认的数据保留策略
INSERT INTO stats_data_retention (granularity, retention_days) VALUES
    ('Hour', 30),     -- 小时数据保留30天
    ('Day', 365),     -- 日数据保留1年
    ('Week', 730),    -- 周数据保留2年
    ('Month', 1825),  -- 月数据保留5年
    ('Year', -1)      -- 年数据永久保留（-1表示永不删除）
ON CONFLICT (granularity) DO NOTHING;

-- 用于存储系统配置的表
CREATE TABLE IF NOT EXISTS stats_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 插入默认配置
INSERT INTO stats_config (key, value, description) VALUES
    ('event_batch_size', '1000', '事件批量插入大小'),
    ('aggregation_delay_minutes', '5', '聚合任务延迟分钟数'),
    ('cache_ttl_seconds', '300', '统计缓存TTL（秒）'),
    ('enable_ip_tracking', 'false', '是否启用IP跟踪'),
    ('enable_user_agent_tracking', 'false', '是否启用User-Agent跟踪')
ON CONFLICT (key) DO NOTHING;

-- 创建触发器用于自动更新配置表的updated_at字段
CREATE OR REPLACE FUNCTION update_stats_config_timestamp()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

CREATE TRIGGER trigger_update_stats_config_timestamp
    BEFORE UPDATE ON stats_config
    FOR EACH ROW
    EXECUTE FUNCTION update_stats_config_timestamp();

-- 创建视图，便于查询最新的统计信息
CREATE OR REPLACE VIEW latest_room_stats AS
SELECT
    room_id,
    granularity,
    time_bucket,
    peak_online_count,
    avg_online_count,
    total_connections,
    unique_users,
    avg_session_duration
FROM stats_aggregated s1
WHERE s1.time_bucket = (
    SELECT MAX(s2.time_bucket)
    FROM stats_aggregated s2
    WHERE s2.room_id = s1.room_id
    AND s2.granularity = s1.granularity
);

-- 创建用于快速查询房间当前统计的视图
CREATE OR REPLACE VIEW current_room_stats AS
SELECT
    room_id,
    MAX(CASE WHEN granularity = 'Hour' THEN peak_online_count END) as current_hour_peak,
    MAX(CASE WHEN granularity = 'Day' THEN peak_online_count END) as current_day_peak,
    MAX(CASE WHEN granularity = 'Hour' THEN avg_online_count END) as current_hour_avg,
    MAX(CASE WHEN granularity = 'Day' THEN avg_online_count END) as current_day_avg
FROM latest_room_stats
WHERE granularity IN ('Hour', 'Day')
GROUP BY room_id;

-- 注释
COMMENT ON TABLE presence_events IS '用户状态变化事件原始数据表，按月分区存储，分区管理由应用层负责';
COMMENT ON TABLE stats_aggregated IS '预聚合的统计数据表，支持多种时间粒度';
COMMENT ON TABLE stats_data_retention IS '数据保留策略配置表';
COMMENT ON TABLE stats_config IS '统计系统配置表';