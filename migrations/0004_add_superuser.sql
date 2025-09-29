-- Add superuser support for system-level admin access
-- This migration adds an is_superuser column to the users table

ALTER TABLE users ADD COLUMN IF NOT EXISTS is_superuser BOOLEAN NOT NULL DEFAULT FALSE;

-- Create index for faster superuser queries
CREATE INDEX IF NOT EXISTS idx_users_is_superuser ON users(is_superuser) WHERE is_superuser = TRUE;

-- Add comment for documentation
COMMENT ON COLUMN users.is_superuser IS 'Indicates if user has system-level administrator privileges';