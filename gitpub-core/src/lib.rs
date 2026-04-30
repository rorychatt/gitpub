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
}
