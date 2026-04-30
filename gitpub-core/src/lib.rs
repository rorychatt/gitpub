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
        sqlx::query!(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
            test_id,
            test_username,
            test_email,
            created_at
        )
        .execute(db.pool())
        .await
        .expect("User insertion should succeed");

        // Query it back
        let user = sqlx::query!(
            "SELECT id, username, email, created_at FROM users WHERE id = $1",
            test_id
        )
        .fetch_one(db.pool())
        .await
        .expect("User query should succeed");

        assert_eq!(user.id, test_id);
        assert_eq!(user.username, test_username);
        assert_eq!(user.email, test_email);
        assert_eq!(user.created_at, created_at);

        // Test unique constraint on username
        let duplicate_username_result = sqlx::query!(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
            uuid::Uuid::new_v4().to_string(),
            test_username,
            format!("different_{}@example.com", uuid::Uuid::new_v4()),
            chrono::Utc::now().timestamp()
        )
        .execute(db.pool())
        .await;

        assert!(
            duplicate_username_result.is_err(),
            "Duplicate username should fail"
        );

        // Test unique constraint on email
        let duplicate_email_result = sqlx::query!(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
            uuid::Uuid::new_v4().to_string(),
            format!("different_user_{}", uuid::Uuid::new_v4()),
            test_email,
            chrono::Utc::now().timestamp()
        )
        .execute(db.pool())
        .await;

        assert!(
            duplicate_email_result.is_err(),
            "Duplicate email should fail"
        );

        // Cleanup - delete the user
        sqlx::query!("DELETE FROM users WHERE id = $1", test_id)
            .execute(db.pool())
            .await
            .expect("User deletion should succeed");

        // Verify deletion
        let deleted_user = sqlx::query!("SELECT id FROM users WHERE id = $1", test_id)
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

        sqlx::query!(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
            user_id,
            username,
            email,
            chrono::Utc::now().timestamp()
        )
        .execute(db.pool())
        .await
        .expect("User insertion should succeed");

        // Insert a repository
        let repo_id = uuid::Uuid::new_v4().to_string();
        let repo_name = format!("test-repo-{}", uuid::Uuid::new_v4());
        let description = Some("Test repository description");
        let is_private = false;
        let default_branch = "main";
        let created_at = chrono::Utc::now().timestamp();

        sqlx::query!(
            "INSERT INTO repositories (id, name, owner, description, is_private, default_branch, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            repo_id,
            repo_name,
            user_id,
            description,
            is_private,
            default_branch,
            created_at
        )
        .execute(db.pool())
        .await
        .expect("Repository insertion should succeed");

        // Query it back with JOIN to verify owner relationship
        let repo = sqlx::query!(
            r#"
            SELECT r.id, r.name, r.owner, r.description, r.is_private, r.default_branch, r.created_at, u.username
            FROM repositories r
            JOIN users u ON r.owner = u.id
            WHERE r.id = $1
            "#,
            repo_id
        )
        .fetch_one(db.pool())
        .await
        .expect("Repository query should succeed");

        assert_eq!(repo.id, repo_id);
        assert_eq!(repo.name, repo_name);
        assert_eq!(repo.owner, user_id);
        assert_eq!(repo.description, description);
        assert_eq!(repo.is_private, is_private);
        assert_eq!(repo.default_branch, default_branch);
        assert_eq!(repo.created_at, created_at);
        assert_eq!(repo.username, username);

        // Test unique constraint: same owner can't create duplicate repo names
        let duplicate_repo_result = sqlx::query!(
            "INSERT INTO repositories (id, name, owner, description, is_private, default_branch, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            uuid::Uuid::new_v4().to_string(),
            repo_name,
            user_id,
            description,
            is_private,
            default_branch,
            chrono::Utc::now().timestamp()
        )
        .execute(db.pool())
        .await;

        assert!(
            duplicate_repo_result.is_err(),
            "Duplicate repository name for same owner should fail"
        );

        // Test CASCADE DELETE: deleting user should delete their repositories
        let repo_count_before = sqlx::query!(
            "SELECT COUNT(*) as count FROM repositories WHERE owner = $1",
            user_id
        )
        .fetch_one(db.pool())
        .await
        .expect("Query should succeed")
        .count
        .unwrap_or(0);

        assert_eq!(repo_count_before, 1, "Should have one repository");

        sqlx::query!("DELETE FROM users WHERE id = $1", user_id)
            .execute(db.pool())
            .await
            .expect("User deletion should succeed");

        let repo_count_after = sqlx::query!(
            "SELECT COUNT(*) as count FROM repositories WHERE owner = $1",
            user_id
        )
        .fetch_one(db.pool())
        .await
        .expect("Query should succeed")
        .count
        .unwrap_or(0);

        assert_eq!(
            repo_count_after, 0,
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

        sqlx::query!(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, $4)",
            user_id,
            username,
            email,
            chrono::Utc::now().timestamp()
        )
        .execute(db.pool())
        .await
        .expect("User insertion should succeed");

        // Create repository
        let repo_id = uuid::Uuid::new_v4().to_string();
        let repo_name = format!("commit-test-repo-{}", uuid::Uuid::new_v4());

        sqlx::query!(
            "INSERT INTO repositories (id, name, owner, is_private, default_branch, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
            repo_id,
            repo_name,
            user_id,
            false,
            "main",
            chrono::Utc::now().timestamp()
        )
        .execute(db.pool())
        .await
        .expect("Repository insertion should succeed");

        // Insert commits with foreign keys
        let commit1_sha = format!("{:040x}", uuid::Uuid::new_v4().as_u128());
        let commit1_message = "First commit";
        let commit1_timestamp = chrono::Utc::now().timestamp() - 100;

        sqlx::query!(
            "INSERT INTO commits (sha, repository_id, message, author, timestamp) VALUES ($1, $2, $3, $4, $5)",
            commit1_sha,
            repo_id,
            commit1_message,
            user_id,
            commit1_timestamp
        )
        .execute(db.pool())
        .await
        .expect("Commit insertion should succeed");

        let commit2_sha = format!("{:040x}", uuid::Uuid::new_v4().as_u128());
        let commit2_message = "Second commit";
        let commit2_timestamp = chrono::Utc::now().timestamp();

        sqlx::query!(
            "INSERT INTO commits (sha, repository_id, message, author, timestamp) VALUES ($1, $2, $3, $4, $5)",
            commit2_sha,
            repo_id,
            commit2_message,
            user_id,
            commit2_timestamp
        )
        .execute(db.pool())
        .await
        .expect("Commit insertion should succeed");

        // Query commits by repository_id
        let commits = sqlx::query!(
            "SELECT sha, message, author, timestamp FROM commits WHERE repository_id = $1 ORDER BY timestamp ASC",
            repo_id
        )
        .fetch_all(db.pool())
        .await
        .expect("Commit query should succeed");

        assert_eq!(commits.len(), 2, "Should have two commits");
        assert_eq!(commits[0].sha, commit1_sha);
        assert_eq!(commits[0].message, commit1_message);
        assert_eq!(commits[1].sha, commit2_sha);
        assert_eq!(commits[1].message, commit2_message);

        // Query commits by author
        let author_commits = sqlx::query!("SELECT sha FROM commits WHERE author = $1", user_id)
            .fetch_all(db.pool())
            .await
            .expect("Author query should succeed");

        assert_eq!(author_commits.len(), 2, "Author should have two commits");

        // Verify timestamp ordering
        assert!(
            commits[0].timestamp < commits[1].timestamp,
            "Commits should be ordered by timestamp"
        );

        // Test CASCADE DELETE: deleting repository should delete its commits
        let commit_count_before = sqlx::query!(
            "SELECT COUNT(*) as count FROM commits WHERE repository_id = $1",
            repo_id
        )
        .fetch_one(db.pool())
        .await
        .expect("Query should succeed")
        .count
        .unwrap_or(0);

        assert_eq!(commit_count_before, 2, "Should have two commits");

        sqlx::query!("DELETE FROM repositories WHERE id = $1", repo_id)
            .execute(db.pool())
            .await
            .expect("Repository deletion should succeed");

        let commit_count_after = sqlx::query!(
            "SELECT COUNT(*) as count FROM commits WHERE repository_id = $1",
            repo_id
        )
        .fetch_one(db.pool())
        .await
        .expect("Query should succeed")
        .count
        .unwrap_or(0);

        assert_eq!(
            commit_count_after, 0,
            "Commits should be cascade deleted when repository is deleted"
        );

        // Cleanup user
        sqlx::query!("DELETE FROM users WHERE id = $1", user_id)
            .execute(db.pool())
            .await
            .expect("User deletion should succeed");
    }
}
