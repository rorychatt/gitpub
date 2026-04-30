use anyhow::Result;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
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
    #[serde(skip_serializing)]
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

    pub fn new_with_password(username: String, email: String, password: &str) -> Result<Self> {
        let password_hash = Self::hash_password(password)?;
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            username,
            email,
            password_hash,
            created_at: chrono::Utc::now().timestamp(),
        })
    }

    pub fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;
        Ok(hash.to_string())
    }

    pub fn verify_password(&self, password: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(&self.password_hash)
            .map_err(|e| anyhow::anyhow!("Failed to parse password hash: {}", e))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }
}

/// Commit metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
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

    #[test]
    fn test_hash_password_creates_valid_hash() {
        let password = "test_password_123";
        let hash = User::hash_password(password).expect("Failed to hash password");

        assert_ne!(hash, password);
        assert!(!hash.is_empty());
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_verify_password_correct() {
        let password = "correct_password";
        let user = User::new_with_password(
            "testuser".to_string(),
            "test@example.com".to_string(),
            password,
        )
        .expect("Failed to create user");

        let result = user
            .verify_password(password)
            .expect("Failed to verify password");
        assert!(result);
    }

    #[test]
    fn test_verify_password_incorrect() {
        let password = "correct_password";
        let user = User::new_with_password(
            "testuser".to_string(),
            "test@example.com".to_string(),
            password,
        )
        .expect("Failed to create user");

        let result = user
            .verify_password("wrong_password")
            .expect("Failed to verify password");
        assert!(!result);
    }
}
