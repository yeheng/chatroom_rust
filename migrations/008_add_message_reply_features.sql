-- 为消息表添加回复功能和扩展消息类型

-- 扩展消息类型枚举，添加更多类型
DROP TYPE IF EXISTS message_type CASCADE;
CREATE TYPE message_type AS ENUM ('text', 'image', 'file', 'system', 'bot', 'emoji');

-- 为消息表添加缺失字段
ALTER TABLE messages
ADD COLUMN IF NOT EXISTS reply_to_message_id UUID REFERENCES messages(id) ON DELETE SET NULL;

ALTER TABLE messages
ADD COLUMN IF NOT EXISTS is_edited BOOLEAN DEFAULT false;

ALTER TABLE messages
ADD COLUMN IF NOT EXISTS is_deleted BOOLEAN DEFAULT false;

ALTER TABLE messages
ADD COLUMN IF NOT EXISTS metadata JSONB DEFAULT '{}';

ALTER TABLE messages
ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

-- 重新应用message_type字段（因为枚举更新）
ALTER TABLE messages
ALTER COLUMN message_type TYPE VARCHAR(20);

-- 更新现有数据
UPDATE messages SET message_type = 'text' WHERE message_type IS NULL;

-- 添加约束
ALTER TABLE messages
ADD CONSTRAINT messages_type_check
CHECK (message_type IN ('text', 'image', 'file', 'system', 'bot', 'emoji'));

-- 添加软删除约束：删除的消息内容应该被清空
ALTER TABLE messages
ADD CONSTRAINT messages_deleted_content_check
CHECK (
    (is_deleted = false) OR
    (is_deleted = true AND content = '[已删除]')
);

-- 创建新的索引
CREATE INDEX IF NOT EXISTS idx_messages_reply_to ON messages(reply_to_message_id);
CREATE INDEX IF NOT EXISTS idx_messages_type ON messages(message_type);
CREATE INDEX IF NOT EXISTS idx_messages_is_deleted ON messages(is_deleted);
CREATE INDEX IF NOT EXISTS idx_messages_is_edited ON messages(is_edited);
CREATE INDEX IF NOT EXISTS idx_messages_updated_at ON messages(updated_at DESC);

-- 复合索引用于常见查询
CREATE INDEX IF NOT EXISTS idx_messages_room_not_deleted ON messages(room_id, created_at DESC)
WHERE is_deleted = false;

CREATE INDEX IF NOT EXISTS idx_messages_user_not_deleted ON messages(user_id, created_at DESC)
WHERE is_deleted = false;

CREATE INDEX IF NOT EXISTS idx_messages_room_type ON messages(room_id, message_type, created_at DESC);

-- 全文搜索索引（仅对文本类型的未删除消息）
CREATE INDEX IF NOT EXISTS idx_messages_content_search ON messages
USING gin(to_tsvector('english', content))
WHERE is_deleted = false AND message_type = 'text';

-- 元数据字段GIN索引
CREATE INDEX IF NOT EXISTS idx_messages_metadata_gin ON messages USING gin(metadata);

-- 创建触发器来自动更新updated_at字段
CREATE OR REPLACE FUNCTION update_messages_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_messages_updated_at ON messages;
CREATE TRIGGER trg_messages_updated_at
    BEFORE UPDATE ON messages
    FOR EACH ROW EXECUTE FUNCTION update_messages_updated_at();

-- 创建触发器来更新房间最后活跃时间
CREATE OR REPLACE FUNCTION update_room_last_activity()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE chat_rooms
    SET last_activity_at = NOW()
    WHERE id = NEW.room_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_room_last_activity ON messages;
CREATE TRIGGER trg_room_last_activity
    AFTER INSERT ON messages
    FOR EACH ROW EXECUTE FUNCTION update_room_last_activity();

-- 创建消息统计视图
CREATE OR REPLACE VIEW message_stats AS
SELECT
    room_id,
    COUNT(*) as total_messages,
    COUNT(*) FILTER (WHERE is_deleted = false) as active_messages,
    COUNT(*) FILTER (WHERE message_type = 'text') as text_messages,
    COUNT(*) FILTER (WHERE message_type = 'image') as image_messages,
    COUNT(*) FILTER (WHERE message_type = 'file') as file_messages,
    COUNT(*) FILTER (WHERE reply_to_message_id IS NOT NULL) as reply_messages,
    MAX(created_at) as last_message_at,
    MIN(created_at) as first_message_at
FROM messages
GROUP BY room_id;

-- 创建消息回复链查询的递归函数
CREATE OR REPLACE FUNCTION get_message_reply_chain(p_message_id UUID)
RETURNS TABLE (
    message_id UUID,
    reply_to_message_id UUID,
    content TEXT,
    user_id UUID,
    username VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE,
    level INTEGER
) AS $$
BEGIN
    RETURN QUERY
    WITH RECURSIVE reply_chain AS (
        -- 基础案例：起始消息
        SELECT
            m.id as message_id,
            m.reply_to_message_id,
            m.content,
            m.user_id,
            u.username,
            m.created_at,
            0 as level
        FROM messages m
        JOIN users u ON m.user_id = u.id
        WHERE m.id = p_message_id

        UNION ALL

        -- 递归案例：查找回复
        SELECT
            m.id as message_id,
            m.reply_to_message_id,
            m.content,
            m.user_id,
            u.username,
            m.created_at,
            rc.level + 1
        FROM messages m
        JOIN users u ON m.user_id = u.id
        JOIN reply_chain rc ON m.reply_to_message_id = rc.message_id
        WHERE rc.level < 10 -- 防止无限递归
    )
    SELECT * FROM reply_chain ORDER BY level, created_at;
END;
$$ LANGUAGE plpgsql;

-- 创建函数来软删除消息
CREATE OR REPLACE FUNCTION soft_delete_message(p_message_id UUID, p_user_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    message_user_id UUID;
    is_admin BOOLEAN := false;
BEGIN
    -- 检查消息是否存在及其所有者
    SELECT user_id INTO message_user_id
    FROM messages
    WHERE id = p_message_id AND is_deleted = false;

    IF NOT FOUND THEN
        RETURN false;
    END IF;

    -- 检查是否有权限删除（消息所有者或房间管理员）
    IF message_user_id = p_user_id THEN
        is_admin := true;
    ELSE
        -- 检查是否是房间管理员
        SELECT check_room_admin_permission(p_user_id, m.room_id) INTO is_admin
        FROM messages m
        WHERE m.id = p_message_id;
    END IF;

    IF is_admin THEN
        UPDATE messages
        SET
            is_deleted = true,
            content = '[已删除]',
            updated_at = NOW()
        WHERE id = p_message_id;
        RETURN true;
    END IF;

    RETURN false;
END;
$$ LANGUAGE plpgsql;

-- 更新现有消息的默认值
UPDATE messages
SET
    is_edited = false,
    is_deleted = false,
    metadata = '{}',
    updated_at = COALESCE(updated_at, created_at)
WHERE is_edited IS NULL OR is_deleted IS NULL;