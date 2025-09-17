-- 创建文件上传管理表

-- 创建文件上传表
CREATE TABLE IF NOT EXISTS file_uploads (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    room_id UUID REFERENCES chat_rooms(id) ON DELETE SET NULL, -- 如果文件属于特定房间
    filename VARCHAR(255) NOT NULL,
    original_filename VARCHAR(255) NOT NULL,
    file_size BIGINT NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    storage_path VARCHAR(500) NOT NULL,
    storage_type VARCHAR(20) NOT NULL DEFAULT 'local',
    checksum VARCHAR(64), -- SHA256 校验和
    thumbnail_path VARCHAR(500), -- 图片缩略图路径
    is_public BOOLEAN DEFAULT false,
    is_temporary BOOLEAN DEFAULT true, -- 临时文件标记
    download_count INTEGER DEFAULT 0,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- 约束
    CONSTRAINT file_uploads_storage_type_check CHECK (
        storage_type IN ('local', 's3', 'minio', 'azure', 'gcs')
    ),
    CONSTRAINT file_uploads_size_check CHECK (file_size > 0),
    CONSTRAINT file_uploads_filename_check CHECK (length(filename) > 0),
    CONSTRAINT file_uploads_mime_type_check CHECK (length(mime_type) > 0)
);

-- 创建文件分享表（用于生成分享链接）
CREATE TABLE IF NOT EXISTS file_shares (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    file_id UUID NOT NULL REFERENCES file_uploads(id) ON DELETE CASCADE,
    shared_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    share_token VARCHAR(255) NOT NULL UNIQUE,
    share_name VARCHAR(255), -- 可自定义的分享名称
    password_hash VARCHAR(255), -- 可选的访问密码
    download_limit INTEGER, -- 下载次数限制
    download_count INTEGER DEFAULT 0,
    expires_at TIMESTAMP WITH TIME ZONE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT file_shares_download_limit_check CHECK (
        download_limit IS NULL OR download_limit > 0
    ),
    CONSTRAINT file_shares_download_count_check CHECK (download_count >= 0)
);

-- 创建文件访问日志表
CREATE TABLE IF NOT EXISTS file_access_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    file_id UUID NOT NULL REFERENCES file_uploads(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    share_id UUID REFERENCES file_shares(id) ON DELETE SET NULL,
    access_type VARCHAR(20) NOT NULL, -- 'view', 'download', 'share'
    ip_address INET,
    user_agent TEXT,
    referrer TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CONSTRAINT file_access_type_check CHECK (
        access_type IN ('view', 'download', 'share', 'preview')
    )
);

-- 文件上传表索引
CREATE INDEX IF NOT EXISTS idx_file_uploads_user ON file_uploads(user_id);
CREATE INDEX IF NOT EXISTS idx_file_uploads_room ON file_uploads(room_id);
CREATE INDEX IF NOT EXISTS idx_file_uploads_public ON file_uploads(is_public);
CREATE INDEX IF NOT EXISTS idx_file_uploads_temporary ON file_uploads(is_temporary);
CREATE INDEX IF NOT EXISTS idx_file_uploads_expires_at ON file_uploads(expires_at);
CREATE INDEX IF NOT EXISTS idx_file_uploads_created_at ON file_uploads(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_file_uploads_mime_type ON file_uploads(mime_type);
CREATE INDEX IF NOT EXISTS idx_file_uploads_storage_type ON file_uploads(storage_type);
CREATE INDEX IF NOT EXISTS idx_file_uploads_checksum ON file_uploads(checksum);

-- 复合索引
CREATE INDEX IF NOT EXISTS idx_file_uploads_user_room ON file_uploads(user_id, room_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_file_uploads_public_type ON file_uploads(is_public, mime_type, created_at DESC);

-- 文件分享表索引
CREATE INDEX IF NOT EXISTS idx_file_shares_file ON file_shares(file_id);
CREATE INDEX IF NOT EXISTS idx_file_shares_shared_by ON file_shares(shared_by);
CREATE INDEX IF NOT EXISTS idx_file_shares_token ON file_shares(share_token);
CREATE INDEX IF NOT EXISTS idx_file_shares_active ON file_shares(is_active, expires_at);
CREATE INDEX IF NOT EXISTS idx_file_shares_expires_at ON file_shares(expires_at);

-- 文件访问日志表索引
CREATE INDEX IF NOT EXISTS idx_file_access_logs_file ON file_access_logs(file_id);
CREATE INDEX IF NOT EXISTS idx_file_access_logs_user ON file_access_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_file_access_logs_share ON file_access_logs(share_id);
CREATE INDEX IF NOT EXISTS idx_file_access_logs_type ON file_access_logs(access_type);
CREATE INDEX IF NOT EXISTS idx_file_access_logs_created_at ON file_access_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_file_access_logs_ip ON file_access_logs(ip_address);

-- 复合索引
CREATE INDEX IF NOT EXISTS idx_file_access_logs_file_type ON file_access_logs(file_id, access_type, created_at DESC);

-- 创建清理过期文件的函数
CREATE OR REPLACE FUNCTION cleanup_expired_files()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    -- 删除过期的文件记录
    DELETE FROM file_uploads
    WHERE expires_at IS NOT NULL AND expires_at < NOW();

    GET DIAGNOSTICS deleted_count = ROW_COUNT;

    -- 清理过期的分享链接
    UPDATE file_shares
    SET is_active = false
    WHERE expires_at IS NOT NULL AND expires_at < NOW() AND is_active = true;

    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- 创建清理临时文件的函数（超过24小时未被引用的临时文件）
CREATE OR REPLACE FUNCTION cleanup_temporary_files()
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM file_uploads
    WHERE is_temporary = true
      AND created_at < NOW() - INTERVAL '24 hours'
      AND id NOT IN (
          SELECT DISTINCT jsonb_array_elements_text((metadata->'attachments')::jsonb)::UUID
          FROM messages
          WHERE metadata ? 'attachments'
      );

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- 创建文件统计视图
CREATE OR REPLACE VIEW file_upload_stats AS
SELECT
    user_id,
    COUNT(*) as total_files,
    SUM(file_size) as total_size_bytes,
    COUNT(*) FILTER (WHERE is_public = true) as public_files,
    COUNT(*) FILTER (WHERE is_temporary = true) as temporary_files,
    COUNT(DISTINCT mime_type) as unique_mime_types,
    MAX(created_at) as last_upload_at,
    MIN(created_at) as first_upload_at
FROM file_uploads
GROUP BY user_id;

-- 创建存储类型统计视图
CREATE OR REPLACE VIEW storage_type_stats AS
SELECT
    storage_type,
    COUNT(*) as file_count,
    SUM(file_size) as total_size_bytes,
    AVG(file_size) as avg_file_size,
    MIN(file_size) as min_file_size,
    MAX(file_size) as max_file_size
FROM file_uploads
GROUP BY storage_type;

-- 创建触发器来自动更新文件上传数和下载计数
CREATE OR REPLACE FUNCTION update_download_count()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.access_type = 'download' THEN
        -- 更新文件下载计数
        UPDATE file_uploads
        SET download_count = download_count + 1,
            updated_at = NOW()
        WHERE id = NEW.file_id;

        -- 如果是通过分享链接下载，更新分享下载计数
        IF NEW.share_id IS NOT NULL THEN
            UPDATE file_shares
            SET download_count = download_count + 1
            WHERE id = NEW.share_id;
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_update_download_count ON file_access_logs;
CREATE TRIGGER trg_update_download_count
    AFTER INSERT ON file_access_logs
    FOR EACH ROW EXECUTE FUNCTION update_download_count();

-- 创建生成文件分享链接的函数
CREATE OR REPLACE FUNCTION create_file_share(
    p_file_id UUID,
    p_shared_by UUID,
    p_share_name VARCHAR(255) DEFAULT NULL,
    p_password VARCHAR(255) DEFAULT NULL,
    p_download_limit INTEGER DEFAULT NULL,
    p_expires_hours INTEGER DEFAULT 24
)
RETURNS TABLE (
    share_id UUID,
    share_token VARCHAR(255),
    share_url VARCHAR(500)
) AS $$
DECLARE
    new_token VARCHAR(255);
    new_share_id UUID;
    password_hash_val VARCHAR(255) := NULL;
BEGIN
    -- 生成分享令牌
    new_token := encode(gen_random_bytes(32), 'base64url');

    -- 如果有密码，进行哈希
    IF p_password IS NOT NULL THEN
        password_hash_val := crypt(p_password, gen_salt('bf'));
    END IF;

    -- 插入分享记录
    INSERT INTO file_shares (
        file_id, shared_by, share_token, share_name,
        password_hash, download_limit,
        expires_at
    )
    VALUES (
        p_file_id, p_shared_by, new_token, p_share_name,
        password_hash_val, p_download_limit,
        CASE WHEN p_expires_hours > 0 THEN NOW() + (p_expires_hours || ' hours')::INTERVAL ELSE NULL END
    )
    RETURNING id INTO new_share_id;

    RETURN QUERY SELECT
        new_share_id,
        new_token,
        '/files/share/' || new_token;
END;
$$ LANGUAGE plpgsql;

-- 创建验证文件分享访问权限的函数
CREATE OR REPLACE FUNCTION validate_file_share_access(
    p_share_token VARCHAR(255),
    p_password VARCHAR(255) DEFAULT NULL
)
RETURNS TABLE (
    file_id UUID,
    filename VARCHAR(255),
    file_size BIGINT,
    mime_type VARCHAR(100),
    can_access BOOLEAN,
    error_message TEXT
) AS $$
DECLARE
    share_record RECORD;
    file_record RECORD;
BEGIN
    -- 查找分享记录
    SELECT * INTO share_record
    FROM file_shares
    WHERE share_token = p_share_token AND is_active = true;

    IF NOT FOUND THEN
        RETURN QUERY SELECT NULL::UUID, NULL::VARCHAR, NULL::BIGINT, NULL::VARCHAR, false, '分享链接不存在或已失效';
        RETURN;
    END IF;

    -- 检查是否过期
    IF share_record.expires_at IS NOT NULL AND share_record.expires_at < NOW() THEN
        RETURN QUERY SELECT NULL::UUID, NULL::VARCHAR, NULL::BIGINT, NULL::VARCHAR, false, '分享链接已过期';
        RETURN;
    END IF;

    -- 检查下载次数限制
    IF share_record.download_limit IS NOT NULL AND share_record.download_count >= share_record.download_limit THEN
        RETURN QUERY SELECT NULL::UUID, NULL::VARCHAR, NULL::BIGINT, NULL::VARCHAR, false, '下载次数已达上限';
        RETURN;
    END IF;

    -- 检查密码
    IF share_record.password_hash IS NOT NULL THEN
        IF p_password IS NULL OR NOT (crypt(p_password, share_record.password_hash) = share_record.password_hash) THEN
            RETURN QUERY SELECT NULL::UUID, NULL::VARCHAR, NULL::BIGINT, NULL::VARCHAR, false, '密码错误';
            RETURN;
        END IF;
    END IF;

    -- 获取文件信息
    SELECT * INTO file_record
    FROM file_uploads
    WHERE id = share_record.file_id;

    IF NOT FOUND THEN
        RETURN QUERY SELECT NULL::UUID, NULL::VARCHAR, NULL::BIGINT, NULL::VARCHAR, false, '文件不存在';
        RETURN;
    END IF;

    -- 检查文件是否过期
    IF file_record.expires_at IS NOT NULL AND file_record.expires_at < NOW() THEN
        RETURN QUERY SELECT NULL::UUID, NULL::VARCHAR, NULL::BIGINT, NULL::VARCHAR, false, '文件已过期';
        RETURN;
    END IF;

    -- 访问权限验证通过
    RETURN QUERY SELECT
        file_record.id,
        file_record.original_filename,
        file_record.file_size,
        file_record.mime_type,
        true,
        NULL::TEXT;
END;
$$ LANGUAGE plpgsql;