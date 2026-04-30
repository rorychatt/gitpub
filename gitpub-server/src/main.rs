mod auth;
mod rate_limit;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use gitpub_core::User;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    users: Arc<RwLock<HashMap<String, User>>>,
    login_limiter: rate_limit::AuthRateLimiter,
    register_limiter: rate_limit::AuthRateLimiter,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    auth::get_jwt_secret().expect("JWT_SECRET must be set and at least 32 bytes");

    let state = Arc::new(AppState {
        users: Arc::new(RwLock::new(HashMap::new())),
        login_limiter: rate_limit::create_login_limiter(),
        register_limiter: rate_limit::create_register_limiter(),
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(get_current_user))
        .route("/api/repositories", get(list_repositories))
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

async fn list_repositories(
    State(_state): State<Arc<AppState>>,
    auth: auth::RequireAuth,
) -> Json<RepositoryListResponse> {
    tracing::info!("Listing repositories for user: {}", auth.claims.username);
    Json(RepositoryListResponse {
        repositories: vec![],
    })
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<auth::RegisterRequest>,
) -> Result<(StatusCode, Json<auth::LoginResponse>), auth::AuthError> {
    if state.register_limiter.check().is_err() {
        tracing::warn!("Rate limit exceeded for register endpoint");
        return Err(auth::AuthError::RateLimitExceeded);
    }

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
    if state.login_limiter.check().is_err() {
        tracing::warn!("Rate limit exceeded for login endpoint");
        return Err(auth::AuthError::RateLimitExceeded);
    }

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

async fn get_current_user(
    State(state): State<Arc<AppState>>,
    auth: auth::RequireAuth,
) -> Result<Json<auth::UserInfo>, auth::AuthError> {
    let users = state.users.read().await;
    let user = users
        .get(&auth.claims.username)
        .ok_or(auth::AuthError::InvalidToken)?;

    Ok(Json(user.clone().into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn create_test_app() -> Router {
        std::env::set_var(
            "JWT_SECRET",
            "test_secret_key_that_is_at_least_32_bytes_long",
        );

        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(HashMap::new())),
            login_limiter: rate_limit::create_login_limiter(),
            register_limiter: rate_limit::create_register_limiter(),
        });

        Router::new()
            .route("/health", get(health_check))
            .route("/api/auth/register", post(register))
            .route("/api/auth/login", post(login))
            .route("/api/auth/me", get(get_current_user))
            .route("/api/repositories", get(list_repositories))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response, "OK");
    }

    #[tokio::test]
    async fn test_register_creates_user_with_hashed_password() {
        let app = create_test_app();

        let body = serde_json::json!({
            "username": "newuser",
            "email": "newuser@example.com",
            "password": "securepassword123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: auth::LoginResponse = serde_json::from_slice(&body).unwrap();

        assert!(!json.token.is_empty());
        assert_eq!(json.user.username, "newuser");
        assert_eq!(json.user.email, "newuser@example.com");
    }

    #[tokio::test]
    async fn test_login_returns_jwt_on_valid_credentials() {
        let app = create_test_app();

        let register_body = serde_json::json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123"
        });

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let login_body = serde_json::json!({
            "username": "testuser",
            "password": "password123"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: auth::LoginResponse = serde_json::from_slice(&body).unwrap();

        assert!(!json.token.is_empty());
        assert_eq!(json.user.username, "testuser");
    }

    #[tokio::test]
    async fn test_login_rejects_invalid_credentials() {
        let app = create_test_app();

        let register_body = serde_json::json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "correctpassword"
        });

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let login_body = serde_json::json!({
            "username": "testuser",
            "password": "wrongpassword"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_protected_endpoint_requires_auth() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/repositories")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_protected_endpoint_accepts_valid_jwt() {
        let app = create_test_app();

        let register_body = serde_json::json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123"
        });

        let register_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(register_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: auth::LoginResponse = serde_json::from_slice(&body).unwrap();
        let token = json.token;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/repositories")
                    .header("authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_protected_endpoint_rejects_invalid_jwt() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/repositories")
                    .header("authorization", "Bearer invalid.token.here")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_login_rate_limit_blocks_excessive_requests() {
        let app = create_test_app();

        let register_body = serde_json::json!({
            "username": "testuser",
            "email": "test@example.com",
            "password": "password123"
        });

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        for i in 0..11 {
            let login_body = serde_json::json!({
                "username": "testuser",
                "password": "wrongpassword"
            });

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/auth/login")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_string(&login_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            if i < 10 {
                assert_eq!(
                    response.status(),
                    StatusCode::UNAUTHORIZED,
                    "Request {} should be rate limited but got {:?}",
                    i,
                    response.status()
                );
            } else {
                assert_eq!(
                    response.status(),
                    StatusCode::TOO_MANY_REQUESTS,
                    "Request {} should return 429 but got {:?}",
                    i,
                    response.status()
                );
            }
        }
    }

    #[tokio::test]
    async fn test_register_rate_limit_blocks_excessive_requests() {
        let app = create_test_app();

        for i in 0..6 {
            let register_body = serde_json::json!({
                "username": format!("user{}", i),
                "email": format!("user{}@example.com", i),
                "password": "password123"
            });

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/auth/register")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_string(&register_body).unwrap()))
                        .unwrap(),
                )
                .await
                .unwrap();

            if i < 5 {
                assert_eq!(
                    response.status(),
                    StatusCode::CREATED,
                    "Request {} should succeed but got {:?}",
                    i,
                    response.status()
                );
            } else {
                assert_eq!(
                    response.status(),
                    StatusCode::TOO_MANY_REQUESTS,
                    "Request {} should return 429 but got {:?}",
                    i,
                    response.status()
                );
            }
        }
    }
}
