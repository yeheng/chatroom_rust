-- 为原始事件增加组织路径，这是聚合查询的数据来源
ALTER TABLE presence_events
    ADD COLUMN IF NOT EXISTS org_path LTREE;
CREATE INDEX IF NOT EXISTS presence_events_org_path_gist_idx ON presence_events
    USING GIST(org_path gist_ltree_ops(siglen=100));
COMMENT ON COLUMN presence_events.org_path IS '事件发生时的组织路径，用于树形聚合';