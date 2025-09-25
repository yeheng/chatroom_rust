-- Migration: Convert TEXT fields to proper PostgreSQL ENUM types

-- Create ENUM types
CREATE TYPE user_status AS ENUM ('active', 'inactive', 'suspended');
CREATE TYPE room_role AS ENUM ('owner', 'admin', 'member');
CREATE TYPE message_type AS ENUM ('text', 'image', 'file');

-- Convert users table
ALTER TABLE users
    ALTER COLUMN status TYPE user_status USING status::user_status;

-- Convert room_members table
ALTER TABLE room_members
    ALTER COLUMN role TYPE room_role USING role::room_role;

-- Convert messages table
ALTER TABLE messages
    ALTER COLUMN message_type TYPE message_type USING message_type::message_type;

-- Drop old constraints (they're now enforced by the ENUM type)
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_status_check;
ALTER TABLE room_members DROP CONSTRAINT IF EXISTS room_members_role_check;
ALTER TABLE messages DROP CONSTRAINT IF EXISTS messages_type_check;