mod auth;
mod rate_limit;
mod git_http;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use gitpub_core::{Database, User};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub users: Arc<RwLock<std::collections::HashMap<String, User>>>,
    pub repos_path: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    auth::get_jwt_secret().expect("JWT_SECRET must be set and at least 32 bytes");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/gitpub".to_string());
    let db = Database::new(&database_url).await?;

    let repos_path = std::env::var("GITPUB_REPOS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/gitpub/repos"));

    let state = Arc::new(AppState {
        db: Arc::new(db),
        users: Arc::new(RwLock::new(std::collections::HashMap::new())),
        repos_path,
    });

    let rate_limiter = rate_limit::create_auth_rate_limiter();

    let auth_routes = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .layer(rate_limiter);

    let app = Router::new()
        .route("/health", get(health_check))
        .merge(auth_routes)
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
) -> Result<(StatusCode, Json<auth::LoginResponse>), auth::AuthError> {
    if let Ok(Some(_)) = state.db.get_user_by_username(&req.username).await {
        return Err(auth::AuthError::UserAlreadyExists);
    }

    if let Ok(Some(_)) = state.db.get_user_by_email(&req.email).await {
        return Err(auth::AuthError::UserAlreadyExists);
    }

    let password_hash = auth::hash_password(&req.password)?;
    let user = User::new(req.username.clone(), req.email.clone(), password_hash);

    state.db.insert_user(&user).await.map_err(|e| {
        tracing::error!("Failed to insert user: {}", e);
        if e.to_string().contains("duplicate key") || e.to_string().contains("UNIQUE constraint") {
            auth::AuthError::UserAlreadyExists
        } else {
            auth::AuthError::InternalError
        }
    })?;

    let token = auth::generate_jwt(&user)?;

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
    let user = state
        .db
        .get_user_by_username(&req.username)
        .await
        .map_err(|_| auth::AuthError::InternalError)?
        .ok_or(auth::AuthError::InvalidCredentials)?;

    let is_valid = auth::verify_password(&req.password, &user.password_hash)?;
    if !is_valid {
        return Err(auth::AuthError::InvalidCredentials);
    }

    let token = auth::generate_jwt(&user)?;

    Ok(Json(auth::LoginResponse {
        token,
        user: user.into(),
    }))
}

async fn get_current_user(
    State(state): State<Arc<AppState>>,
    auth: auth::RequireAuth,
) -> Result<Json<auth::UserInfo>, auth::AuthError> {
    let user = state
        .db
        .get_user_by_username(&auth.claims.username)
        .await
        .map_err(|_| auth::AuthError::InternalError)?
        .ok_or(auth::AuthError::InvalidToken)?;

    Ok(Json(user.into()))
}
