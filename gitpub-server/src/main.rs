mod git_http;

use axum::{extract::State, routing::get, routing::post, Json, Router};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub repo_storage_path: PathBuf,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/repositories", get(list_repositories))
        .route("/:owner/:repo/info/refs", get(git_http::git_info_refs))
        .route(
            "/:owner/:repo/git-upload-pack",
            post(git_http::git_upload_pack),
        )
        .route(
            "/:owner/:repo/git-receive-pack",
            post(git_http::git_receive_pack),
        )
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let repo_storage_path = std::env::var("GITPUB_REPO_PATH")
        .unwrap_or_else(|_| "/var/gitpub/repositories".to_string());

    let state = Arc::new(AppState {
        repo_storage_path: PathBuf::from(repo_storage_path),
    });

    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    tracing::info!("Server listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

#[derive(Serialize)]
struct RepositoryListResponse {
    repositories: Vec<RepositoryInfo>,
}

#[derive(Serialize)]
struct RepositoryInfo {
    name: String,
    owner: String,
    description: Option<String>,
}

async fn list_repositories(State(_state): State<Arc<AppState>>) -> Json<RepositoryListResponse> {
    Json(RepositoryListResponse {
        repositories: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response, "OK");
    }
}
