use gitpub_core::{Database, Repository, User};

mod helpers;
use helpers::TestDatabase;
use testcontainers::clients::Cli;

#[test]
fn test_repository_creation() {
    let repo = Repository::new("test-repo".to_string(), "test-owner".to_string());
    assert_eq!(repo.name, "test-repo");
    assert_eq!(repo.owner, "test-owner");
    assert_eq!(repo.default_branch, "main");
    assert!(!repo.is_private);
    assert!(repo.description.is_none());
}

#[test]
fn test_repository_with_description() {
    let mut repo = Repository::new("test-repo".to_string(), "test-owner".to_string());
    repo.description = Some("A test repository".to_string());
    assert_eq!(repo.description, Some("A test repository".to_string()));
}

#[test]
fn test_user_creation() {
    let user = User::new("testuser".to_string(), "test@example.com".to_string());
    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "test@example.com");
    assert!(!user.id.is_empty());
}

#[test]
fn test_multiple_repositories_have_unique_ids() {
    let repo1 = Repository::new("repo1".to_string(), "owner1".to_string());
    let repo2 = Repository::new("repo2".to_string(), "owner2".to_string());
    assert_ne!(repo1.id, repo2.id);
}

#[test]
fn test_multiple_users_have_unique_ids() {
    let user1 = User::new("user1".to_string(), "user1@example.com".to_string());
    let user2 = User::new("user2".to_string(), "user2@example.com".to_string());
    assert_ne!(user1.id, user2.id);
}

#[tokio::test]
async fn test_database_connection() {
    let docker = Cli::default();
    let test_db = TestDatabase::new(&docker).await;

    let query_result = sqlx::query("SELECT 1").fetch_one(test_db.db.pool()).await;
    assert!(query_result.is_ok());
}
