use crate::auth;
use crate::AppState;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

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
    auth: auth::RequireAuth,
) -> Json<RepositoryListResponse> {
    tracing::info!("Listing repositories for user: {}", auth.claims.username);
    Json(RepositoryListResponse {
        repositories: vec![],
    })
}
