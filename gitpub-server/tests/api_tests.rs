use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use axum_test::TestServer;
use gitpub_core::User;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

fn create_test_app() -> Router {
    use axum::{extract::State, routing::get, Json, Router};
    use serde::Serialize;
    use std::sync::Arc;

    #[derive(Clone)]
    struct AppState {}

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

    async fn health_check() -> &'static str {
        "OK"
    }

    async fn list_repositories(
        State(_state): State<Arc<AppState>>,
    ) -> Json<RepositoryListResponse> {
        Json(RepositoryListResponse {
            repositories: vec![],
        })
    }

    let state = Arc::new(AppState {});

    Router::new()
        .route("/health", get(health_check))
        .route("/api/repositories", get(list_repositories))
        .with_state(state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let server = TestServer::new(create_test_app()).unwrap();
    let response = server.get("/health").await;

    response.assert_status_ok();
    response.assert_text("OK");
}

#[tokio::test]
async fn test_list_repositories_empty() {
    let server = TestServer::new(create_test_app()).unwrap();
    let response = server.get("/api/repositories").await;

    response.assert_status_ok();
    let json: serde_json::Value = response.json();

    assert!(json["repositories"].is_array());
    assert_eq!(json["repositories"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_invalid_route_returns_404() {
    let server = TestServer::new(create_test_app()).unwrap();
    let response = server.get("/invalid/route").await;

    response.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_health_endpoint_multiple_calls() {
    let server = TestServer::new(create_test_app()).unwrap();

    for _ in 0..5 {
        let response = server.get("/health").await;
        response.assert_status_ok();
    }
}

#[tokio::test]
async fn test_repositories_endpoint_content_type() {
    let server = TestServer::new(create_test_app()).unwrap();
    let response = server.get("/api/repositories").await;

    response.assert_status_ok();
    let content_type = response.header("content-type");
    assert!(content_type.to_str().unwrap().contains("application/json"));
}

// Include auth module from main.rs - using path attribute to avoid module conflicts
#[path = "../src/auth.rs"]
mod auth;

fn create_auth_test_app() -> Router {
    #[derive(Clone)]
    struct AppState {
        users: Arc<RwLock<HashMap<String, User>>>,
    }

    async fn register(
        State(state): State<Arc<AppState>>,
        Json(req): Json<auth::RegisterRequest>,
    ) -> Result<(StatusCode, Json<auth::LoginResponse>), auth::AuthError> {
        let users = state.users.read().await;
        if users.contains_key(&req.username) {
            return Err(auth::AuthError::UserAlreadyExists);
        }
        drop(users);

        auth::validate_password_strength(&req.password)?;
        let password_hash = auth::hash_password(&req.password)?;
        let user = User::new(req.username.clone(), req.email.clone(), password_hash);

        let token = auth::generate_jwt(&user)?;

        let mut users = state.users.write().await;
        users.insert(req.username.clone(), user.clone());

        Ok((
            StatusCode::CREATED,
            Json(auth::LoginResponse {
                token,
                user: user.into(),
            }),
        ))
    }

    let state = Arc::new(AppState {
        users: Arc::new(RwLock::new(HashMap::new())),
    });

    Router::new()
        .route("/api/auth/register", post(register))
        .with_state(state)
}

#[tokio::test]
async fn test_register_weak_password_rejected() {
    std::env::set_var(
        "JWT_SECRET",
        "test_secret_key_that_is_at_least_32_bytes_long",
    );

    let server = TestServer::new(create_auth_test_app()).unwrap();

    // Test password too short
    let response = server
        .post("/api/auth/register")
        .json(&serde_json::json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "short"
        }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json();
    assert!(json["error"]
        .as_str()
        .unwrap()
        .contains("at least 8 characters"));

    // Test weak password
    let response = server
        .post("/api/auth/register")
        .json(&serde_json::json!({
            "username": "testuser2",
            "email": "test2@example.com",
            "password": "password"
        }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json();
    assert!(json["error"].as_str().unwrap().contains("too weak"));

    // Test strong password succeeds
    let response = server
        .post("/api/auth/register")
        .json(&serde_json::json!({
            "username": "testuser3",
            "email": "test3@example.com",
            "password": "Tr0ub4dor&3"
        }))
        .await;

    response.assert_status(StatusCode::CREATED);
    let json: serde_json::Value = response.json();
    assert!(json["token"].is_string());
    assert_eq!(json["user"]["username"], "testuser3");
}
