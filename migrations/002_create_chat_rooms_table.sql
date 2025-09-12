-- 创建聊天室表
CREATE TABLE IF NOT EXISTS chat_rooms (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_chat_rooms_owner_id ON chat_rooms(owner_id);
CREATE INDEX IF NOT EXISTS idx_chat_rooms_name ON chat_rooms(name);
CREATE INDEX IF NOT EXISTS idx_chat_rooms_created_at ON chat_rooms(created_at);