mod auth;

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
    refresh_tokens: Arc<RwLock<HashMap<String, auth::RefreshToken>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    auth::get_jwt_secret().expect("JWT_SECRET must be set and at least 32 bytes");

    let state = Arc::new(AppState {
        users: Arc::new(RwLock::new(HashMap::new())),
        refresh_tokens: Arc::new(RwLock::new(HashMap::new())),
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh_token))
        .route("/api/auth/me", get(get_current_user))
        .route("/api/repositories", get(list_repositories))
        .with_state(state.clone());

    tokio::spawn(cleanup_expired_tokens(state.clone()));

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
    let users = state.users.read().await;
    if users.values().any(|u| u.username == req.username) {
        return Err(auth::AuthError::UserAlreadyExists);
    }
    drop(users);

    let password_hash = auth::hash_password(&req.password)?;
    let user = User::new(req.username.clone(), req.email.clone(), password_hash);

    let access_token = auth::generate_jwt(&user)?;
    let refresh_token = auth::generate_refresh_token(&user);

    let mut users = state.users.write().await;
    users.insert(user.id.clone(), user.clone());
    drop(users);

    let mut tokens = state.refresh_tokens.write().await;
    tokens.insert(refresh_token.token_id.clone(), refresh_token.clone());
    drop(tokens);

    Ok((
        StatusCode::CREATED,
        Json(auth::LoginResponse {
            access_token,
            refresh_token: refresh_token.token_id,
            token_type: "Bearer".to_string(),
            expires_in: 15 * 60,
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
        .values()
        .find(|u| u.username == req.username)
        .ok_or(auth::AuthError::InvalidCredentials)?
        .clone();
    drop(users);

    let is_valid = auth::verify_password(&req.password, &user.password_hash)?;
    if !is_valid {
        return Err(auth::AuthError::InvalidCredentials);
    }

    let access_token = auth::generate_jwt(&user)?;
    let refresh_token = auth::generate_refresh_token(&user);

    let mut tokens = state.refresh_tokens.write().await;
    tokens.insert(refresh_token.token_id.clone(), refresh_token.clone());
    drop(tokens);

    Ok(Json(auth::LoginResponse {
        access_token,
        refresh_token: refresh_token.token_id,
        token_type: "Bearer".to_string(),
        expires_in: 15 * 60,
        user: user.into(),
    }))
}

async fn get_current_user(
    State(state): State<Arc<AppState>>,
    auth: auth::RequireAuth,
) -> Result<Json<auth::UserInfo>, auth::AuthError> {
    let users = state.users.read().await;
    let user = users
        .get(&auth.claims.user_id)
        .ok_or(auth::AuthError::InvalidToken)?;

    Ok(Json(user.clone().into()))
}

#[derive(serde::Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh_token(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<auth::LoginResponse>, auth::AuthError> {
    let tokens = state.refresh_tokens.read().await;
    let refresh_token = tokens
        .get(&req.refresh_token)
        .ok_or(auth::AuthError::RefreshTokenNotFound)?
        .clone();
    drop(tokens);

    let now = chrono::Utc::now().timestamp();
    if refresh_token.expires_at < now {
        return Err(auth::AuthError::RefreshTokenExpired);
    }

    let users = state.users.read().await;
    let user = users
        .get(&refresh_token.user_id)
        .ok_or(auth::AuthError::RefreshTokenInvalid)?
        .clone();
    drop(users);

    let access_token = auth::generate_jwt(&user)?;
    let new_refresh_token = auth::generate_refresh_token(&user);

    let mut tokens = state.refresh_tokens.write().await;
    tokens.remove(&req.refresh_token);
    tokens.insert(
        new_refresh_token.token_id.clone(),
        new_refresh_token.clone(),
    );
    drop(tokens);

    Ok(Json(auth::LoginResponse {
        access_token,
        refresh_token: new_refresh_token.token_id,
        token_type: "Bearer".to_string(),
        expires_in: 15 * 60,
        user: user.into(),
    }))
}

async fn cleanup_expired_tokens(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
    loop {
        interval.tick().await;
        let now = chrono::Utc::now().timestamp();
        let mut tokens = state.refresh_tokens.write().await;
        tokens.retain(|_, token| token.expires_at >= now);
        drop(tokens);
        tracing::info!("Cleaned up expired refresh tokens");
    }
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
            refresh_tokens: Arc::new(RwLock::new(HashMap::new())),
        });

        Router::new()
            .route("/health", get(health_check))
            .route("/api/auth/register", post(register))
            .route("/api/auth/login", post(login))
            .route("/api/auth/refresh", post(refresh_token))
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

        assert!(!json.access_token.is_empty());
        assert!(!json.refresh_token.is_empty());
        assert_eq!(json.token_type, "Bearer");
        assert_eq!(json.expires_in, 15 * 60);
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

        assert!(!json.access_token.is_empty());
        assert!(!json.refresh_token.is_empty());
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
        let token = json.access_token;

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
    async fn test_refresh_endpoint_returns_new_access_token() {
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
        let refresh_token = json.refresh_token;

        let refresh_body = serde_json::json!({
            "refresh_token": refresh_token
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/refresh")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&refresh_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let new_json: auth::LoginResponse = serde_json::from_slice(&body).unwrap();

        assert!(!new_json.access_token.is_empty());
        assert!(!new_json.refresh_token.is_empty());
        assert_ne!(new_json.refresh_token, refresh_token);
    }

    #[tokio::test]
    async fn test_refresh_endpoint_rejects_invalid_token() {
        let app = create_test_app();

        let refresh_body = serde_json::json!({
            "refresh_token": "invalid-token-id"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/refresh")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&refresh_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_login_returns_both_tokens() {
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

        assert!(!json.access_token.is_empty());
        assert!(!json.refresh_token.is_empty());
        assert_eq!(json.token_type, "Bearer");
        assert_eq!(json.expires_in, 15 * 60);
    }

    #[tokio::test]
    async fn test_register_returns_both_tokens() {
        let app = create_test_app();

        let register_body = serde_json::json!({
            "username": "newuser",
            "email": "new@example.com",
            "password": "password123"
        });

        let response = app
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

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: auth::LoginResponse = serde_json::from_slice(&body).unwrap();

        assert!(!json.access_token.is_empty());
        assert!(!json.refresh_token.is_empty());
        assert_eq!(json.token_type, "Bearer");
        assert_eq!(json.expires_in, 15 * 60);
    }
}
