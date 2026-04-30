use gitpub_core::{Commit, Repository, User};

pub fn test_repository(name: &str, owner: &str) -> Repository {
    Repository::new(name.to_string(), owner.to_string())
}

pub fn test_user(username: &str, email: &str) -> User {
    User::new(
        username.to_string(),
        email.to_string(),
        "test_hash".to_string(),
    )
}

pub fn test_commit(sha: &str, message: &str, repository_id: &str) -> Commit {
    Commit {
        sha: sha.to_string(),
        repository_id: repository_id.to_string(),
        message: message.to_string(),
        author: "test-author".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    }
}
