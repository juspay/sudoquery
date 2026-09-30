-- Add chat_type enum and column to chats table
CREATE TYPE chat_type AS ENUM ('general', 'create_dashboard');

ALTER TABLE chats ADD COLUMN chat_type chat_type NOT NULL DEFAULT 'general';

CREATE INDEX IF NOT EXISTS idx_chats_chat_type ON chats(chat_type);
