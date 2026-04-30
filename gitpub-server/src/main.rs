mod auth;
mod git_http;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use gitpub_core::User;
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    users: Arc<RwLock<HashMap<String, User>>>,
    repos_path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    auth::get_jwt_secret().expect("JWT_SECRET must be set and at least 32 bytes");

    let repos_path = std::env::var("GITPUB_REPOS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/gitpub/repos"));

    let state = Arc::new(AppState {
        users: Arc::new(RwLock::new(HashMap::new())),
        repos_path,
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/verify", post(verify_email))
        .route("/api/auth/resend-verification", post(resend_verification))
        .route("/api/auth/me", get(get_current_user))
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
) -> Result<(StatusCode, Json<auth::RegisterResponse>), auth::AuthError> {
    let users = state.users.read().await;
    if users.contains_key(&req.username) {
        return Err(auth::AuthError::UserAlreadyExists);
    }
    drop(users);

    let password_hash = auth::hash_password(&req.password)?;

    // Generate verification token
    let verification_token = uuid::Uuid::new_v4().to_string();
    let token_expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp();

    let user = User::new(req.username.clone(), req.email.clone(), password_hash)
        .with_verification_token(verification_token.clone(), token_expiration);

    // Log verification URL to console
    tracing::info!(
        "Verification URL for user '{}': http://localhost:3000/api/auth/verify?token={}",
        user.username,
        verification_token
    );

    let mut users = state.users.write().await;
    users.insert(req.username.clone(), user.clone());

    Ok((
        StatusCode::CREATED,
        Json(auth::RegisterResponse {
            message: "Registration successful. Please verify your email.".to_string(),
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

    // Check if email is verified
    if !user.email_verified {
        return Err(auth::AuthError::EmailNotVerified);
    }

    let token = auth::generate_jwt(user)?;

    Ok(Json(auth::LoginResponse {
        token,
        user: user.clone().into(),
    }))
}

async fn verify_email(
    State(state): State<Arc<AppState>>,
    Json(req): Json<auth::VerifyEmailRequest>,
) -> Result<Json<auth::LoginResponse>, auth::AuthError> {
    let mut users = state.users.write().await;

    // Find user by verification token
    let user_entry = users
        .iter_mut()
        .find(|(_, u)| u.verification_token.as_ref() == Some(&req.token))
        .ok_or(auth::AuthError::InvalidVerificationToken)?;

    let user = user_entry.1;

    // Check if token is expired
    let now = chrono::Utc::now().timestamp();
    if let Some(expires_at) = user.verification_token_expires_at {
        if expires_at < now {
            return Err(auth::AuthError::VerificationTokenExpired);
        }
    }

    // Mark email as verified and clear token
    user.email_verified = true;
    user.verification_token = None;
    user.verification_token_expires_at = None;

    let token = auth::generate_jwt(user)?;

    Ok(Json(auth::LoginResponse {
        token,
        user: user.clone().into(),
    }))
}

async fn resend_verification(
    State(state): State<Arc<AppState>>,
    Json(req): Json<auth::ResendVerificationRequest>,
) -> Result<(StatusCode, Json<auth::RegisterResponse>), auth::AuthError> {
    let mut users = state.users.write().await;

    // Find user by email
    let user = users
        .iter_mut()
        .find(|(_, u)| u.email == req.email)
        .map(|(_, u)| u)
        .ok_or(auth::AuthError::InvalidCredentials)?;

    // Check if already verified
    if user.email_verified {
        return Ok((
            StatusCode::OK,
            Json(auth::RegisterResponse {
                message: "Email is already verified.".to_string(),
            }),
        ));
    }

    // Generate new token
    let verification_token = uuid::Uuid::new_v4().to_string();
    let token_expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp();

    user.verification_token = Some(verification_token.clone());
    user.verification_token_expires_at = Some(token_expiration);

    // Log verification URL to console
    tracing::info!(
        "Verification URL for user '{}': http://localhost:3000/api/auth/verify?token={}",
        user.username,
        verification_token
    );

    Ok((
        StatusCode::OK,
        Json(auth::RegisterResponse {
            message: "Verification email resent. Please check your email.".to_string(),
        }),
    ))
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
            repos_path: PathBuf::from("/tmp/test-repos"),
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
}
