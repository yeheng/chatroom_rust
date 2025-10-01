-- 用户表增加组织字段 (仅org_id，移除冗余的org_path)
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS org_id UUID REFERENCES organizations(id) ON DELETE SET NULL;

-- 索引用于JOIN查询
CREATE INDEX IF NOT EXISTS users_org_id_idx ON users(org_id);

COMMENT ON COLUMN users.org_id IS '所属组织ID,通过JOIN organizations获取org_path';