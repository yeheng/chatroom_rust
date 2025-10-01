-- 批量任务状态跟踪表
CREATE TABLE bulk_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_by UUID NOT NULL REFERENCES users(id),
    total_count INT NOT NULL,
    processed_count INT NOT NULL DEFAULT 0,
    success_count INT NOT NULL DEFAULT 0,
    failed_count INT NOT NULL DEFAULT 0,
    error_message TEXT,
    result_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CONSTRAINT valid_status CHECK (status IN ('pending', 'processing', 'completed', 'failed'))
);
CREATE INDEX bulk_tasks_status_idx ON bulk_tasks(status);
COMMENT ON TABLE bulk_tasks IS '批量任务状态跟踪表';