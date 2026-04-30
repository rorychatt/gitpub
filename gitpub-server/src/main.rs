mod auth;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use gitpub_core::{RefreshToken, User};
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    users: Arc<RwLock<HashMap<String, User>>>,
    refresh_tokens: Arc<RwLock<HashMap<String, RefreshToken>>>,
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
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/logout", post(logout))
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
    let users = state.users.read().await;
    if users.contains_key(&req.username) {
        return Err(auth::AuthError::UserAlreadyExists);
    }
    drop(users);

    let password_hash = auth::hash_password(&req.password)?;
    let user = User::new(req.username.clone(), req.email.clone(), password_hash);

    let access_token = auth::generate_jwt(&user)?;
    let refresh_token = auth::generate_refresh_token();
    let refresh_token_hash = auth::hash_refresh_token(&refresh_token)?;

    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(auth::REFRESH_TOKEN_EXPIRATION_DAYS))
        .expect("valid timestamp")
        .timestamp();

    let refresh_token_record = RefreshToken::new(user.id.clone(), refresh_token_hash.clone(), expires_at);

    let mut users = state.users.write().await;
    users.insert(req.username.clone(), user.clone());

    let mut refresh_tokens = state.refresh_tokens.write().await;
    refresh_tokens.insert(refresh_token_hash, refresh_token_record);

    Ok((
        StatusCode::CREATED,
        Json(auth::LoginResponse {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: auth::ACCESS_TOKEN_EXPIRATION_MINUTES * 60,
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

    let access_token = auth::generate_jwt(user)?;
    let refresh_token = auth::generate_refresh_token();
    let refresh_token_hash = auth::hash_refresh_token(&refresh_token)?;

    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(auth::REFRESH_TOKEN_EXPIRATION_DAYS))
        .expect("valid timestamp")
        .timestamp();

    let refresh_token_record = RefreshToken::new(user.id.clone(), refresh_token_hash.clone(), expires_at);

    let mut refresh_tokens = state.refresh_tokens.write().await;
    refresh_tokens.insert(refresh_token_hash, refresh_token_record);

    Ok(Json(auth::LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: auth::ACCESS_TOKEN_EXPIRATION_MINUTES * 60,
        user: user.clone().into(),
    }))
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(req): Json<auth::RefreshRequest>,
) -> Result<Json<auth::RefreshResponse>, auth::AuthError> {
    let refresh_token_hash = auth::hash_refresh_token(&req.refresh_token)?;

    let refresh_tokens = state.refresh_tokens.read().await;
    let token_record = refresh_tokens
        .get(&refresh_token_hash)
        .ok_or(auth::AuthError::InvalidRefreshToken)?;

    if token_record.is_expired() {
        return Err(auth::AuthError::RefreshTokenExpired);
    }

    if token_record.is_revoked() {
        return Err(auth::AuthError::RefreshTokenRevoked);
    }

    let users = state.users.read().await;
    let user = users
        .values()
        .find(|u| u.id == token_record.user_id)
        .ok_or(auth::AuthError::InvalidRefreshToken)?;

    let access_token = auth::generate_jwt(user)?;

    Ok(Json(auth::RefreshResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: auth::ACCESS_TOKEN_EXPIRATION_MINUTES * 60,
    }))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    Json(req): Json<auth::RefreshRequest>,
) -> Result<StatusCode, auth::AuthError> {
    let refresh_token_hash = auth::hash_refresh_token(&req.refresh_token)?;

    let mut refresh_tokens = state.refresh_tokens.write().await;

    if let Some(token_record) = refresh_tokens.get_mut(&refresh_token_hash) {
        let now = chrono::Utc::now().timestamp();
        let mut updated_record = token_record.clone();
        updated_record.revoked_at = Some(now);
        refresh_tokens.insert(refresh_token_hash, updated_record);
    }

    Ok(StatusCode::NO_CONTENT)
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
            refresh_tokens: Arc::new(RwLock::new(HashMap::new())),
        });

        Router::new()
            .route("/health", get(health_check))
            .route("/api/auth/register", post(register))
            .route("/api/auth/login", post(login))
            .route("/api/auth/refresh", post(refresh))
            .route("/api/auth/logout", post(logout))
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
    async fn test_login_returns_access_and_refresh_tokens() {
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
        let json: auth::RefreshResponse = serde_json::from_slice(&body).unwrap();

        assert!(!json.access_token.is_empty());
        assert_eq!(json.token_type, "Bearer");
        assert_eq!(json.expires_in, 15 * 60);
    }

    #[tokio::test]
    async fn test_refresh_endpoint_rejects_invalid_refresh_token() {
        let app = create_test_app();

        let refresh_body = serde_json::json!({
            "refresh_token": "invalid_refresh_token"
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
    async fn test_logout_revokes_refresh_token() {
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

        let logout_body = serde_json::json!({
            "refresh_token": refresh_token.clone()
        });

        let logout_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&logout_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

        let refresh_body = serde_json::json!({
            "refresh_token": refresh_token
        });

        let refresh_response = app
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

        assert_eq!(refresh_response.status(), StatusCode::UNAUTHORIZED);
    }
}
