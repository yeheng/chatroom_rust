-- 创建房间成员表和相关索引

-- 创建房间成员角色枚举
DO $$ BEGIN
    CREATE TYPE room_member_role AS ENUM ('owner', 'admin', 'moderator', 'member');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 创建房间成员表
CREATE TABLE IF NOT EXISTS room_members (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    room_id UUID NOT NULL REFERENCES chat_rooms(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL DEFAULT 'member',
    joined_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_read_message_id UUID,
    is_muted BOOLEAN DEFAULT false,
    notifications_enabled BOOLEAN DEFAULT true,
    permissions JSONB DEFAULT '{}',
    invited_by UUID REFERENCES users(id) ON DELETE SET NULL,
    invitation_accepted_at TIMESTAMP WITH TIME ZONE,
    last_activity_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- 约束
    CONSTRAINT room_members_role_check CHECK (role IN ('owner', 'admin', 'moderator', 'member')),
    CONSTRAINT room_members_unique UNIQUE (room_id, user_id)
);

-- 消息回复表（支持消息线程）
CREATE TABLE IF NOT EXISTS message_replies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    reply_message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT message_replies_unique UNIQUE (message_id, reply_message_id),
    CONSTRAINT message_replies_self_check CHECK (message_id != reply_message_id)
);

-- 基础索引
CREATE INDEX IF NOT EXISTS idx_room_members_room_id ON room_members(room_id);
CREATE INDEX IF NOT EXISTS idx_room_members_user_id ON room_members(user_id);
CREATE INDEX IF NOT EXISTS idx_room_members_role ON room_members(role);
CREATE INDEX IF NOT EXISTS idx_room_members_joined_at ON room_members(joined_at);
CREATE INDEX IF NOT EXISTS idx_room_members_last_activity ON room_members(last_activity_at DESC);
CREATE INDEX IF NOT EXISTS idx_room_members_invited_by ON room_members(invited_by);

-- 复合索引用于常见查询
CREATE INDEX IF NOT EXISTS idx_room_members_room_role ON room_members(room_id, role);
CREATE INDEX IF NOT EXISTS idx_room_members_user_room ON room_members(user_id, room_id);
CREATE INDEX IF NOT EXISTS idx_room_members_active ON room_members(room_id, is_muted, notifications_enabled);

-- 消息回复表索引
CREATE INDEX IF NOT EXISTS idx_message_replies_message_id ON message_replies(message_id);
CREATE INDEX IF NOT EXISTS idx_message_replies_reply_id ON message_replies(reply_message_id);
CREATE INDEX IF NOT EXISTS idx_message_replies_created_at ON message_replies(created_at);

-- 权限字段GIN索引
CREATE INDEX IF NOT EXISTS idx_room_members_permissions_gin ON room_members USING gin(permissions);

-- 创建触发器函数来维护房间成员数量
CREATE OR REPLACE FUNCTION update_room_member_count()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        -- 增加成员数量
        UPDATE chat_rooms
        SET member_count = member_count + 1,
            last_activity_at = NOW()
        WHERE id = NEW.room_id;
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        -- 减少成员数量
        UPDATE chat_rooms
        SET member_count = GREATEST(member_count - 1, 0),
            last_activity_at = NOW()
        WHERE id = OLD.room_id;
        RETURN OLD;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- 创建触发器
DROP TRIGGER IF EXISTS trg_room_member_count ON room_members;
CREATE TRIGGER trg_room_member_count
    AFTER INSERT OR DELETE ON room_members
    FOR EACH ROW EXECUTE FUNCTION update_room_member_count();

-- 创建函数来获取用户在房间的最高权限角色
CREATE OR REPLACE FUNCTION get_user_room_role(p_user_id UUID, p_room_id UUID)
RETURNS VARCHAR(20) AS $$
DECLARE
    user_role VARCHAR(20);
    room_owner_id UUID;
BEGIN
    -- 检查是否是房间所有者
    SELECT owner_id INTO room_owner_id FROM chat_rooms WHERE id = p_room_id;
    IF room_owner_id = p_user_id THEN
        RETURN 'owner';
    END IF;

    -- 检查房间成员角色
    SELECT role INTO user_role
    FROM room_members
    WHERE user_id = p_user_id AND room_id = p_room_id;

    RETURN COALESCE(user_role, 'none');
END;
$$ LANGUAGE plpgsql;

-- 创建视图来简化房间成员信息查询
CREATE OR REPLACE VIEW room_members_with_user_info AS
SELECT
    rm.*,
    u.username,
    u.email,
    u.avatar_url,
    u.status as user_status,
    u.last_active_at as user_last_active_at,
    cr.name as room_name,
    cr.is_private as room_is_private
FROM room_members rm
JOIN users u ON rm.user_id = u.id
JOIN chat_rooms cr ON rm.room_id = cr.id;

-- 初始化现有房间的成员数量（如果需要）
UPDATE chat_rooms
SET member_count = (
    SELECT COUNT(*)
    FROM room_members rm
    WHERE rm.room_id = chat_rooms.id
);

-- 创建房间管理员权限检查函数
CREATE OR REPLACE FUNCTION check_room_admin_permission(p_user_id UUID, p_room_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    user_role VARCHAR(20);
    room_owner_id UUID;
BEGIN
    -- 检查是否是房间所有者
    SELECT owner_id INTO room_owner_id FROM chat_rooms WHERE id = p_room_id;
    IF room_owner_id = p_user_id THEN
        RETURN true;
    END IF;

    -- 检查是否是管理员或更高权限
    SELECT role INTO user_role
    FROM room_members
    WHERE user_id = p_user_id AND room_id = p_room_id;

    RETURN user_role IN ('owner', 'admin');
END;
$$ LANGUAGE plpgsql;