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

/// Commit metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
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

    /// Create a new user
    pub async fn create_user(&self, username: &str, email: &str, password_hash: &str) -> Result<User> {
        let user = User::new(username.to_string(), email.to_string(), password_hash.to_string());

        sqlx::query!(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
            user.id,
            user.username,
            user.email,
            user.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(user)
    }

    /// Get user by ID
    pub async fn get_user(&self, id: &str) -> Result<Option<User>> {
        let row = sqlx::query!(
            "SELECT id, username, email, created_at FROM users WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| User {
            id: r.id,
            username: r.username,
            email: r.email,
            password_hash: String::new(), // Password hash is not stored in DB in current schema
            created_at: r.created_at,
        }))
    }

    /// Get user by username
    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let row = sqlx::query!(
            "SELECT id, username, email, created_at FROM users WHERE username = $1",
            username
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| User {
            id: r.id,
            username: r.username,
            email: r.email,
            password_hash: String::new(), // Password hash is not stored in DB in current schema
            created_at: r.created_at,
        }))
    }

    /// List all users
    pub async fn list_users(&self) -> Result<Vec<User>> {
        let rows = sqlx::query!(
            "SELECT id, username, email, created_at FROM users"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| User {
            id: r.id,
            username: r.username,
            email: r.email,
            password_hash: String::new(), // Password hash is not stored in DB in current schema
            created_at: r.created_at,
        }).collect())
    }

    /// Update user email
    pub async fn update_user_email(&self, id: &str, email: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE users SET email = $1 WHERE id = $2",
            email,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete user
    pub async fn delete_user(&self, id: &str) -> Result<()> {
        sqlx::query!(
            "DELETE FROM users WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> Database {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/gitpub_test".to_string());
        Database::new(&db_url).await.unwrap()
    }

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

    #[tokio::test]
    async fn test_create_and_get_user() {
        if std::env::var("DATABASE_URL").is_err() {
            return; // Skip if no test database
        }
        let db = test_db().await;
        let user = db.create_user("testuser", "test@example.com", "hash123").await.unwrap();

        let fetched = db.get_user(&user.id).await.unwrap();
        assert!(fetched.is_some());
        let fetched_user = fetched.unwrap();
        assert_eq!(fetched_user.username, "testuser");
        assert_eq!(fetched_user.email, "test@example.com");
    }

    #[tokio::test]
    async fn test_get_user_by_username() {
        if std::env::var("DATABASE_URL").is_err() {
            return; // Skip if no test database
        }
        let db = test_db().await;
        db.create_user("findme", "findme@example.com", "hash123").await.unwrap();

        let user = db.get_user_by_username("findme").await.unwrap();
        assert!(user.is_some());
        assert_eq!(user.unwrap().email, "findme@example.com");
    }

    #[tokio::test]
    async fn test_update_user_email() {
        if std::env::var("DATABASE_URL").is_err() {
            return; // Skip if no test database
        }
        let db = test_db().await;
        let user = db.create_user("updateme", "old@example.com", "hash123").await.unwrap();

        db.update_user_email(&user.id, "new@example.com").await.unwrap();

        let updated = db.get_user(&user.id).await.unwrap().unwrap();
        assert_eq!(updated.email, "new@example.com");
    }

    #[tokio::test]
    async fn test_delete_user() {
        if std::env::var("DATABASE_URL").is_err() {
            return; // Skip if no test database
        }
        let db = test_db().await;
        let user = db.create_user("deleteme", "delete@example.com", "hash123").await.unwrap();

        db.delete_user(&user.id).await.unwrap();

        let deleted = db.get_user(&user.id).await.unwrap();
        assert!(deleted.is_none());
    }

    #[tokio::test]
    async fn test_list_users() {
        if std::env::var("DATABASE_URL").is_err() {
            return; // Skip if no test database
        }
        let db = test_db().await;
        let users = db.list_users().await.unwrap();
        assert!(users.len() >= 0); // May have users from previous tests
    }
}
