-- Convert TEXT column to enum type
-- First drop the column if it exists (to start fresh)
ALTER TABLE chats DROP COLUMN IF EXISTS chat_type;

-- Create the enum type if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'chat_type') THEN
        CREATE TYPE chat_type AS ENUM ('general', 'create_dashboard');
    END IF;
END$$;

-- Add the column with the enum type
ALTER TABLE chats ADD COLUMN chat_type chat_type NOT NULL DEFAULT 'general';

-- Create index
CREATE INDEX IF NOT EXISTS idx_chats_chat_type ON chats(chat_type);
