CREATE TABLE repositories (
    id VARCHAR(36) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    owner VARCHAR(36) NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    description TEXT,
    is_private BOOLEAN NOT NULL DEFAULT false,
    default_branch VARCHAR(255) NOT NULL DEFAULT 'main',
    created_at BIGINT NOT NULL,
    UNIQUE(owner, name)
);

CREATE INDEX idx_repositories_owner ON repositories(owner);
CREATE INDEX idx_repositories_name ON repositories(name);
