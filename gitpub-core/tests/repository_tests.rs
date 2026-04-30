mod common;

use gitpub_core::Repository;

#[test]
fn test_repository_has_valid_uuid() {
    let repo = common::test_repository("my-repo", "alice");
    assert!(!repo.id.is_empty());
    assert!(uuid::Uuid::parse_str(&repo.id).is_ok());
}

#[test]
fn test_repository_default_values() {
    let repo = common::test_repository("my-repo", "alice");
    assert_eq!(repo.name, "my-repo");
    assert_eq!(repo.owner, "alice");
    assert_eq!(repo.default_branch, "main");
    assert!(!repo.is_private);
    assert!(repo.description.is_none());
    assert!(repo.created_at > 0);
}

#[test]
fn test_repository_unique_ids() {
    let repo1 = common::test_repository("repo-a", "alice");
    let repo2 = common::test_repository("repo-b", "bob");
    assert_ne!(repo1.id, repo2.id);
}

#[test]
fn test_repository_serialization_roundtrip() {
    let repo = common::test_repository("my-repo", "alice");
    let json = serde_json::to_string(&repo).unwrap();
    let deserialized: Repository = serde_json::from_str(&json).unwrap();
    assert_eq!(repo, deserialized);
}

#[test]
fn test_repository_with_special_characters() {
    let repo = common::test_repository("my-repo-123_test", "user.name");
    assert_eq!(repo.name, "my-repo-123_test");
    assert_eq!(repo.owner, "user.name");
}

#[test]
fn test_user_has_valid_uuid() {
    let user = common::test_user("alice", "alice@example.com");
    assert!(!user.id.is_empty());
    assert!(uuid::Uuid::parse_str(&user.id).is_ok());
}

#[test]
fn test_user_unique_ids() {
    let user1 = common::test_user("alice", "alice@example.com");
    let user2 = common::test_user("bob", "bob@example.com");
    assert_ne!(user1.id, user2.id);
}

#[test]
fn test_user_serialization_roundtrip() {
    let user = common::test_user("alice", "alice@example.com");
    let json = serde_json::to_string(&user).unwrap();
    let deserialized: gitpub_core::User = serde_json::from_str(&json).unwrap();
    assert_eq!(user, deserialized);
}

#[test]
fn test_commit_creation() {
    let commit = common::test_commit("abc123", "Initial commit");
    assert_eq!(commit.sha, "abc123");
    assert_eq!(commit.message, "Initial commit");
    assert_eq!(commit.author, "test-author");
    assert!(commit.timestamp > 0);
}

#[test]
fn test_commit_serialization_roundtrip() {
    let commit = common::test_commit("def456", "Add feature");
    let json = serde_json::to_string(&commit).unwrap();
    let deserialized: gitpub_core::Commit = serde_json::from_str(&json).unwrap();
    assert_eq!(commit, deserialized);
}
