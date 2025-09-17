-- 补全用户表缺失字段
-- 添加头像URL字段
ALTER TABLE users
ADD COLUMN IF NOT EXISTS avatar_url VARCHAR(500);

-- 添加密码哈希字段
ALTER TABLE users
ADD COLUMN IF NOT EXISTS password_hash VARCHAR(255);

-- 添加用户状态字段
ALTER TABLE users
ADD COLUMN IF NOT EXISTS status VARCHAR(20) NOT NULL DEFAULT 'active';

-- 添加最后活跃时间字段
ALTER TABLE users
ADD COLUMN IF NOT EXISTS last_active_at TIMESTAMP WITH TIME ZONE;

-- 创建用户状态枚举类型
DO $$ BEGIN
    CREATE TYPE user_status AS ENUM ('active', 'inactive', 'banned');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 修改status字段为枚举类型（如果需要，可以逐步迁移）
-- 暂时保持VARCHAR类型，但添加约束
ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_status_check;

ALTER TABLE users
ADD CONSTRAINT users_status_check
CHECK (status IN ('active', 'inactive', 'banned'));

-- 添加邮箱格式验证约束
ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_email_check;

ALTER TABLE users
ADD CONSTRAINT users_email_check
CHECK (email ~* '^[A-Za-z0-9._%-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,4}$');

-- 添加用户名长度约束
ALTER TABLE users
DROP CONSTRAINT IF EXISTS users_username_length_check;

ALTER TABLE users
ADD CONSTRAINT users_username_length_check
CHECK (length(username) >= 3 AND length(username) <= 50);

-- 创建新的索引
CREATE INDEX IF NOT EXISTS idx_users_status ON users(status);
CREATE INDEX IF NOT EXISTS idx_users_last_active ON users(last_active_at DESC);

-- 为用户名和邮箱创建模糊搜索索引（需要pg_trgm扩展）
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE INDEX IF NOT EXISTS idx_users_username_like ON users USING gin(username gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_users_email_like ON users USING gin(email gin_trgm_ops);

-- 更新现有用户的默认值（如果需要）
UPDATE users
SET status = 'active'
WHERE status IS NULL;

-- 将password_hash设为NOT NULL（生产环境中需要先确保所有用户都有密码）
-- ALTER TABLE users ALTER COLUMN password_hash SET NOT NULL;