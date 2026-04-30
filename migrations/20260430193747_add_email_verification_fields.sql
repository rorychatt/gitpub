ALTER TABLE users
  ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN verification_token TEXT,
  ADD COLUMN verification_token_expires_at BIGINT;

CREATE INDEX idx_users_verification_token ON users(verification_token);
