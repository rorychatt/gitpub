use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::AppState;

#[derive(Deserialize)]
pub struct InfoRefsQuery {
    service: Option<String>,
}

fn resolve_repo_path(state: &AppState, owner: &str, repo: &str) -> PathBuf {
    let repo_name = repo.strip_suffix(".git").unwrap_or(repo);
    state
        .repos_path
        .join(owner)
        .join(format!("{}.git", repo_name))
}

fn pkt_line(data: &str) -> Vec<u8> {
    let len = data.len() + 4;
    format!("{:04x}{}", len, data).into_bytes()
}

pub async fn handle_info_refs(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
    Query(query): Query<InfoRefsQuery>,
) -> Response {
    let service = match &query.service {
        Some(s) if s == "git-upload-pack" || s == "git-receive-pack" => s.clone(),
        Some(_) => return (StatusCode::BAD_REQUEST, "Invalid service").into_response(),
        None => return (StatusCode::BAD_REQUEST, "Service parameter required").into_response(),
    };

    let path = resolve_repo_path(&state, &owner, &repo);
    if git2::Repository::open_bare(&path).is_err() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    let git_cmd = service.strip_prefix("git-").unwrap_or(&service);
    let result = Command::new("git")
        .arg(git_cmd)
        .arg("--stateless-rpc")
        .arg("--advertise-refs")
        .arg(&path)
        .output()
        .await;

    let output = match result {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            tracing::error!(
                "git {} failed: {}",
                git_cmd,
                String::from_utf8_lossy(&o.stderr)
            );
            return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
        }
        Err(e) => {
            tracing::error!("Failed to spawn git: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to execute git").into_response();
        }
    };

    let content_type = format!("application/x-{}-advertisement", service);
    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(&format!("# service={}\n", service)));
    body.extend_from_slice(b"0000");
    body.extend_from_slice(&output);

    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header("Cache-Control", "no-cache")
        .body(Body::from(body))
        .unwrap()
}

pub async fn handle_upload_pack(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    service_rpc(&state, &owner, &repo, "upload-pack", &body).await
}

pub async fn handle_receive_pack(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
    auth: crate::auth::RequireGitAuth,
    body: Bytes,
) -> Response {
    if auth.username != owner {
        return (
            StatusCode::FORBIDDEN,
            "You don't have permission to push to this repository",
        )
            .into_response();
    }

    service_rpc(&state, &owner, &repo, "receive-pack", &body).await
}

async fn service_rpc(
    state: &AppState,
    owner: &str,
    repo: &str,
    service: &str,
    input: &[u8],
) -> Response {
    let path = resolve_repo_path(state, owner, repo);
    if git2::Repository::open_bare(&path).is_err() {
        return (StatusCode::NOT_FOUND, "Repository not found").into_response();
    }

    let mut child = match Command::new("git")
        .arg(service)
        .arg("--stateless-rpc")
        .arg(&path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to spawn git {}: {}", service, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to execute git").into_response();
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(input).await {
            tracing::error!("Failed to write to git stdin: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    }

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("git {} failed: {}", service, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
        }
    };

    if !output.status.success() {
        tracing::error!(
            "git {} exited {}: {}",
            service,
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        return (StatusCode::INTERNAL_SERVER_ERROR, "Git command failed").into_response();
    }

    Response::builder()
        .header(
            header::CONTENT_TYPE,
            format!("application/x-git-{}-result", service),
        )
        .header("Cache-Control", "no-cache")
        .body(Body::from(output.stdout))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get, post};
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn create_test_repo(base: &std::path::Path) -> PathBuf {
        let owner_dir = base.join("testowner");
        std::fs::create_dir_all(&owner_dir).unwrap();
        let repo_path = owner_dir.join("testrepo.git");
        let repo = git2::Repository::init_bare(&repo_path).unwrap();

        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        base.to_path_buf()
    }

    fn test_app(repos_path: PathBuf) -> Router {
        use std::collections::HashMap;
        use tokio::sync::RwLock;
        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(HashMap::new())),
            repos_path,
        });
        Router::new()
            .route("/:owner/:repo/info/refs", get(handle_info_refs))
            .route("/:owner/:repo/git-upload-pack", post(handle_upload_pack))
            .route("/:owner/:repo/git-receive-pack", post(handle_receive_pack))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_info_refs_upload_pack() {
        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());
        let app = test_app(repos_path);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/testrepo.git/info/refs?service=git-upload-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "application/x-git-upload-pack-advertisement"
        );

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains("# service=git-upload-pack"));
    }

    #[tokio::test]
    async fn test_info_refs_receive_pack() {
        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());
        let app = test_app(repos_path);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/testrepo.git/info/refs?service=git-receive-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "application/x-git-receive-pack-advertisement"
        );

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains("# service=git-receive-pack"));
    }

    #[tokio::test]
    async fn test_upload_pack_request() {
        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());
        let app = test_app(repos_path.clone());

        let repo_path = repos_path.join("testowner").join("testrepo.git");
        let repo = git2::Repository::open_bare(&repo_path).unwrap();
        let head_oid = repo.head().unwrap().target().unwrap();
        let sha = head_oid.to_string();

        let want = format!("want {}\n", sha);
        let pkt_len = want.len() + 4;
        let want_pkt = format!("{:04x}{}", pkt_len, want);
        let body_data = format!("{}00000009done\n", want_pkt);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo.git/git-upload-pack")
                    .header("Content-Type", "application/x-git-upload-pack-request")
                    .body(Body::from(body_data))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "application/x-git-upload-pack-result"
        );
    }

    #[tokio::test]
    async fn test_receive_pack_requires_auth() {
        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());
        let app = test_app(repos_path);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo.git/git-receive-pack")
                    .header("Content-Type", "application/x-git-receive-pack-request")
                    .body(Body::from("0000"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert!(resp
            .headers()
            .get("www-authenticate")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Basic realm=\"gitpub\""));
    }

    #[tokio::test]
    async fn test_repository_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_app(tmp.path().to_path_buf());

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/nonexistent/repo.git/info/refs?service=git-upload-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_malformed_request() {
        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());
        let app = test_app(repos_path);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/testrepo.git/info/refs?service=invalid-service")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_receive_pack_rejects_invalid_credentials() {
        use gitpub_core::User;
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        std::env::set_var(
            "JWT_SECRET",
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());

        let password_hash = crate::auth::hash_password("correct_password").unwrap();
        let user = User::new(
            "testowner".to_string(),
            "test@example.com".to_string(),
            password_hash,
        );

        let mut users = HashMap::new();
        users.insert("testowner".to_string(), user);

        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(users)),
            repos_path,
        });

        let app = Router::new()
            .route("/:owner/:repo/git-receive-pack", post(handle_receive_pack))
            .with_state(state);

        use base64::Engine;
        let wrong_creds =
            base64::engine::general_purpose::STANDARD.encode("testowner:wrong_password");

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo.git/git-receive-pack")
                    .header("Authorization", format!("Basic {}", wrong_creds))
                    .body(Body::from("0000"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_receive_pack_accepts_valid_basic_auth() {
        use gitpub_core::User;
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        std::env::set_var(
            "JWT_SECRET",
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());

        let password_hash = crate::auth::hash_password("correct_password").unwrap();
        let user = User::new(
            "testowner".to_string(),
            "test@example.com".to_string(),
            password_hash,
        );

        let mut users = HashMap::new();
        users.insert("testowner".to_string(), user);

        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(users)),
            repos_path,
        });

        let app = Router::new()
            .route("/:owner/:repo/git-receive-pack", post(handle_receive_pack))
            .with_state(state);

        use base64::Engine;
        let creds = base64::engine::general_purpose::STANDARD.encode("testowner:correct_password");

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo.git/git-receive-pack")
                    .header("Authorization", format!("Basic {}", creds))
                    .body(Body::from("0000"))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = resp.status();
        assert!(
            status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR,
            "Expected 200 or 500, got {}",
            status
        );
    }

    #[tokio::test]
    async fn test_receive_pack_accepts_valid_bearer_token() {
        use gitpub_core::User;
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        std::env::set_var(
            "JWT_SECRET",
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());

        let password_hash = crate::auth::hash_password("password").unwrap();
        let user = User::new(
            "testowner".to_string(),
            "test@example.com".to_string(),
            password_hash,
        );

        let token = crate::auth::generate_jwt(&user).unwrap();

        let mut users = HashMap::new();
        users.insert("testowner".to_string(), user);

        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(users)),
            repos_path,
        });

        let app = Router::new()
            .route("/:owner/:repo/git-receive-pack", post(handle_receive_pack))
            .with_state(state);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo.git/git-receive-pack")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::from("0000"))
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = resp.status();
        assert!(
            status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR,
            "Expected 200 or 500, got {}",
            status
        );
    }

    #[tokio::test]
    async fn test_receive_pack_enforces_ownership() {
        use gitpub_core::User;
        use std::collections::HashMap;
        use tokio::sync::RwLock;

        std::env::set_var(
            "JWT_SECRET",
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());

        let password_hash = crate::auth::hash_password("password").unwrap();
        let user = User::new(
            "otheruser".to_string(),
            "other@example.com".to_string(),
            password_hash,
        );

        let mut users = HashMap::new();
        users.insert("otheruser".to_string(), user);

        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(users)),
            repos_path,
        });

        let app = Router::new()
            .route("/:owner/:repo/git-receive-pack", post(handle_receive_pack))
            .with_state(state);

        use base64::Engine;
        let creds = base64::engine::general_purpose::STANDARD.encode("otheruser:password");

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo.git/git-receive-pack")
                    .header("Authorization", format!("Basic {}", creds))
                    .body(Body::from("0000"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_info_refs_receive_pack_unauthenticated() {
        let tmp = tempfile::tempdir().unwrap();
        let repos_path = create_test_repo(tmp.path());
        let app = test_app(repos_path);

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/testrepo.git/info/refs?service=git-receive-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }
}
