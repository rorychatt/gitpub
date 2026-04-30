use axum::Router;
use gitpub_server::{create_app, AppState};
use std::sync::Arc;

pub fn test_app() -> Router {
    let state = Arc::new(AppState::new());
    create_app(state)
}
