use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Core repository representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub default_branch: String,
    pub created_at: i64,
}

impl Repository {
    pub fn new(name: String, owner: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            owner,
            description: None,
            is_private: false,
            default_branch: "main".to_string(),
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// User representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: i64,
}

impl User {
    pub fn new(username: String, email: String, password_hash: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            username,
            email,
            password_hash,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// Refresh token representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: i64,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
}

impl RefreshToken {
    pub fn new(user_id: String, token_hash: String, expires_at: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            token_hash,
            expires_at,
            created_at: chrono::Utc::now().timestamp(),
            last_used_at: None,
            revoked_at: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp() > self.expires_at
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_revoked()
    }
}

/// Commit metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub repository_id: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
    pub repository_id: String,
}

/// Database connection manager
pub struct Database {
    pool: sqlx::PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::PgPool::connect(database_url).await?;
        sqlx::migrate!("../migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    pub async fn create_refresh_token(
        &self,
        user_id: &str,
        token_hash: &str,
        expires_at: i64,
    ) -> Result<RefreshToken> {
        let refresh_token = RefreshToken::new(user_id.to_string(), token_hash.to_string(), expires_at);

        sqlx::query!(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            refresh_token.id,
            refresh_token.user_id,
            refresh_token.token_hash,
            refresh_token.expires_at,
            refresh_token.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(refresh_token)
    }

    pub async fn find_refresh_token(&self, token_hash: &str) -> Result<Option<RefreshToken>> {
        let result = sqlx::query_as!(
            RefreshToken,
            r#"
            SELECT id, user_id, token_hash, expires_at, created_at, last_used_at, revoked_at
            FROM refresh_tokens
            WHERE token_hash = $1
            "#,
            token_hash
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn update_refresh_token_last_used(&self, token_hash: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query!(
            r#"
            UPDATE refresh_tokens
            SET last_used_at = $1
            WHERE token_hash = $2
            "#,
            now,
            token_hash
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn revoke_refresh_token(&self, token_hash: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query!(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = $1
            WHERE token_hash = $2
            "#,
            now,
            token_hash
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn cleanup_expired_tokens(&self) -> Result<u64> {
        let now = chrono::Utc::now().timestamp();
        let result = sqlx::query!(
            r#"
            DELETE FROM refresh_tokens
            WHERE expires_at < $1
            "#,
            now
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_creation() {
        let repo = Repository::new("test-repo".to_string(), "test-user".to_string());
        assert_eq!(repo.name, "test-repo");
        assert_eq!(repo.owner, "test-user");
        assert_eq!(repo.default_branch, "main");
        assert!(!repo.is_private);
    }

    #[test]
    fn test_user_creation() {
        let user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hash123".to_string(),
        );
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.password_hash, "hash123");
    }

    #[tokio::test]
    async fn test_database_migrations() {
        // Requires a test database URL in environment
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            let db = Database::new(&db_url).await;
            assert!(db.is_ok(), "Database migrations should run successfully");
        }
    }
}
