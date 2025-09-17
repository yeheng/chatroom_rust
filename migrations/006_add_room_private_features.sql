-- 为聊天室表添加私密功能和缺失字段

-- 添加是否私密房间字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS is_private BOOLEAN NOT NULL DEFAULT false;

-- 添加房间密码哈希字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS password_hash VARCHAR(255);

-- 添加最大成员数字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS max_members INTEGER DEFAULT 1000;

-- 添加是否允许邀请字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS allow_invites BOOLEAN DEFAULT true;

-- 添加是否需要审批字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS require_approval BOOLEAN DEFAULT false;

-- 添加房间设置JSONB字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS settings JSONB DEFAULT '{}';

-- 添加最后活跃时间字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS last_activity_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

-- 添加成员数量字段（冗余字段，用于性能优化）
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS member_count INTEGER DEFAULT 0;

-- 添加房间状态字段
ALTER TABLE chat_rooms
ADD COLUMN IF NOT EXISTS status VARCHAR(20) DEFAULT 'active';

-- 创建房间状态枚举
DO $$ BEGIN
    CREATE TYPE room_status AS ENUM ('active', 'archived', 'deleted');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 添加房间名称格式约束
ALTER TABLE chat_rooms
DROP CONSTRAINT IF EXISTS chat_rooms_name_check;

ALTER TABLE chat_rooms
ADD CONSTRAINT chat_rooms_name_check
CHECK (length(name) >= 1 AND length(name) <= 100);

-- 添加私密房间必须有密码的约束
ALTER TABLE chat_rooms
DROP CONSTRAINT IF EXISTS chat_rooms_password_check;

ALTER TABLE chat_rooms
ADD CONSTRAINT chat_rooms_password_check
CHECK (
    (is_private = true AND password_hash IS NOT NULL) OR
    (is_private = false)
);

-- 添加最大成员数合理性约束
ALTER TABLE chat_rooms
ADD CONSTRAINT chat_rooms_max_members_check
CHECK (max_members > 0 AND max_members <= 10000);

-- 添加状态约束
ALTER TABLE chat_rooms
ADD CONSTRAINT chat_rooms_status_check
CHECK (status IN ('active', 'archived', 'deleted'));

-- 创建新的索引
CREATE INDEX IF NOT EXISTS idx_chat_rooms_is_private ON chat_rooms(is_private);
CREATE INDEX IF NOT EXISTS idx_chat_rooms_status ON chat_rooms(status);
CREATE INDEX IF NOT EXISTS idx_chat_rooms_last_activity ON chat_rooms(last_activity_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_rooms_member_count ON chat_rooms(member_count DESC);

-- 为房间名称创建模糊搜索索引
CREATE INDEX IF NOT EXISTS idx_chat_rooms_name_like ON chat_rooms USING gin(name gin_trgm_ops);

-- 为设置字段创建GIN索引
CREATE INDEX IF NOT EXISTS idx_chat_rooms_settings_gin ON chat_rooms USING gin(settings);

-- 复合索引用于常见查询
CREATE INDEX IF NOT EXISTS idx_chat_rooms_owner_status ON chat_rooms(owner_id, status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_rooms_private_status ON chat_rooms(is_private, status, created_at DESC);

-- 更新现有房间的默认值
UPDATE chat_rooms
SET
    is_private = false,
    max_members = 1000,
    allow_invites = true,
    require_approval = false,
    settings = '{}',
    last_activity_at = COALESCE(updated_at, created_at),
    member_count = 0,
    status = 'active'
WHERE is_private IS NULL OR status IS NULL;