-- 统一维度表设计：一张表处理所有维度（房间、组织、用户等）
-- 假设原有 stats_aggregated 表只有 room_id，现在需要重构为多维度表

-- 1. 创建新的统一维度表
CREATE TABLE stats_aggregated_v2 (
    dimension_type TEXT NOT NULL,  -- 'room' | 'org' | 'user' | ...
    dimension_id UUID NOT NULL,
    time_bucket TIMESTAMPTZ NOT NULL,
    granularity time_granularity NOT NULL,
    peak_online_count BIGINT NOT NULL DEFAULT 0,
    avg_online_count DOUBLE PRECISION NOT NULL DEFAULT 0,
    total_connections BIGINT NOT NULL DEFAULT 0,
    unique_users BIGINT NOT NULL DEFAULT 0,
    avg_session_duration DOUBLE PRECISION NOT NULL DEFAULT 0,
    PRIMARY KEY (dimension_type, dimension_id, time_bucket, granularity)
);

-- 复合索引用于按维度和时间查询
CREATE INDEX idx_stats_dimension_time ON stats_aggregated_v2
    (dimension_type, time_bucket, granularity);

-- 单独的时间索引用于时间范围查询
CREATE INDEX idx_stats_time_bucket ON stats_aggregated_v2 (time_bucket);

COMMENT ON TABLE stats_aggregated_v2 IS '统一维度统计表,通过dimension_type区分不同维度';
COMMENT ON COLUMN stats_aggregated_v2.dimension_type IS '维度类型: room(房间) org(组织) user(用户)等';
COMMENT ON COLUMN stats_aggregated_v2.dimension_id IS '维度实体ID';

-- 2. 迁移现有数据（如果 stats_aggregated 表已存在）
INSERT INTO stats_aggregated_v2
    (dimension_type, dimension_id, time_bucket, granularity,
     peak_online_count, avg_online_count, total_connections, unique_users, avg_session_duration)
SELECT
    'room'::TEXT,
    room_id,
    time_bucket,
    granularity,
    peak_online_count,
    avg_online_count,
    total_connections,
    unique_users,
    avg_session_duration
FROM stats_aggregated
ON CONFLICT (dimension_type, dimension_id, time_bucket, granularity) DO NOTHING;

-- 3. 重命名表（在确认数据迁移无误后）
-- 注意：这个操作是破坏性的，在生产环境需要谨慎执行
-- DROP TABLE stats_aggregated;
-- ALTER TABLE stats_aggregated_v2 RENAME TO stats_aggregated;

-- 4. 更新视图以支持新的统一维度表
CREATE OR REPLACE VIEW latest_stats AS
SELECT
    dimension_type,
    dimension_id,
    granularity,
    time_bucket,
    peak_online_count,
    avg_online_count,
    total_connections,
    unique_users,
    avg_session_duration
FROM stats_aggregated_v2 s1
WHERE s1.time_bucket = (
    SELECT MAX(s2.time_bucket)
    FROM stats_aggregated_v2 s2
    WHERE s2.dimension_type = s1.dimension_type
    AND s2.dimension_id = s1.dimension_id
    AND s2.granularity = s1.granularity
);

-- 5. 创建用于快速查询当前统计的视图
CREATE OR REPLACE VIEW current_stats AS
SELECT
    dimension_type,
    dimension_id,
    MAX(CASE WHEN granularity = 'Hour' THEN peak_online_count END) as current_hour_peak,
    MAX(CASE WHEN granularity = 'Day' THEN peak_online_count END) as current_day_peak,
    MAX(CASE WHEN granularity = 'Hour' THEN avg_online_count END) as current_hour_avg,
    MAX(CASE WHEN granularity = 'Day' THEN avg_online_count END) as current_day_avg
FROM latest_stats
WHERE granularity IN ('Hour', 'Day')
GROUP BY dimension_type, dimension_id;