use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Core repository representation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
    pub repository_id: String,
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

    // User operations
    pub async fn insert_user(&self, user: &User) -> Result<()> {
        sqlx::query("INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)")
            .bind(&user.id)
            .bind(&user.username)
            .bind(&user.email)
            .bind(user.created_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_user_by_id(&self, id: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, username, email, created_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, username, email, created_at FROM users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT id, username, email, created_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    pub async fn list_users(&self, limit: Option<i64>, offset: Option<i64>) -> Result<Vec<User>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let users = sqlx::query_as::<_, User>(
            "SELECT id, username, email, created_at FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(users)
    }

    // Repository operations
    pub async fn insert_repository(&self, repo: &Repository) -> Result<()> {
        sqlx::query(
            "INSERT INTO repositories (id, name, owner, description, is_private, default_branch, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&repo.id)
        .bind(&repo.name)
        .bind(&repo.owner)
        .bind(&repo.description)
        .bind(repo.is_private)
        .bind(&repo.default_branch)
        .bind(repo.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_repository_by_id(&self, id: &str) -> Result<Option<Repository>> {
        let repo = sqlx::query_as::<_, Repository>(
            "SELECT id, name, owner, description, is_private, default_branch, created_at FROM repositories WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(repo)
    }

    pub async fn get_repository_by_owner_and_name(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<Option<Repository>> {
        let repo = sqlx::query_as::<_, Repository>(
            "SELECT id, name, owner, description, is_private, default_branch, created_at FROM repositories WHERE owner = $1 AND name = $2",
        )
        .bind(owner)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        Ok(repo)
    }

    pub async fn list_repositories_by_owner(&self, owner: &str) -> Result<Vec<Repository>> {
        let repos = sqlx::query_as::<_, Repository>(
            "SELECT id, name, owner, description, is_private, default_branch, created_at FROM repositories WHERE owner = $1 ORDER BY created_at DESC",
        )
        .bind(owner)
        .fetch_all(&self.pool)
        .await?;
        Ok(repos)
    }

    pub async fn list_repositories(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Repository>> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);
        let repos = sqlx::query_as::<_, Repository>(
            "SELECT id, name, owner, description, is_private, default_branch, created_at FROM repositories ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(repos)
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

    #[tokio::test]
    #[ignore]
    async fn test_insert_and_get_user() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        let user = User::new("testuser".to_string(), "test@example.com".to_string(), "hashed_password".to_string());
        db.insert_user(&user).await.unwrap();

        let fetched_by_id = db.get_user_by_id(&user.id).await.unwrap();
        assert!(fetched_by_id.is_some());
        assert_eq!(fetched_by_id.unwrap().username, "testuser");

        let fetched_by_username = db.get_user_by_username("testuser").await.unwrap();
        assert!(fetched_by_username.is_some());
        assert_eq!(fetched_by_username.unwrap().email, "test@example.com");

        let fetched_by_email = db.get_user_by_email("test@example.com").await.unwrap();
        assert!(fetched_by_email.is_some());
        assert_eq!(fetched_by_email.unwrap().username, "testuser");
    }

    #[tokio::test]
    #[ignore]
    async fn test_insert_and_get_repository() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        let user = User::new("repoowner".to_string(), "owner@example.com".to_string(), "hashed_password".to_string());
        db.insert_user(&user).await.unwrap();

        let mut repo = Repository::new("test-repo".to_string(), user.id.clone());
        repo.description = Some("A test repository".to_string());
        db.insert_repository(&repo).await.unwrap();

        let fetched_by_id = db.get_repository_by_id(&repo.id).await.unwrap();
        assert!(fetched_by_id.is_some());
        assert_eq!(fetched_by_id.unwrap().name, "test-repo");

        let fetched_by_owner_name = db
            .get_repository_by_owner_and_name(&user.id, "test-repo")
            .await
            .unwrap();
        assert!(fetched_by_owner_name.is_some());
        assert_eq!(
            fetched_by_owner_name.unwrap().description,
            Some("A test repository".to_string())
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_users_pagination() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        for i in 0..5 {
            let user = User::new(format!("user{}", i), format!("user{}@example.com", i), "hashed_password".to_string());
            db.insert_user(&user).await.unwrap();
        }

        let users_page1 = db.list_users(Some(2), Some(0)).await.unwrap();
        assert_eq!(users_page1.len(), 2);

        let users_page2 = db.list_users(Some(2), Some(2)).await.unwrap();
        assert_eq!(users_page2.len(), 2);
    }

    #[tokio::test]
    #[ignore]
    async fn test_list_repositories_by_owner() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        let user1 = User::new("owner1".to_string(), "owner1@example.com".to_string(), "hashed_password".to_string());
        let user2 = User::new("owner2".to_string(), "owner2@example.com".to_string(), "hashed_password".to_string());
        db.insert_user(&user1).await.unwrap();
        db.insert_user(&user2).await.unwrap();

        let repo1 = Repository::new("repo1".to_string(), user1.id.clone());
        let repo2 = Repository::new("repo2".to_string(), user1.id.clone());
        let repo3 = Repository::new("repo3".to_string(), user2.id.clone());
        db.insert_repository(&repo1).await.unwrap();
        db.insert_repository(&repo2).await.unwrap();
        db.insert_repository(&repo3).await.unwrap();

        let owner1_repos = db.list_repositories_by_owner(&user1.id).await.unwrap();
        assert_eq!(owner1_repos.len(), 2);

        let owner2_repos = db.list_repositories_by_owner(&user2.id).await.unwrap();
        assert_eq!(owner2_repos.len(), 1);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_nonexistent_records() {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        let user = db.get_user_by_id("nonexistent-id").await.unwrap();
        assert!(user.is_none());

        let repo = db.get_repository_by_id("nonexistent-id").await.unwrap();
        assert!(repo.is_none());

        let repo_by_owner = db
            .get_repository_by_owner_and_name("nonexistent-owner", "nonexistent-repo")
            .await
            .unwrap();
        assert!(repo_by_owner.is_none());
    }
}
