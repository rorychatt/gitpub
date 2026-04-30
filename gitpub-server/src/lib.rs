pub mod auth;
pub mod rate_limit;

use axum::{extract::State, routing::get, Json, Router};
use gitpub_core::{RefreshToken, User};
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<RwLock<HashMap<String, User>>>,
    pub refresh_tokens: Arc<RwLock<HashMap<String, RefreshToken>>>,
    pub repos_path: PathBuf,
}

impl AppState {
    pub fn new(repos_path: PathBuf) -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            refresh_tokens: Arc::new(RwLock::new(HashMap::new())),
            repos_path,
        }
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
