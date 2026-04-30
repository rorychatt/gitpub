use gitpub_core::{Database, Repository, User};
use testcontainers::{clients::Cli, core::WaitFor, GenericImage};

async fn setup_test_db() -> (Cli, testcontainers::Container<'static, GenericImage>, Database) {
    let docker = Cli::default();
    let postgres_image = GenericImage::new("postgres", "16-alpine")
        .with_exposed_port(5432)
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", "gitpub_test")
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ));

    let container = docker.run(postgres_image);
    let port = container.get_host_port_ipv4(5432);
    let db_url = format!("postgresql://postgres:postgres@127.0.0.1:{}/gitpub_test", port);

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let db = Database::new(&db_url).await.expect("Failed to connect to test database");

    (docker, container, db)
}

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
    let user = User::new("testuser".to_string(), "test@example.com".to_string(), "hash123".to_string());
    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "test@example.com");
    assert_eq!(user.password_hash, "hash123");
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
    let user1 = User::new("user1".to_string(), "user1@example.com".to_string(), "hash1".to_string());
    let user2 = User::new("user2".to_string(), "user2@example.com".to_string(), "hash2".to_string());
    assert_ne!(user1.id, user2.id);
}

#[tokio::test]
async fn test_user_insert_and_retrieve() {
    let (_docker, _container, db) = setup_test_db().await;

    let user = User::new("testuser".to_string(), "test@example.com".to_string(), "hash123".to_string());
    db.insert_user(&user).await.expect("Failed to insert user");

    let fetched = db.get_user_by_id(&user.id).await.expect("Query failed");
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.username, "testuser");
    assert_eq!(fetched.email, "test@example.com");
    assert_eq!(fetched.password_hash, "hash123");
}

#[tokio::test]
async fn test_duplicate_username_constraint() {
    let (_docker, _container, db) = setup_test_db().await;

    let user1 = User::new("testuser".to_string(), "test1@example.com".to_string(), "hash1".to_string());
    db.insert_user(&user1).await.expect("Failed to insert first user");

    let user2 = User::new("testuser".to_string(), "test2@example.com".to_string(), "hash2".to_string());
    let result = db.insert_user(&user2).await;

    assert!(result.is_err(), "Expected duplicate username to fail");
}

#[tokio::test]
async fn test_duplicate_email_constraint() {
    let (_docker, _container, db) = setup_test_db().await;

    let user1 = User::new("testuser1".to_string(), "test@example.com".to_string(), "hash1".to_string());
    db.insert_user(&user1).await.expect("Failed to insert first user");

    let user2 = User::new("testuser2".to_string(), "test@example.com".to_string(), "hash2".to_string());
    let result = db.insert_user(&user2).await;

    assert!(result.is_err(), "Expected duplicate email to fail");
}

#[tokio::test]
async fn test_repository_persistence() {
    let (_docker, _container, db) = setup_test_db().await;

    let mut repo = Repository::new("test-repo".to_string(), "test-owner".to_string());
    repo.description = Some("A test repository".to_string());
    repo.is_private = true;

    db.insert_repository(&repo).await.expect("Failed to insert repository");

    let fetched = db.get_repository_by_id(&repo.id).await.expect("Query failed");
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.name, "test-repo");
    assert_eq!(fetched.owner, "test-owner");
    assert_eq!(fetched.description, Some("A test repository".to_string()));
    assert!(fetched.is_private);
    assert_eq!(fetched.default_branch, "main");
}
