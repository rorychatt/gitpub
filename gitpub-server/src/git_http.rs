use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::AppState;

#[derive(Deserialize)]
pub struct ServiceQuery {
    pub service: Option<String>,
}

pub struct GitHttpError {
    status: StatusCode,
    message: String,
}

impl IntoResponse for GitHttpError {
    fn into_response(self) -> Response {
        let mut response = (self.status, self.message).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                "WWW-Authenticate",
                "Basic realm=\"gitpub\"".parse().unwrap(),
            );
        }
        response
    }
}

fn pkt_line(data: &str) -> Vec<u8> {
    let len = data.len() + 4;
    format!("{:04x}{}", len, data).into_bytes()
}

fn resolve_repo_path(state: &AppState, owner: &str, repo: &str) -> Result<PathBuf, GitHttpError> {
    let repo_name = repo.strip_suffix(".git").unwrap_or(repo);
    let path = state
        .repo_storage_path
        .join(owner)
        .join(format!("{}.git", repo_name));
    if !path.exists() {
        return Err(GitHttpError {
            status: StatusCode::NOT_FOUND,
            message: format!("Repository {}/{} not found", owner, repo_name),
        });
    }
    Ok(path)
}

fn check_write_auth(headers: &HeaderMap) -> Result<(), GitHttpError> {
    if headers.get(header::AUTHORIZATION).is_none() {
        return Err(GitHttpError {
            status: StatusCode::UNAUTHORIZED,
            message: "Authentication required".to_string(),
        });
    }
    Ok(())
}

pub async fn git_info_refs(
    Path((owner, repo)): Path<(String, String)>,
    Query(params): Query<ServiceQuery>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, GitHttpError> {
    let service = params.service.ok_or_else(|| GitHttpError {
        status: StatusCode::BAD_REQUEST,
        message: "Missing service parameter".to_string(),
    })?;

    if service != "git-upload-pack" && service != "git-receive-pack" {
        return Err(GitHttpError {
            status: StatusCode::BAD_REQUEST,
            message: format!("Invalid service: {}", service),
        });
    }

    if service == "git-receive-pack" {
        check_write_auth(&headers)?;
    }

    let repo_path = resolve_repo_path(&state, &owner, &repo)?;

    let git_cmd = service.strip_prefix("git-").unwrap_or(&service);
    let output = Command::new("git")
        .arg(git_cmd)
        .arg("--stateless-rpc")
        .arg("--advertise-refs")
        .arg(&repo_path)
        .output()
        .await
        .map_err(|e| GitHttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to execute git: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!("git {} --advertise-refs failed: {}", service, stderr);
        return Err(GitHttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("git {} failed", service),
        });
    }

    let content_type = format!("application/x-{}-advertisement", service);

    let mut body = Vec::new();
    body.extend_from_slice(&pkt_line(&format!("# service={}\n", service)));
    body.extend_from_slice(b"0000");
    body.extend_from_slice(&output.stdout);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(body))
        .unwrap())
}

pub async fn git_upload_pack(
    Path((owner, repo)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<Response, GitHttpError> {
    let repo_path = resolve_repo_path(&state, &owner, &repo)?;

    let mut child = Command::new("git")
        .arg("upload-pack")
        .arg("--stateless-rpc")
        .arg(&repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| GitHttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to spawn git upload-pack: {}", e),
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&body).await.map_err(|e| GitHttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to write to git stdin: {}", e),
        })?;
    }

    let output = child.wait_with_output().await.map_err(|e| GitHttpError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("git upload-pack failed: {}", e),
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/x-git-upload-pack-result")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(output.stdout))
        .unwrap())
}

pub async fn git_receive_pack(
    Path((owner, repo)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, GitHttpError> {
    check_write_auth(&headers)?;

    let repo_path = resolve_repo_path(&state, &owner, &repo)?;

    let mut child = Command::new("git")
        .arg("receive-pack")
        .arg("--stateless-rpc")
        .arg(&repo_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| GitHttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to spawn git receive-pack: {}", e),
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&body).await.map_err(|e| GitHttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("Failed to write to git stdin: {}", e),
        })?;
    }

    let output = child.wait_with_output().await.map_err(|e| GitHttpError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("git receive-pack failed: {}", e),
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/x-git-receive-pack-result",
        )
        .header(header::CACHE_CONTROL, "no-cache")
        .body(axum::body::Body::from(output.stdout))
        .unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use std::path::Path;
    use tower::ServiceExt;

    fn create_test_router(storage_path: &Path) -> axum::Router {
        let state = Arc::new(AppState {
            repo_storage_path: storage_path.to_path_buf(),
        });
        crate::create_router(state)
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn create_bare_repo(storage_path: &Path, owner: &str, name: &str) {
        let owner_dir = storage_path.join(owner);
        std::fs::create_dir_all(&owner_dir).unwrap();
        let bare_path = owner_dir.join(format!("{}.git", name));

        let output = std::process::Command::new("git")
            .args(["init", "--bare"])
            .arg(&bare_path)
            .output()
            .unwrap();
        assert!(output.status.success(), "Failed to init bare repo");

        let work_dir = tempfile::tempdir().unwrap();
        let work_path = work_dir.path();

        run_git(work_path, &["init"]);
        run_git(work_path, &["config", "user.name", "Test"]);
        run_git(work_path, &["config", "user.email", "test@test.com"]);

        std::fs::write(work_path.join("README.md"), "# Test\n").unwrap();

        run_git(work_path, &["add", "."]);
        run_git(work_path, &["commit", "-m", "initial commit"]);
        run_git(work_path, &["branch", "-M", "main"]);

        let bare_str = bare_path.to_string_lossy().to_string();
        run_git(work_path, &["remote", "add", "origin", &bare_str]);
        run_git(work_path, &["push", "origin", "main"]);

        run_git(&bare_path, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    }

    #[tokio::test]
    async fn test_git_info_refs_upload_pack() {
        let dir = tempfile::tempdir().unwrap();
        create_bare_repo(dir.path(), "testowner", "testrepo");

        let app = create_test_router(dir.path());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/testrepo/info/refs?service=git-upload-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-git-upload-pack-advertisement"
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8_lossy(&body);
        assert!(
            body_str.starts_with("001e# service=git-upload-pack\n0000"),
            "Unexpected body prefix: {:?}",
            &body_str[..body_str.len().min(80)]
        );
    }

    #[tokio::test]
    async fn test_git_info_refs_receive_pack() {
        let dir = tempfile::tempdir().unwrap();
        create_bare_repo(dir.path(), "testowner", "testrepo");

        let app = create_test_router(dir.path());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/testrepo/info/refs?service=git-receive-pack")
                    .header(header::AUTHORIZATION, "Basic dGVzdDp0ZXN0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-git-receive-pack-advertisement"
        );

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8_lossy(&body);
        assert!(
            body_str.starts_with("001f# service=git-receive-pack\n0000"),
            "Unexpected body prefix: {:?}",
            &body_str[..body_str.len().min(80)]
        );
    }

    #[tokio::test]
    async fn test_git_upload_pack_basic() {
        let dir = tempfile::tempdir().unwrap();
        create_bare_repo(dir.path(), "testowner", "testrepo");

        let app = create_test_router(dir.path());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo/git-upload-pack")
                    .header(
                        header::CONTENT_TYPE,
                        "application/x-git-upload-pack-request",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-git-upload-pack-result"
        );
    }

    #[tokio::test]
    async fn test_git_receive_pack_basic() {
        let dir = tempfile::tempdir().unwrap();
        create_bare_repo(dir.path(), "testowner", "testrepo");

        let app = create_test_router(dir.path());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo/git-receive-pack")
                    .header(
                        header::CONTENT_TYPE,
                        "application/x-git-receive-pack-request",
                    )
                    .header(header::AUTHORIZATION, "Basic dGVzdDp0ZXN0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/x-git-receive-pack-result"
        );
    }

    #[tokio::test]
    async fn test_git_http_authentication() {
        let dir = tempfile::tempdir().unwrap();
        create_bare_repo(dir.path(), "testowner", "testrepo");

        let app = create_test_router(dir.path());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/testrepo/info/refs?service=git-receive-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get("WWW-Authenticate").is_some());

        let app2 = create_test_router(dir.path());
        let response2 = app2
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/testowner/testrepo/git-receive-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response2.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_git_http_invalid_repo() {
        let dir = tempfile::tempdir().unwrap();

        let app = create_test_router(dir.path());
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/testowner/nonexistent/info/refs?service=git-upload-pack")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn integration_test_clone_via_http() {
        let dir = tempfile::tempdir().unwrap();
        create_bare_repo(dir.path(), "testowner", "testrepo");

        let state = Arc::new(AppState {
            repo_storage_path: dir.path().to_path_buf(),
        });
        let app = crate::create_router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let clone_dir = dir.path().join("cloned");
        std::fs::create_dir_all(&clone_dir).unwrap();

        let url = format!("http://127.0.0.1:{}/testowner/testrepo", addr.port());
        let output = tokio::process::Command::new("git")
            .args(["clone", &url, "testrepo"])
            .current_dir(&clone_dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .await
            .unwrap();

        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(clone_dir.join("testrepo").join("README.md").exists());
    }
}
