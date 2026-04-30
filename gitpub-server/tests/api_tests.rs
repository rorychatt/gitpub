use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use axum_test::TestServer;
use gitpub_core::User;
use std::{collections::HashMap, sync::Arc, time::Duration};
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
#[allow(dead_code)]
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

mod rate_limit_tests {
    use super::*;
    use gitpub_core::User;
    use gitpub_server::{auth, rate_limit};

    #[derive(Clone)]
    struct AppState {
        users: Arc<RwLock<HashMap<String, User>>>,
    }

    fn create_auth_test_app() -> Router {
        std::env::set_var(
            "JWT_SECRET",
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(HashMap::new())),
        });

        async fn register(
            State(state): State<Arc<AppState>>,
            Json(req): Json<auth::RegisterRequest>,
        ) -> Result<(StatusCode, Json<auth::LoginResponse>), auth::AuthError> {
            let users = state.users.read().await;
            if users.contains_key(&req.username) {
                return Err(auth::AuthError::UserAlreadyExists);
            }
            drop(users);

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

        async fn login(
            State(state): State<Arc<AppState>>,
            Json(req): Json<auth::LoginRequest>,
        ) -> Result<Json<auth::LoginResponse>, auth::AuthError> {
            let users = state.users.read().await;
            let user = users
                .get(&req.username)
                .ok_or(auth::AuthError::InvalidCredentials)?;

            let is_valid = auth::verify_password(&req.password, &user.password_hash)?;
            if !is_valid {
                return Err(auth::AuthError::InvalidCredentials);
            }

            let token = auth::generate_jwt(user)?;

            Ok(Json(auth::LoginResponse {
                token,
                user: user.clone().into(),
            }))
        }

        async fn health_check() -> &'static str {
            "OK"
        }

        let rate_limiter = rate_limit::create_auth_rate_limiter();

        let auth_routes = Router::new()
            .route("/api/auth/register", post(register))
            .route("/api/auth/login", post(login))
            .layer(rate_limiter);

        Router::new()
            .route("/health", get(health_check))
            .merge(auth_routes)
            .with_state(state)
    }

    #[tokio::test]
    async fn test_login_rate_limit_blocks_excessive_attempts() {
        let server = TestServer::new(create_auth_test_app()).unwrap();

        let register_body = serde_json::json!({
            "username": "ratelimituser",
            "email": "ratelimit@example.com",
            "password": "password123"
        });

        let response = server.post("/api/auth/register").json(&register_body).await;
        response.assert_status(StatusCode::CREATED);

        let login_body = serde_json::json!({
            "username": "ratelimituser",
            "password": "wrongpassword"
        });

        // Register used 1 request, so we have 4 left in the burst
        for i in 1..=4 {
            let response = server.post("/api/auth/login").json(&login_body).await;
            assert!(
                response.status_code() == StatusCode::UNAUTHORIZED,
                "Attempt {} should be allowed but fail auth",
                i
            );
        }

        // 5th login attempt (6th total auth request) should be rate limited
        let response = server.post("/api/auth/login").json(&login_body).await;
        assert_eq!(
            response.status_code(),
            StatusCode::TOO_MANY_REQUESTS,
            "5th login attempt (6th total auth request) should be rate limited"
        );
    }

    #[tokio::test]
    async fn test_register_rate_limit_blocks_excessive_attempts() {
        let server = TestServer::new(create_auth_test_app()).unwrap();

        for i in 1..=5 {
            let register_body = serde_json::json!({
                "username": format!("user{}", i),
                "email": format!("user{}@example.com", i),
                "password": "password123"
            });

            let response = server.post("/api/auth/register").json(&register_body).await;
            assert_eq!(
                response.status_code(),
                StatusCode::CREATED,
                "Attempt {} should succeed",
                i
            );
        }

        let register_body = serde_json::json!({
            "username": "user6",
            "email": "user6@example.com",
            "password": "password123"
        });

        let response = server.post("/api/auth/register").json(&register_body).await;
        assert_eq!(
            response.status_code(),
            StatusCode::TOO_MANY_REQUESTS,
            "6th registration attempt should be rate limited"
        );
    }

    #[tokio::test]
    async fn test_rate_limit_resets_after_window() {
        let server = TestServer::new(create_auth_test_app()).unwrap();

        for i in 1..=5 {
            let register_body = serde_json::json!({
                "username": format!("resetuser{}", i),
                "email": format!("resetuser{}@example.com", i),
                "password": "password123"
            });

            let response = server.post("/api/auth/register").json(&register_body).await;
            assert_eq!(response.status_code(), StatusCode::CREATED);
        }

        let register_body = serde_json::json!({
            "username": "resetuser6",
            "email": "resetuser6@example.com",
            "password": "password123"
        });

        let response = server.post("/api/auth/register").json(&register_body).await;
        assert_eq!(response.status_code(), StatusCode::TOO_MANY_REQUESTS);

        tokio::time::sleep(Duration::from_secs(13)).await;

        let register_body = serde_json::json!({
            "username": "resetuser7",
            "email": "resetuser7@example.com",
            "password": "password123"
        });

        let response = server.post("/api/auth/register").json(&register_body).await;
        assert_eq!(
            response.status_code(),
            StatusCode::CREATED,
            "Request should succeed after rate limit window reset"
        );
    }

    #[tokio::test]
    async fn test_non_auth_endpoints_not_rate_limited() {
        let server = TestServer::new(create_auth_test_app()).unwrap();

        for _ in 0..10 {
            let response = server.get("/health").await;
            response.assert_status_ok();
        }
    }
}
