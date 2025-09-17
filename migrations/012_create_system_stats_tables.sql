-- 创建系统统计和监控表

-- 创建每日统计表
CREATE TABLE IF NOT EXISTS daily_stats (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    stat_date DATE NOT NULL,
    total_users INTEGER NOT NULL DEFAULT 0,
    active_users INTEGER NOT NULL DEFAULT 0, -- 当日活跃用户数
    new_users INTEGER NOT NULL DEFAULT 0, -- 当日新注册用户数
    total_rooms INTEGER NOT NULL DEFAULT 0,
    active_rooms INTEGER NOT NULL DEFAULT 0, -- 当日有活动的房间数
    new_rooms INTEGER NOT NULL DEFAULT 0, -- 当日新创建房间数
    total_messages INTEGER NOT NULL DEFAULT 0,
    new_messages INTEGER NOT NULL DEFAULT 0, -- 当日新消息数
    total_files INTEGER NOT NULL DEFAULT 0,
    new_files INTEGER NOT NULL DEFAULT 0, -- 当日新上传文件数
    storage_used_bytes BIGINT NOT NULL DEFAULT 0, -- 存储使用量
    avg_session_duration_minutes INTEGER DEFAULT 0, -- 平均会话时长（分钟）
    peak_concurrent_users INTEGER DEFAULT 0, -- 峰值并发用户数
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT daily_stats_unique UNIQUE (stat_date),
    CONSTRAINT daily_stats_positive_check CHECK (
        total_users >= 0 AND active_users >= 0 AND new_users >= 0 AND
        total_rooms >= 0 AND active_rooms >= 0 AND new_rooms >= 0 AND
        total_messages >= 0 AND new_messages >= 0 AND
        storage_used_bytes >= 0
    )
);

-- 创建系统指标表
CREATE TABLE IF NOT EXISTS system_metrics (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    metric_unit VARCHAR(20), -- 'percent', 'bytes', 'count', 'milliseconds'
    tags JSONB DEFAULT '{}', -- 附加标签信息
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    server_id VARCHAR(100), -- 服务器标识
    instance_id VARCHAR(100), -- 实例标识

    CONSTRAINT system_metrics_name_check CHECK (
        metric_name IN (
            'cpu_usage', 'memory_usage', 'disk_usage', 'network_io_in', 'network_io_out',
            'active_connections', 'message_rate', 'error_rate', 'response_time',
            'queue_depth', 'cache_hit_rate', 'db_connections', 'redis_connections'
        )
    ),
    CONSTRAINT system_metrics_unit_check CHECK (
        metric_unit IN ('percent', 'bytes', 'count', 'milliseconds', 'seconds', 'rate')
    )
);

-- 创建实时在线用户表
CREATE TABLE IF NOT EXISTS online_users (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    room_id UUID REFERENCES chat_rooms(id) ON DELETE SET NULL, -- 当前所在房间
    status VARCHAR(20) DEFAULT 'online', -- 'online', 'away', 'busy', 'invisible'
    last_seen_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    device_type VARCHAR(20) DEFAULT 'web', -- 'web', 'mobile', 'desktop'
    ip_address INET,
    user_agent TEXT,
    location_data JSONB DEFAULT '{}', -- 地理位置信息
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT online_users_status_check CHECK (
        status IN ('online', 'away', 'busy', 'invisible')
    ),
    CONSTRAINT online_users_device_check CHECK (
        device_type IN ('web', 'mobile', 'desktop', 'tablet', 'bot')
    )
);

-- 创建房间活动统计表
CREATE TABLE IF NOT EXISTS room_activity_stats (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    room_id UUID NOT NULL REFERENCES chat_rooms(id) ON DELETE CASCADE,
    stat_date DATE NOT NULL,
    message_count INTEGER DEFAULT 0,
    active_users_count INTEGER DEFAULT 0, -- 当日活跃用户数
    peak_concurrent_users INTEGER DEFAULT 0, -- 峰值在线用户数
    avg_response_time_seconds INTEGER DEFAULT 0, -- 平均响应时间
    file_uploads_count INTEGER DEFAULT 0,
    total_file_size_bytes BIGINT DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT room_activity_stats_unique UNIQUE (room_id, stat_date)
);

-- 创建错误日志表
CREATE TABLE IF NOT EXISTS error_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    error_type VARCHAR(50) NOT NULL, -- 'application', 'database', 'network', 'auth'
    error_code VARCHAR(50),
    error_message TEXT NOT NULL,
    stack_trace TEXT,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    request_path VARCHAR(500),
    request_method VARCHAR(10),
    request_params JSONB DEFAULT '{}',
    ip_address INET,
    user_agent TEXT,
    server_id VARCHAR(100),
    severity VARCHAR(20) DEFAULT 'error', -- 'debug', 'info', 'warning', 'error', 'critical'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT error_logs_type_check CHECK (
        error_type IN ('application', 'database', 'network', 'auth', 'validation', 'system')
    ),
    CONSTRAINT error_logs_severity_check CHECK (
        severity IN ('debug', 'info', 'warning', 'error', 'critical')
    )
);

-- 索引创建
-- 每日统计表索引
CREATE INDEX IF NOT EXISTS idx_daily_stats_date ON daily_stats(stat_date DESC);

-- 系统指标表索引
CREATE INDEX IF NOT EXISTS idx_system_metrics_name_time ON system_metrics(metric_name, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_system_metrics_server ON system_metrics(server_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_system_metrics_timestamp ON system_metrics(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_system_metrics_tags_gin ON system_metrics USING gin(tags);

-- 在线用户表索引
CREATE INDEX IF NOT EXISTS idx_online_users_session ON online_users(session_id);
CREATE INDEX IF NOT EXISTS idx_online_users_room ON online_users(room_id);
CREATE INDEX IF NOT EXISTS idx_online_users_status ON online_users(status);
CREATE INDEX IF NOT EXISTS idx_online_users_last_seen ON online_users(last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_online_users_device ON online_users(device_type);
CREATE INDEX IF NOT EXISTS idx_online_users_location_gin ON online_users USING gin(location_data);

-- 房间活动统计表索引
CREATE INDEX IF NOT EXISTS idx_room_activity_stats_room ON room_activity_stats(room_id);
CREATE INDEX IF NOT EXISTS idx_room_activity_stats_date ON room_activity_stats(stat_date DESC);
CREATE INDEX IF NOT EXISTS idx_room_activity_stats_room_date ON room_activity_stats(room_id, stat_date DESC);

-- 错误日志表索引
CREATE INDEX IF NOT EXISTS idx_error_logs_type ON error_logs(error_type);
CREATE INDEX IF NOT EXISTS idx_error_logs_severity ON error_logs(severity);
CREATE INDEX IF NOT EXISTS idx_error_logs_created_at ON error_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_error_logs_user ON error_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_error_logs_server ON error_logs(server_id);
CREATE INDEX IF NOT EXISTS idx_error_logs_params_gin ON error_logs USING gin(request_params);

-- 复合索引
CREATE INDEX IF NOT EXISTS idx_error_logs_type_severity ON error_logs(error_type, severity, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_system_metrics_name_server ON system_metrics(metric_name, server_id, timestamp DESC);

-- 创建系统健康状态视图
CREATE OR REPLACE VIEW system_health_status AS
WITH recent_metrics AS (
    SELECT
        metric_name,
        AVG(metric_value) as avg_value,
        MAX(metric_value) as max_value,
        MIN(metric_value) as min_value,
        COUNT(*) as sample_count
    FROM system_metrics
    WHERE timestamp > NOW() - INTERVAL '5 minutes'
    GROUP BY metric_name
),
recent_errors AS (
    SELECT
        COUNT(*) as error_count,
        COUNT(*) FILTER (WHERE severity IN ('error', 'critical')) as critical_error_count
    FROM error_logs
    WHERE created_at > NOW() - INTERVAL '5 minutes'
)
SELECT
    CASE
        WHEN rm.avg_value IS NULL THEN 'unknown'
        WHEN rm.metric_name = 'cpu_usage' AND rm.avg_value > 90 THEN 'critical'
        WHEN rm.metric_name = 'cpu_usage' AND rm.avg_value > 70 THEN 'warning'
        WHEN rm.metric_name = 'memory_usage' AND rm.avg_value > 90 THEN 'critical'
        WHEN rm.metric_name = 'memory_usage' AND rm.avg_value > 80 THEN 'warning'
        WHEN re.critical_error_count > 0 THEN 'critical'
        WHEN re.error_count > 10 THEN 'warning'
        ELSE 'healthy'
    END as overall_status,
    rm.*,
    re.error_count,
    re.critical_error_count,
    NOW() as checked_at
FROM recent_metrics rm
CROSS JOIN recent_errors re;

-- 创建实时统计视图
CREATE OR REPLACE VIEW realtime_stats AS
SELECT
    (SELECT COUNT(*) FROM online_users) as current_online_users,
    (SELECT COUNT(DISTINCT room_id) FROM online_users WHERE room_id IS NOT NULL) as active_rooms,
    (SELECT COUNT(*) FROM messages WHERE created_at > NOW() - INTERVAL '1 hour') as messages_last_hour,
    (SELECT COUNT(*) FROM sessions WHERE is_active = true) as active_sessions,
    (SELECT COUNT(*) FROM notifications WHERE is_read = false) as unread_notifications,
    (SELECT SUM(file_size) FROM file_uploads WHERE created_at > CURRENT_DATE) as bytes_uploaded_today;

-- 创建自动生成每日统计的函数
CREATE OR REPLACE FUNCTION generate_daily_stats(target_date DATE DEFAULT CURRENT_DATE)
RETURNS VOID AS $$
DECLARE
    stats_record RECORD;
BEGIN
    -- 计算统计数据
    SELECT
        (SELECT COUNT(*) FROM users WHERE created_at::DATE <= target_date) as total_users,
        (SELECT COUNT(DISTINCT user_id) FROM sessions WHERE last_accessed_at::DATE = target_date) as active_users,
        (SELECT COUNT(*) FROM users WHERE created_at::DATE = target_date) as new_users,
        (SELECT COUNT(*) FROM chat_rooms WHERE created_at::DATE <= target_date AND status = 'active') as total_rooms,
        (SELECT COUNT(DISTINCT room_id) FROM messages WHERE created_at::DATE = target_date) as active_rooms,
        (SELECT COUNT(*) FROM chat_rooms WHERE created_at::DATE = target_date) as new_rooms,
        (SELECT COUNT(*) FROM messages WHERE created_at::DATE <= target_date) as total_messages,
        (SELECT COUNT(*) FROM messages WHERE created_at::DATE = target_date) as new_messages,
        (SELECT COUNT(*) FROM file_uploads WHERE created_at::DATE <= target_date) as total_files,
        (SELECT COUNT(*) FROM file_uploads WHERE created_at::DATE = target_date) as new_files,
        (SELECT COALESCE(SUM(file_size), 0) FROM file_uploads WHERE created_at::DATE <= target_date) as storage_used_bytes,
        (SELECT COALESCE(AVG(EXTRACT(EPOCH FROM (COALESCE(last_accessed_at, expires_at) - created_at))/60), 0)::INTEGER
         FROM sessions WHERE created_at::DATE = target_date) as avg_session_duration_minutes
    INTO stats_record;

    -- 插入或更新统计记录
    INSERT INTO daily_stats (
        stat_date, total_users, active_users, new_users,
        total_rooms, active_rooms, new_rooms,
        total_messages, new_messages,
        total_files, new_files, storage_used_bytes,
        avg_session_duration_minutes
    )
    VALUES (
        target_date, stats_record.total_users, stats_record.active_users, stats_record.new_users,
        stats_record.total_rooms, stats_record.active_rooms, stats_record.new_rooms,
        stats_record.total_messages, stats_record.new_messages,
        stats_record.total_files, stats_record.new_files, stats_record.storage_used_bytes,
        stats_record.avg_session_duration_minutes
    )
    ON CONFLICT (stat_date) DO UPDATE SET
        total_users = EXCLUDED.total_users,
        active_users = EXCLUDED.active_users,
        new_users = EXCLUDED.new_users,
        total_rooms = EXCLUDED.total_rooms,
        active_rooms = EXCLUDED.active_rooms,
        new_rooms = EXCLUDED.new_rooms,
        total_messages = EXCLUDED.total_messages,
        new_messages = EXCLUDED.new_messages,
        total_files = EXCLUDED.total_files,
        new_files = EXCLUDED.new_files,
        storage_used_bytes = EXCLUDED.storage_used_bytes,
        avg_session_duration_minutes = EXCLUDED.avg_session_duration_minutes,
        updated_at = NOW();
END;
$$ LANGUAGE plpgsql;

-- 创建记录系统指标的函数
CREATE OR REPLACE FUNCTION record_system_metric(
    p_metric_name VARCHAR(100),
    p_metric_value DOUBLE PRECISION,
    p_metric_unit VARCHAR(20) DEFAULT NULL,
    p_tags JSONB DEFAULT '{}',
    p_server_id VARCHAR(100) DEFAULT NULL,
    p_instance_id VARCHAR(100) DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    metric_id UUID;
BEGIN
    INSERT INTO system_metrics (
        metric_name, metric_value, metric_unit, tags, server_id, instance_id
    )
    VALUES (
        p_metric_name, p_metric_value, p_metric_unit, p_tags, p_server_id, p_instance_id
    )
    RETURNING id INTO metric_id;

    RETURN metric_id;
END;
$$ LANGUAGE plpgsql;

-- 创建清理旧数据的函数
CREATE OR REPLACE FUNCTION cleanup_old_data()
RETURNS TABLE (
    table_name TEXT,
    deleted_count BIGINT
) AS $$
DECLARE
    deleted_metrics BIGINT;
    deleted_errors BIGINT;
    deleted_activity_logs BIGINT;
BEGIN
    -- 清理30天前的系统指标
    DELETE FROM system_metrics WHERE timestamp < NOW() - INTERVAL '30 days';
    GET DIAGNOSTICS deleted_metrics = ROW_COUNT;

    -- 清理90天前的错误日志
    DELETE FROM error_logs WHERE created_at < NOW() - INTERVAL '90 days';
    GET DIAGNOSTICS deleted_errors = ROW_COUNT;

    -- 清理180天前的用户活动日志
    DELETE FROM user_activity_logs WHERE created_at < NOW() - INTERVAL '180 days';
    GET DIAGNOSTICS deleted_activity_logs = ROW_COUNT;

    RETURN QUERY VALUES
        ('system_metrics', deleted_metrics),
        ('error_logs', deleted_errors),
        ('user_activity_logs', deleted_activity_logs);
END;
$$ LANGUAGE plpgsql;

-- 创建触发器来自动更新在线用户表
CREATE OR REPLACE FUNCTION update_online_users()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' OR TG_OP = 'UPDATE' THEN
        INSERT INTO online_users (user_id, session_id, last_seen_at, device_type, ip_address, user_agent)
        VALUES (NEW.user_id, NEW.id, NEW.last_accessed_at, NEW.session_type, NEW.ip_address, NEW.user_agent)
        ON CONFLICT (user_id) DO UPDATE SET
            session_id = EXCLUDED.session_id,
            last_seen_at = EXCLUDED.last_seen_at,
            device_type = EXCLUDED.device_type,
            ip_address = EXCLUDED.ip_address,
            user_agent = EXCLUDED.user_agent,
            updated_at = NOW();
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        DELETE FROM online_users WHERE session_id = OLD.id;
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_update_online_users ON sessions;
CREATE TRIGGER trg_update_online_users
    AFTER INSERT OR UPDATE OR DELETE ON sessions
    FOR EACH ROW EXECUTE FUNCTION update_online_users();