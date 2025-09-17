-- 创建用户会话管理表

-- 创建会话表
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL UNIQUE,
    refresh_token_hash VARCHAR(255),
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    ip_address INET,
    user_agent TEXT,
    device_info JSONB DEFAULT '{}',
    is_active BOOLEAN DEFAULT true,
    session_type VARCHAR(20) DEFAULT 'web',

    -- 约束
    CONSTRAINT sessions_expires_check CHECK (expires_at > created_at),
    CONSTRAINT sessions_type_check CHECK (session_type IN ('web', 'mobile', 'api', 'bot'))
);

-- 创建在线时长统计表
CREATE TABLE IF NOT EXISTS online_time_stats (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    total_seconds INTEGER NOT NULL DEFAULT 0,
    sessions_count INTEGER NOT NULL DEFAULT 0,
    first_session_at TIMESTAMP WITH TIME ZONE,
    last_session_at TIMESTAMP WITH TIME ZONE,
    device_types JSONB DEFAULT '{}', -- 存储设备类型统计
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- 约束
    CONSTRAINT online_time_stats_unique UNIQUE (user_id, date),
    CONSTRAINT online_time_stats_seconds_check CHECK (total_seconds >= 0),
    CONSTRAINT online_time_stats_sessions_check CHECK (sessions_count >= 0)
);

-- 创建用户活动日志表（用于审计）
CREATE TABLE IF NOT EXISTS user_activity_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
    activity_type VARCHAR(50) NOT NULL,
    activity_data JSONB DEFAULT '{}',
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- 约束
    CONSTRAINT activity_type_check CHECK (
        activity_type IN (
            'login', 'logout', 'password_change', 'profile_update',
            'room_join', 'room_leave', 'room_create', 'room_delete',
            'message_send', 'message_edit', 'message_delete',
            'file_upload', 'settings_change'
        )
    )
);

-- 会话表索引
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_refresh_token ON sessions(refresh_token_hash);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(is_active, expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_ip_address ON sessions(ip_address);
CREATE INDEX IF NOT EXISTS idx_sessions_last_accessed ON sessions(last_accessed_at DESC);

-- 复合索引
CREATE INDEX IF NOT EXISTS idx_sessions_user_active ON sessions(user_id, is_active, expires_at);
CREATE INDEX IF NOT EXISTS idx_sessions_type_active ON sessions(session_type, is_active);

-- 在线时长统计表索引
CREATE INDEX IF NOT EXISTS idx_online_time_stats_user ON online_time_stats(user_id);
CREATE INDEX IF NOT EXISTS idx_online_time_stats_date ON online_time_stats(date DESC);
CREATE INDEX IF NOT EXISTS idx_online_time_stats_user_date ON online_time_stats(user_id, date DESC);

-- 用户活动日志索引
CREATE INDEX IF NOT EXISTS idx_activity_logs_user ON user_activity_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_activity_logs_session ON user_activity_logs(session_id);
CREATE INDEX IF NOT EXISTS idx_activity_logs_type ON user_activity_logs(activity_type);
CREATE INDEX IF NOT EXISTS idx_activity_logs_created_at ON user_activity_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_logs_ip ON user_activity_logs(ip_address);

-- 复合索引
CREATE INDEX IF NOT EXISTS idx_activity_logs_user_type ON user_activity_logs(user_id, activity_type, created_at DESC);

-- GIN索引用于JSONB字段
CREATE INDEX IF NOT EXISTS idx_sessions_device_info_gin ON sessions USING gin(device_info);
CREATE INDEX IF NOT EXISTS idx_online_stats_device_types_gin ON online_time_stats USING gin(device_types);
CREATE INDEX IF NOT EXISTS idx_activity_logs_data_gin ON user_activity_logs USING gin(activity_data);

-- 创建清理过期会话的函数
CREATE OR REPLACE FUNCTION cleanup_expired_sessions()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM sessions
    WHERE expires_at < NOW() AND is_active = false;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- 创建更新用户在线时长的函数
CREATE OR REPLACE FUNCTION update_user_online_time(
    p_user_id UUID,
    p_session_start TIMESTAMP WITH TIME ZONE,
    p_session_end TIMESTAMP WITH TIME ZONE,
    p_device_type VARCHAR(20) DEFAULT 'web'
)
RETURNS VOID AS $$
DECLARE
    session_date DATE;
    session_seconds INTEGER;
BEGIN
    session_date := p_session_start::DATE;
    session_seconds := EXTRACT(EPOCH FROM (p_session_end - p_session_start))::INTEGER;

    INSERT INTO online_time_stats (user_id, date, total_seconds, sessions_count, first_session_at, last_session_at, device_types)
    VALUES (
        p_user_id,
        session_date,
        session_seconds,
        1,
        p_session_start,
        p_session_end,
        jsonb_build_object(p_device_type, 1)
    )
    ON CONFLICT (user_id, date) DO UPDATE SET
        total_seconds = online_time_stats.total_seconds + session_seconds,
        sessions_count = online_time_stats.sessions_count + 1,
        first_session_at = LEAST(online_time_stats.first_session_at, p_session_start),
        last_session_at = GREATEST(online_time_stats.last_session_at, p_session_end),
        device_types = online_time_stats.device_types || jsonb_build_object(
            p_device_type,
            COALESCE((online_time_stats.device_types->>p_device_type)::INTEGER, 0) + 1
        ),
        updated_at = NOW();
END;
$$ LANGUAGE plpgsql;

-- 创建记录用户活动的函数
CREATE OR REPLACE FUNCTION log_user_activity(
    p_user_id UUID,
    p_session_id UUID,
    p_activity_type VARCHAR(50),
    p_activity_data JSONB DEFAULT '{}',
    p_ip_address INET DEFAULT NULL,
    p_user_agent TEXT DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    log_id UUID;
BEGIN
    INSERT INTO user_activity_logs (
        user_id, session_id, activity_type, activity_data, ip_address, user_agent
    )
    VALUES (
        p_user_id, p_session_id, p_activity_type, p_activity_data, p_ip_address, p_user_agent
    )
    RETURNING id INTO log_id;

    RETURN log_id;
END;
$$ LANGUAGE plpgsql;

-- 创建会话统计视图
CREATE OR REPLACE VIEW session_stats AS
SELECT
    user_id,
    COUNT(*) as total_sessions,
    COUNT(*) FILTER (WHERE is_active = true) as active_sessions,
    COUNT(DISTINCT session_type) as session_types_count,
    COUNT(DISTINCT ip_address) as unique_ips_count,
    MAX(last_accessed_at) as last_session_at,
    MIN(created_at) as first_session_at,
    AVG(EXTRACT(EPOCH FROM (COALESCE(last_accessed_at, expires_at) - created_at))/3600) as avg_session_hours
FROM sessions
GROUP BY user_id;

-- 创建在线时长月度汇总视图
CREATE OR REPLACE VIEW monthly_online_stats AS
SELECT
    user_id,
    EXTRACT(YEAR FROM date) as year,
    EXTRACT(MONTH FROM date) as month,
    SUM(total_seconds) as total_seconds,
    SUM(sessions_count) as total_sessions,
    AVG(total_seconds) as avg_daily_seconds,
    COUNT(DISTINCT date) as active_days,
    MIN(first_session_at) as month_first_session,
    MAX(last_session_at) as month_last_session
FROM online_time_stats
GROUP BY user_id, EXTRACT(YEAR FROM date), EXTRACT(MONTH FROM date);

-- 创建触发器来自动更新最后访问时间
CREATE OR REPLACE FUNCTION update_user_last_active()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE users
    SET last_active_at = NEW.last_accessed_at
    WHERE id = NEW.user_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_update_user_last_active ON sessions;
CREATE TRIGGER trg_update_user_last_active
    AFTER UPDATE OF last_accessed_at ON sessions
    FOR EACH ROW EXECUTE FUNCTION update_user_last_active();