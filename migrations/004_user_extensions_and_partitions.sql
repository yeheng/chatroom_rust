-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Soft delete support for chat_rooms
ALTER TABLE IF EXISTS chat_rooms
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

-- User extensions table (JSONB)
CREATE TABLE IF NOT EXISTS user_extensions (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    extensions JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- GIN index for JSONB queries
CREATE INDEX IF NOT EXISTS idx_user_extensions_gin ON user_extensions USING GIN (extensions);

-- Messages monthly partitioning setup (best-effort)
-- If a partitioned parent does not exist, create one and route new data
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relname = 'messages_parent' AND n.nspname = 'public'
    ) THEN
        -- Create parent partitioned table
        CREATE TABLE messages_parent (
            LIKE messages INCLUDING ALL
        ) PARTITION BY RANGE (created_at);

        -- Create trigger to insert into parent (new writes should target parent)
        CREATE OR REPLACE FUNCTION messages_route_to_parent()
        RETURNS TRIGGER LANGUAGE plpgsql AS $$
        BEGIN
            INSERT INTO messages_parent (id, room_id, user_id, content, message_type, created_at)
            VALUES (NEW.id, NEW.room_id, NEW.user_id, NEW.content, NEW.message_type, NEW.created_at);
            RETURN NULL; -- prevent insert into old table
        END;
        $$;

        -- Attach trigger on legacy messages table to route new rows
        DROP TRIGGER IF EXISTS trg_messages_route ON messages;
        CREATE TRIGGER trg_messages_route
        BEFORE INSERT ON messages
        FOR EACH ROW EXECUTE FUNCTION messages_route_to_parent();
    END IF;
END
$$;

-- Create current and next month partitions (idempotent)
DO $$
DECLARE
    start_current DATE := date_trunc('month', current_date);
    start_next DATE := (date_trunc('month', current_date) + INTERVAL '1 month')::date;
    end_current DATE := (date_trunc('month', current_date) + INTERVAL '1 month')::date;
    end_next DATE := (date_trunc('month', current_date) + INTERVAL '2 month')::date;
    part1 TEXT := 'messages_' || to_char(start_current, 'YYYYMM');
    part2 TEXT := 'messages_' || to_char(start_next, 'YYYYMM');
BEGIN
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF messages_parent FOR VALUES FROM (%L) TO (%L);', part1, start_current, end_current);
    EXECUTE format('CREATE TABLE IF NOT EXISTS %I PARTITION OF messages_parent FOR VALUES FROM (%L) TO (%L);', part2, start_next, end_next);
END$$;

