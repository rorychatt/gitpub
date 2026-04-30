use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Core repository representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::FromRow)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    async fn test_user_crud() {
        // Requires a test database URL in environment
        let db_url = match std::env::var("DATABASE_URL") {
            Ok(url) => url,
            Err(_) => return, // Skip if DATABASE_URL not set
        };

        let db = Database::new(&db_url)
            .await
            .expect("Database connection should succeed");

        let test_id = uuid::Uuid::new_v4().to_string();
        let test_username = format!("testuser_{}", uuid::Uuid::new_v4());
        let test_email = format!("test_{}@example.com", uuid::Uuid::new_v4());
        let created_at = chrono::Utc::now().timestamp();

        // Insert a user
        sqlx::query("INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)")
            .bind(&test_id)
            .bind(&test_username)
            .bind(&test_email)
            .bind(created_at)
            .execute(db.pool())
            .await
            .expect("User insertion should succeed");

        // Query it back
        let user: (String, String, String, i64) =
            sqlx::query_as("SELECT id, username, email, created_at FROM users WHERE id = $1")
                .bind(&test_id)
                .fetch_one(db.pool())
                .await
                .expect("User query should succeed");

        assert_eq!(user.0, test_id);
        assert_eq!(user.1, test_username);
        assert_eq!(user.2, test_email);
        assert_eq!(user.3, created_at);

        // Test unique constraint on username
        let duplicate_username_result = sqlx::query(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&test_username)
        .bind(format!("different_{}@example.com", uuid::Uuid::new_v4()))
        .bind(chrono::Utc::now().timestamp())
        .execute(db.pool())
        .await;

        assert!(
            duplicate_username_result.is_err(),
            "Duplicate username should fail"
        );

        // Test unique constraint on email
        let duplicate_email_result = sqlx::query(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(format!("different_user_{}", uuid::Uuid::new_v4()))
        .bind(&test_email)
        .bind(chrono::Utc::now().timestamp())
        .execute(db.pool())
        .await;

        assert!(
            duplicate_email_result.is_err(),
            "Duplicate email should fail"
        );

        // Cleanup - delete the user
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&test_id)
            .execute(db.pool())
            .await
            .expect("User deletion should succeed");

        // Verify deletion
        let deleted_user: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1")
            .bind(&test_id)
            .fetch_optional(db.pool())
            .await
            .expect("Query should succeed");

        assert!(deleted_user.is_none(), "User should be deleted");
    }

    #[tokio::test]
    async fn test_repository_crud() {
        // Requires a test database URL in environment
        let db_url = match std::env::var("DATABASE_URL") {
            Ok(url) => url,
            Err(_) => return, // Skip if DATABASE_URL not set
        };

        let db = Database::new(&db_url)
            .await
            .expect("Database connection should succeed");

        // Create a user first (foreign key requirement)
        let user_id = uuid::Uuid::new_v4().to_string();
        let username = format!("repoowner_{}", uuid::Uuid::new_v4());
        let email = format!("owner_{}@example.com", uuid::Uuid::new_v4());

        sqlx::query("INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)")
            .bind(&user_id)
            .bind(&username)
            .bind(&email)
            .bind(chrono::Utc::now().timestamp())
            .execute(db.pool())
            .await
            .expect("User insertion should succeed");

        // Insert a repository
        let repo_id = uuid::Uuid::new_v4().to_string();
        let repo_name = format!("test-repo-{}", uuid::Uuid::new_v4());
        let description = "Test repository description";
        let is_private = false;
        let default_branch = "main";
        let created_at = chrono::Utc::now().timestamp();

        sqlx::query("INSERT INTO repositories (id, name, owner, description, is_private, default_branch, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(&repo_id)
            .bind(&repo_name)
            .bind(&user_id)
            .bind(description)
            .bind(is_private)
            .bind(default_branch)
            .bind(created_at)
            .execute(db.pool())
            .await
            .expect("Repository insertion should succeed");

        // Query it back with JOIN to verify owner relationship
        let repo: (String, String, String, Option<String>, bool, String, i64, String) =
            sqlx::query_as(
                r#"
            SELECT r.id, r.name, r.owner, r.description, r.is_private, r.default_branch, r.created_at, u.username
            FROM repositories r
            JOIN users u ON r.owner = u.id
            WHERE r.id = $1
            "#,
            )
            .bind(&repo_id)
            .fetch_one(db.pool())
            .await
            .expect("Repository query should succeed");

        assert_eq!(repo.0, repo_id);
        assert_eq!(repo.1, repo_name);
        assert_eq!(repo.2, user_id);
        assert_eq!(repo.3.as_deref(), Some(description));
        assert_eq!(repo.4, is_private);
        assert_eq!(repo.5, default_branch);
        assert_eq!(repo.6, created_at);
        assert_eq!(repo.7, username);

        // Test unique constraint: same owner can't create duplicate repo names
        let duplicate_repo_result = sqlx::query("INSERT INTO repositories (id, name, owner, description, is_private, default_branch, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&repo_name)
            .bind(&user_id)
            .bind(description)
            .bind(is_private)
            .bind(default_branch)
            .bind(chrono::Utc::now().timestamp())
            .execute(db.pool())
            .await;

        assert!(
            duplicate_repo_result.is_err(),
            "Duplicate repository name for same owner should fail"
        );

        // Test CASCADE DELETE: deleting user should delete their repositories
        let repo_count_before: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM repositories WHERE owner = $1")
                .bind(&user_id)
                .fetch_one(db.pool())
                .await
                .expect("Query should succeed");

        assert_eq!(repo_count_before.0, 1, "Should have one repository");

        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&user_id)
            .execute(db.pool())
            .await
            .expect("User deletion should succeed");

        let repo_count_after: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM repositories WHERE owner = $1")
                .bind(&user_id)
                .fetch_one(db.pool())
                .await
                .expect("Query should succeed");

        assert_eq!(
            repo_count_after.0, 0,
            "Repository should be cascade deleted when user is deleted"
        );
    }

    #[tokio::test]
    async fn test_commit_crud() {
        // Requires a test database URL in environment
        let db_url = match std::env::var("DATABASE_URL") {
            Ok(url) => url,
            Err(_) => return, // Skip if DATABASE_URL not set
        };

        let db = Database::new(&db_url)
            .await
            .expect("Database connection should succeed");

        // Create user first
        let user_id = uuid::Uuid::new_v4().to_string();
        let username = format!("commitauthor_{}", uuid::Uuid::new_v4());
        let email = format!("author_{}@example.com", uuid::Uuid::new_v4());

        sqlx::query("INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)")
            .bind(&user_id)
            .bind(&username)
            .bind(&email)
            .bind(chrono::Utc::now().timestamp())
            .execute(db.pool())
            .await
            .expect("User insertion should succeed");

        // Create repository
        let repo_id = uuid::Uuid::new_v4().to_string();
        let repo_name = format!("commit-test-repo-{}", uuid::Uuid::new_v4());

        sqlx::query("INSERT INTO repositories (id, name, owner, is_private, default_branch, created_at) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(&repo_id)
            .bind(&repo_name)
            .bind(&user_id)
            .bind(false)
            .bind("main")
            .bind(chrono::Utc::now().timestamp())
            .execute(db.pool())
            .await
            .expect("Repository insertion should succeed");

        // Insert commits with foreign keys
        let commit1_sha = format!("{:040x}", uuid::Uuid::new_v4().as_u128());
        let commit1_message = "First commit";
        let commit1_timestamp = chrono::Utc::now().timestamp() - 100;

        sqlx::query("INSERT INTO commits (sha, repository_id, message, author, timestamp) VALUES ($1, $2, $3, $4, $5)")
            .bind(&commit1_sha)
            .bind(&repo_id)
            .bind(commit1_message)
            .bind(&user_id)
            .bind(commit1_timestamp)
            .execute(db.pool())
            .await
            .expect("Commit insertion should succeed");

        let commit2_sha = format!("{:040x}", uuid::Uuid::new_v4().as_u128());
        let commit2_message = "Second commit";
        let commit2_timestamp = chrono::Utc::now().timestamp();

        sqlx::query("INSERT INTO commits (sha, repository_id, message, author, timestamp) VALUES ($1, $2, $3, $4, $5)")
            .bind(&commit2_sha)
            .bind(&repo_id)
            .bind(commit2_message)
            .bind(&user_id)
            .bind(commit2_timestamp)
            .execute(db.pool())
            .await
            .expect("Commit insertion should succeed");

        // Query commits by repository_id
        let commits: Vec<(String, String, String, i64)> =
            sqlx::query_as("SELECT sha, message, author, timestamp FROM commits WHERE repository_id = $1 ORDER BY timestamp ASC")
                .bind(&repo_id)
                .fetch_all(db.pool())
                .await
                .expect("Commit query should succeed");

        assert_eq!(commits.len(), 2, "Should have two commits");
        assert_eq!(commits[0].0, commit1_sha);
        assert_eq!(commits[0].1, commit1_message);
        assert_eq!(commits[1].0, commit2_sha);
        assert_eq!(commits[1].1, commit2_message);

        // Query commits by author
        let author_commits: Vec<(String,)> =
            sqlx::query_as("SELECT sha FROM commits WHERE author = $1")
                .bind(&user_id)
                .fetch_all(db.pool())
                .await
                .expect("Author query should succeed");

        assert_eq!(author_commits.len(), 2, "Author should have two commits");

        // Verify timestamp ordering
        assert!(
            commits[0].3 < commits[1].3,
            "Commits should be ordered by timestamp"
        );

        // Test CASCADE DELETE: deleting repository should delete its commits
        let commit_count_before: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM commits WHERE repository_id = $1")
                .bind(&repo_id)
                .fetch_one(db.pool())
                .await
                .expect("Query should succeed");

        assert_eq!(commit_count_before.0, 2, "Should have two commits");

        sqlx::query("DELETE FROM repositories WHERE id = $1")
            .bind(&repo_id)
            .execute(db.pool())
            .await
            .expect("Repository deletion should succeed");

        let commit_count_after: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM commits WHERE repository_id = $1")
                .bind(&repo_id)
                .fetch_one(db.pool())
                .await
                .expect("Query should succeed");

        assert_eq!(
            commit_count_after.0, 0,
            "Commits should be cascade deleted when repository is deleted"
        );

        // Cleanup user
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&user_id)
            .execute(db.pool())
            .await
            .expect("User deletion should succeed");
    }

    #[tokio::test]
    #[ignore]
    async fn test_insert_and_get_user() {
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        let user = User::new(
            "testuser".to_string(),
            "test@example.com".to_string(),
            "hash123".to_string(),
        );
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
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        let user = User::new(
            "repoowner".to_string(),
            "owner@example.com".to_string(),
            "hash123".to_string(),
        );
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
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        for i in 0..5 {
            let user = User::new(
                format!("user{}", i),
                format!("user{}@example.com", i),
                "hash".to_string(),
            );
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
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
        let db = Database::new(&database_url).await.unwrap();

        let user1 = User::new(
            "owner1".to_string(),
            "owner1@example.com".to_string(),
            "hash".to_string(),
        );
        let user2 = User::new(
            "owner2".to_string(),
            "owner2@example.com".to_string(),
            "hash".to_string(),
        );
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
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
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
