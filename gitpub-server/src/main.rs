mod git_http;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    repos_path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let repos_path = std::env::var("GITPUB_REPOS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/gitpub/repos"));

    let state = Arc::new(AppState { repos_path });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/repositories", get(list_repositories))
        .route("/:owner/:repo/info/refs", get(git_http::handle_info_refs))
        .route(
            "/:owner/:repo/git-upload-pack",
            post(git_http::handle_upload_pack),
        )
        .route(
            "/:owner/:repo/git-receive-pack",
            post(git_http::handle_receive_pack),
        )
        .with_state(state);

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
