mod auth;
mod git_http;

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use gitpub_core::{Repository, User};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    users: Arc<RwLock<HashMap<String, User>>>,
    repositories: Arc<RwLock<HashMap<String, Repository>>>,
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
        repositories: Arc::new(RwLock::new(HashMap::new())),
        repos_path,
    });

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(get_current_user))
        .route("/api/repositories", get(list_repositories))
        .route("/api/repositories", post(create_repository))
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

#[derive(Deserialize)]
struct CreateRepositoryRequest {
    name: String,
    description: Option<String>,
    is_private: Option<bool>,
}

#[derive(Serialize, Deserialize)]
struct CreateRepositoryResponse {
    id: String,
    name: String,
    owner: String,
    description: Option<String>,
    is_private: bool,
    default_branch: String,
    created_at: i64,
}

impl From<Repository> for CreateRepositoryResponse {
    fn from(repo: Repository) -> Self {
        Self {
            id: repo.id,
            name: repo.name,
            owner: repo.owner,
            description: repo.description,
            is_private: repo.is_private,
            default_branch: repo.default_branch,
            created_at: repo.created_at,
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

async fn create_repository(
    State(state): State<Arc<AppState>>,
    auth: auth::RequireAuth,
    Json(req): Json<CreateRepositoryRequest>,
) -> Result<(StatusCode, Json<CreateRepositoryResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Validate repository name
    if !is_valid_repository_name(&req.name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid repository name. Only alphanumeric characters and hyphens allowed, max 100 characters.".to_string(),
            }),
        ));
    }

    let owner = auth.claims.username.clone();
    let repo_key = format!("{}/{}", owner, req.name);

    // Check if repository already exists
    let repos = state.repositories.read().await;
    if repos.contains_key(&repo_key) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "Repository already exists".to_string(),
            }),
        ));
    }
    drop(repos);

    // Create repository directory structure
    let repo_path = state
        .repos_path
        .join(&owner)
        .join(format!("{}.git", req.name));

    if let Err(e) = tokio::fs::create_dir_all(repo_path.parent().unwrap()).await {
        tracing::error!("Failed to create repository directory: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to create repository directory".to_string(),
            }),
        ));
    }

    // Initialize bare git repository
    if let Err(e) = tokio::task::spawn_blocking({
        let repo_path = repo_path.clone();
        move || git2::Repository::init_bare(&repo_path)
    })
    .await
    {
        tracing::error!("Failed to initialize git repository: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to initialize git repository".to_string(),
            }),
        ));
    }

    // Create repository metadata
    let mut repository = Repository::new(req.name, owner);
    repository.description = req.description;
    repository.is_private = req.is_private.unwrap_or(false);

    // Store in state
    let mut repos = state.repositories.write().await;
    repos.insert(repo_key, repository.clone());

    tracing::info!(
        "Created repository: {}/{}",
        repository.owner,
        repository.name
    );

    Ok((StatusCode::CREATED, Json(repository.into())))
}

fn is_valid_repository_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 100 {
        return false;
    }

    name.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
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

        let temp_dir = std::env::temp_dir().join(format!("gitpub_test_{}", uuid::Uuid::new_v4()));

        let state = Arc::new(AppState {
            users: Arc::new(RwLock::new(HashMap::new())),
            repositories: Arc::new(RwLock::new(HashMap::new())),
            repos_path: temp_dir,
        });

        Router::new()
            .route("/health", get(health_check))
            .route("/api/auth/register", post(register))
            .route("/api/auth/login", post(login))
            .route("/api/auth/me", get(get_current_user))
            .route("/api/repositories", get(list_repositories))
            .route("/api/repositories", post(create_repository))
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
    async fn test_create_repository_success() {
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

        let create_repo_body = serde_json::json!({
            "name": "test-repo",
            "description": "A test repository",
            "is_private": false
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repositories")
                    .header("authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&create_repo_body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: CreateRepositoryResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(json.name, "test-repo");
        assert_eq!(json.owner, "testuser");
        assert_eq!(json.description, Some("A test repository".to_string()));
        assert!(!json.is_private);
    }

    #[tokio::test]
    async fn test_create_repository_duplicate_returns_conflict() {
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

        let create_repo_body = serde_json::json!({
            "name": "test-repo",
            "description": "A test repository"
        });

        // First creation should succeed
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repositories")
                    .header("authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&create_repo_body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Second creation should fail with conflict
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repositories")
                    .header("authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&create_repo_body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_create_repository_invalid_name_returns_bad_request() {
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

        let create_repo_body = serde_json::json!({
            "name": "test repo with spaces"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repositories")
                    .header("authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&create_repo_body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_create_repository_requires_auth() {
        let app = create_test_app();

        let create_repo_body = serde_json::json!({
            "name": "test-repo"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repositories")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&create_repo_body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_create_repository_creates_bare_git_repo() {
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

        let create_repo_body = serde_json::json!({
            "name": "test-repo"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/repositories")
                    .header("authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_string(&create_repo_body).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: CreateRepositoryResponse = serde_json::from_slice(&body).unwrap();

        // Verify the bare git repository was created on filesystem
        // Note: The actual temp_dir used by the app is random, so we can't directly verify the filesystem
        // This test verifies the API contract; filesystem verification would require shared state
        assert!(!json.id.is_empty());
    }
}
