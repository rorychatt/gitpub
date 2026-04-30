CREATE TABLE commits (
    sha VARCHAR(40) PRIMARY KEY,
    repository_id VARCHAR(36) NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    author VARCHAR(36) NOT NULL REFERENCES users(id),
    timestamp BIGINT NOT NULL
);

CREATE INDEX idx_commits_repository ON commits(repository_id);
CREATE INDEX idx_commits_author ON commits(author);
CREATE INDEX idx_commits_timestamp ON commits(timestamp);
