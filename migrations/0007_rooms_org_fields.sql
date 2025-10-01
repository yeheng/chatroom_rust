-- 聊天室表增加组织字段
ALTER TABLE chat_rooms
    ADD COLUMN IF NOT EXISTS org_id UUID REFERENCES organizations(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS is_org_visible BOOLEAN DEFAULT FALSE;
CREATE INDEX IF NOT EXISTS chat_rooms_org_id_idx ON chat_rooms(org_id);
COMMENT ON COLUMN chat_rooms.org_id IS '所属组织ID(可选)';
COMMENT ON COLUMN chat_rooms.is_org_visible IS '是否对组织内所有用户可见';