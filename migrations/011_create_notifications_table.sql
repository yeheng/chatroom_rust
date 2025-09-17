-- 创建通知系统表

-- 创建通知类型枚举
DO $$ BEGIN
    CREATE TYPE notification_type AS ENUM (
        'new_message', 'room_invitation', 'user_joined', 'user_left',
        'system_alert', 'file_shared', 'mention', 'reply',
        'room_updated', 'role_changed', 'friend_request'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 创建通知表
CREATE TABLE IF NOT EXISTS notifications (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type VARCHAR(50) NOT NULL,
    title VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    data JSONB DEFAULT '{}', -- 额外的通知数据
    related_user_id UUID REFERENCES users(id) ON DELETE SET NULL, -- 相关用户（如发送者）
    related_room_id UUID REFERENCES chat_rooms(id) ON DELETE SET NULL, -- 相关房间
    related_message_id UUID REFERENCES messages(id) ON DELETE SET NULL, -- 相关消息
    priority VARCHAR(20) DEFAULT 'normal', -- 优先级
    is_read BOOLEAN DEFAULT false,
    is_dismissed BOOLEAN DEFAULT false, -- 是否已忽略
    read_at TIMESTAMP WITH TIME ZONE,
    expires_at TIMESTAMP WITH TIME ZONE, -- 通知过期时间
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- 约束
    CONSTRAINT notifications_type_check CHECK (
        type IN (
            'new_message', 'room_invitation', 'user_joined', 'user_left',
            'system_alert', 'file_shared', 'mention', 'reply',
            'room_updated', 'role_changed', 'friend_request'
        )
    ),
    CONSTRAINT notifications_priority_check CHECK (
        priority IN ('low', 'normal', 'high', 'urgent')
    )
);

-- 创建通知设置表（用户可以自定义通知偏好）
CREATE TABLE IF NOT EXISTS notification_settings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type VARCHAR(50) NOT NULL,
    is_enabled BOOLEAN DEFAULT true,
    push_enabled BOOLEAN DEFAULT true, -- 推送通知
    email_enabled BOOLEAN DEFAULT false, -- 邮件通知
    sound_enabled BOOLEAN DEFAULT true, -- 声音通知
    priority_threshold VARCHAR(20) DEFAULT 'normal', -- 最低优先级阈值
    quiet_hours_start TIME, -- 免打扰开始时间
    quiet_hours_end TIME, -- 免打扰结束时间
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT notification_settings_unique UNIQUE (user_id, notification_type),
    CONSTRAINT notification_settings_type_check CHECK (
        notification_type IN (
            'new_message', 'room_invitation', 'user_joined', 'user_left',
            'system_alert', 'file_shared', 'mention', 'reply',
            'room_updated', 'role_changed', 'friend_request', 'all'
        )
    ),
    CONSTRAINT notification_settings_priority_check CHECK (
        priority_threshold IN ('low', 'normal', 'high', 'urgent')
    )
);

-- 创建通知模板表（用于动态生成通知内容）
CREATE TABLE IF NOT EXISTS notification_templates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    type VARCHAR(50) NOT NULL UNIQUE,
    title_template VARCHAR(255) NOT NULL,
    message_template TEXT NOT NULL,
    default_priority VARCHAR(20) DEFAULT 'normal',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT notification_templates_priority_check CHECK (
        default_priority IN ('low', 'normal', 'high', 'urgent')
    )
);

-- 创建通知发送日志表（用于审计和统计）
CREATE TABLE IF NOT EXISTS notification_delivery_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    notification_id UUID NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    delivery_method VARCHAR(20) NOT NULL, -- 'push', 'email', 'websocket'
    status VARCHAR(20) NOT NULL, -- 'pending', 'sent', 'failed', 'delivered'
    error_message TEXT,
    delivered_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT delivery_method_check CHECK (
        delivery_method IN ('push', 'email', 'websocket', 'sms')
    ),
    CONSTRAINT delivery_status_check CHECK (
        status IN ('pending', 'sent', 'failed', 'delivered', 'read')
    )
);

-- 通知表索引
CREATE INDEX IF NOT EXISTS idx_notifications_user ON notifications(user_id);
CREATE INDEX IF NOT EXISTS idx_notifications_type ON notifications(type);
CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(is_read);
CREATE INDEX IF NOT EXISTS idx_notifications_dismissed ON notifications(is_dismissed);
CREATE INDEX IF NOT EXISTS idx_notifications_priority ON notifications(priority);
CREATE INDEX IF NOT EXISTS idx_notifications_created_at ON notifications(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notifications_expires_at ON notifications(expires_at);
CREATE INDEX IF NOT EXISTS idx_notifications_related_user ON notifications(related_user_id);
CREATE INDEX IF NOT EXISTS idx_notifications_related_room ON notifications(related_room_id);
CREATE INDEX IF NOT EXISTS idx_notifications_related_message ON notifications(related_message_id);

-- 复合索引
CREATE INDEX IF NOT EXISTS idx_notifications_user_unread ON notifications(user_id, is_read, created_at DESC)
WHERE is_read = false;

CREATE INDEX IF NOT EXISTS idx_notifications_user_type ON notifications(user_id, type, created_at DESC);

-- 通知设置表索引
CREATE INDEX IF NOT EXISTS idx_notification_settings_user ON notification_settings(user_id);
CREATE INDEX IF NOT EXISTS idx_notification_settings_type ON notification_settings(notification_type);

-- 通知发送日志索引
CREATE INDEX IF NOT EXISTS idx_notification_delivery_logs_notification ON notification_delivery_logs(notification_id);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_logs_method ON notification_delivery_logs(delivery_method);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_logs_status ON notification_delivery_logs(status);
CREATE INDEX IF NOT EXISTS idx_notification_delivery_logs_created_at ON notification_delivery_logs(created_at DESC);

-- JSONB索引
CREATE INDEX IF NOT EXISTS idx_notifications_data_gin ON notifications USING gin(data);

-- 创建通知统计视图
CREATE OR REPLACE VIEW notification_stats AS
SELECT
    user_id,
    COUNT(*) as total_notifications,
    COUNT(*) FILTER (WHERE is_read = false) as unread_count,
    COUNT(*) FILTER (WHERE priority = 'urgent') as urgent_count,
    COUNT(*) FILTER (WHERE priority = 'high') as high_priority_count,
    COUNT(DISTINCT type) as unique_types_count,
    MAX(created_at) as last_notification_at,
    MIN(created_at) as first_notification_at
FROM notifications
WHERE expires_at IS NULL OR expires_at > NOW()
GROUP BY user_id;

-- 创建自动清理过期通知的函数
CREATE OR REPLACE FUNCTION cleanup_expired_notifications()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM notifications
    WHERE expires_at IS NOT NULL AND expires_at < NOW();

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- 创建批量标记通知为已读的函数
CREATE OR REPLACE FUNCTION mark_notifications_as_read(
    p_user_id UUID,
    p_notification_ids UUID[] DEFAULT NULL,
    p_type VARCHAR(50) DEFAULT NULL
)
RETURNS INTEGER AS $$
DECLARE
    updated_count INTEGER;
BEGIN
    IF p_notification_ids IS NOT NULL THEN
        -- 标记指定通知为已读
        UPDATE notifications
        SET is_read = true, read_at = NOW()
        WHERE user_id = p_user_id
          AND id = ANY(p_notification_ids)
          AND is_read = false;
    ELSIF p_type IS NOT NULL THEN
        -- 标记指定类型的通知为已读
        UPDATE notifications
        SET is_read = true, read_at = NOW()
        WHERE user_id = p_user_id
          AND type = p_type
          AND is_read = false;
    ELSE
        -- 标记所有通知为已读
        UPDATE notifications
        SET is_read = true, read_at = NOW()
        WHERE user_id = p_user_id
          AND is_read = false;
    END IF;

    GET DIAGNOSTICS updated_count = ROW_COUNT;
    RETURN updated_count;
END;
$$ LANGUAGE plpgsql;

-- 创建根据模板生成通知的函数
CREATE OR REPLACE FUNCTION create_notification_from_template(
    p_user_id UUID,
    p_type VARCHAR(50),
    p_data JSONB DEFAULT '{}',
    p_related_user_id UUID DEFAULT NULL,
    p_related_room_id UUID DEFAULT NULL,
    p_related_message_id UUID DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    template_record RECORD;
    notification_id UUID;
    final_title VARCHAR(255);
    final_message TEXT;
    final_priority VARCHAR(20);
BEGIN
    -- 获取通知模板
    SELECT * INTO template_record
    FROM notification_templates
    WHERE type = p_type AND is_active = true;

    IF NOT FOUND THEN
        RAISE EXCEPTION '通知模板不存在: %', p_type;
    END IF;

    -- 简单的模板变量替换（可以根据需要扩展）
    final_title := template_record.title_template;
    final_message := template_record.message_template;
    final_priority := template_record.default_priority;

    -- 替换常见变量
    IF p_data ? 'username' THEN
        final_title := replace(final_title, '{{username}}', p_data->>'username');
        final_message := replace(final_message, '{{username}}', p_data->>'username');
    END IF;

    IF p_data ? 'room_name' THEN
        final_title := replace(final_title, '{{room_name}}', p_data->>'room_name');
        final_message := replace(final_message, '{{room_name}}', p_data->>'room_name');
    END IF;

    -- 检查用户通知设置
    IF NOT should_send_notification(p_user_id, p_type, final_priority) THEN
        RETURN NULL;
    END IF;

    -- 创建通知
    INSERT INTO notifications (
        user_id, type, title, message, data,
        related_user_id, related_room_id, related_message_id,
        priority
    )
    VALUES (
        p_user_id, p_type, final_title, final_message, p_data,
        p_related_user_id, p_related_room_id, p_related_message_id,
        final_priority
    )
    RETURNING id INTO notification_id;

    RETURN notification_id;
END;
$$ LANGUAGE plpgsql;

-- 创建检查是否应该发送通知的函数
CREATE OR REPLACE FUNCTION should_send_notification(
    p_user_id UUID,
    p_type VARCHAR(50),
    p_priority VARCHAR(20)
)
RETURNS BOOLEAN AS $$
DECLARE
    settings_record RECORD;
    priority_order INTEGER;
    threshold_order INTEGER;
    current_time TIME;
BEGIN
    -- 获取用户通知设置
    SELECT * INTO settings_record
    FROM notification_settings
    WHERE user_id = p_user_id AND (notification_type = p_type OR notification_type = 'all')
    ORDER BY CASE WHEN notification_type = p_type THEN 1 ELSE 2 END
    LIMIT 1;

    -- 如果没有设置，默认允许
    IF NOT FOUND THEN
        RETURN true;
    END IF;

    -- 检查是否启用
    IF NOT settings_record.is_enabled THEN
        RETURN false;
    END IF;

    -- 检查优先级阈值
    priority_order := CASE p_priority
        WHEN 'low' THEN 1
        WHEN 'normal' THEN 2
        WHEN 'high' THEN 3
        WHEN 'urgent' THEN 4
        ELSE 2
    END;

    threshold_order := CASE settings_record.priority_threshold
        WHEN 'low' THEN 1
        WHEN 'normal' THEN 2
        WHEN 'high' THEN 3
        WHEN 'urgent' THEN 4
        ELSE 2
    END;

    IF priority_order < threshold_order THEN
        RETURN false;
    END IF;

    -- 检查免打扰时间
    IF settings_record.quiet_hours_start IS NOT NULL AND settings_record.quiet_hours_end IS NOT NULL THEN
        current_time := CURRENT_TIME;
        IF settings_record.quiet_hours_start <= settings_record.quiet_hours_end THEN
            -- 同一天内的时间段
            IF current_time BETWEEN settings_record.quiet_hours_start AND settings_record.quiet_hours_end THEN
                RETURN false;
            END IF;
        ELSE
            -- 跨天的时间段
            IF current_time >= settings_record.quiet_hours_start OR current_time <= settings_record.quiet_hours_end THEN
                RETURN false;
            END IF;
        END IF;
    END IF;

    RETURN true;
END;
$$ LANGUAGE plpgsql;

-- 插入默认通知模板
INSERT INTO notification_templates (type, title_template, message_template, default_priority) VALUES
('new_message', '新消息', '{{username}} 在 {{room_name}} 发送了一条消息', 'normal'),
('room_invitation', '房间邀请', '{{username}} 邀请您加入房间 {{room_name}}', 'normal'),
('user_joined', '用户加入', '{{username}} 加入了房间 {{room_name}}', 'low'),
('user_left', '用户离开', '{{username}} 离开了房间 {{room_name}}', 'low'),
('system_alert', '系统通知', '系统消息', 'high'),
('file_shared', '文件分享', '{{username}} 分享了一个文件', 'normal'),
('mention', '提及通知', '{{username}} 在消息中提及了您', 'high'),
('reply', '回复通知', '{{username}} 回复了您的消息', 'normal'),
('room_updated', '房间更新', '房间 {{room_name}} 设置已更新', 'low'),
('role_changed', '角色变更', '您在房间 {{room_name}} 的角色已更改', 'normal')
ON CONFLICT (type) DO NOTHING;

-- 为每个用户创建默认通知设置
INSERT INTO notification_settings (user_id, notification_type, is_enabled, push_enabled, email_enabled)
SELECT u.id, 'all', true, true, false
FROM users u
WHERE NOT EXISTS (
    SELECT 1 FROM notification_settings ns
    WHERE ns.user_id = u.id AND ns.notification_type = 'all'
);