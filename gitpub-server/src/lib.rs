use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {}

impl AppState {
    pub fn new() -> Self {
        Self {}
    }
}

pub fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/repositories", get(list_repositories))
        .with_state(state)
}

pub async fn health_check() -> &'static str {
    "OK"
}

#[derive(Serialize)]
pub struct RepositoryListResponse {
    pub repositories: Vec<RepositoryInfo>,
}

#[derive(Serialize)]
pub struct RepositoryInfo {
    pub name: String,
    pub owner: String,
    pub description: Option<String>,
}

pub async fn list_repositories(
    State(_state): State<Arc<AppState>>,
) -> Json<RepositoryListResponse> {
    Json(RepositoryListResponse {
        repositories: vec![],
    })
}
