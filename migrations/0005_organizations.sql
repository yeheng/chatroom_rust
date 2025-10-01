-- 启用ltree扩展
CREATE EXTENSION IF NOT EXISTS ltree;

-- 组织表 - 树形结构 (简化版，移除冗余字段)
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    path LTREE NOT NULL UNIQUE,  -- 唯一的真相来源，parent_id和level都可从此派生
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- GIST索引用于高效树形查询
CREATE INDEX org_path_gist_idx ON organizations USING GIST(path gist_ltree_ops(siglen=100));

COMMENT ON TABLE organizations IS '组织架构树形表(使用ltree实现,path是唯一真相来源)';
COMMENT ON COLUMN organizations.path IS 'ltree路径,所有树形信息的唯一来源(level/parent可派生)';