CREATE TABLE commits (
    sha TEXT PRIMARY KEY,
    message TEXT NOT NULL,
    author TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
    repository_id TEXT NOT NULL,
    FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE
);

CREATE INDEX idx_commits_repository_id ON commits(repository_id);
CREATE INDEX idx_commits_author ON commits(author);
CREATE INDEX idx_commits_timestamp ON commits(timestamp);
