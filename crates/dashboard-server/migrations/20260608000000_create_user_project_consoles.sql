CREATE TABLE IF NOT EXISTS user_project_consoles (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    proj_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    console TEXT,
    name VARCHAR(255),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_user_project_consoles_user_id ON user_project_consoles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_project_consoles_proj_id ON user_project_consoles(proj_id);
CREATE INDEX IF NOT EXISTS idx_user_project_consoles_user_proj ON user_project_consoles(user_id, proj_id);
