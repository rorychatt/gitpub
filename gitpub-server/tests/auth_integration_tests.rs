use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use gitpub_core::Database;
use std::sync::Arc;
use testcontainers::{core::{ContainerPort, WaitFor}, runners::AsyncRunner, GenericImage, ImageExt};
use tower::ServiceExt;

async fn setup_test_db() -> (testcontainers::ContainerAsync<GenericImage>, Database) {
    let postgres_image = GenericImage::new("postgres", "16-alpine")
        .with_exposed_port(ContainerPort::Tcp(5432))
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_env_var("POSTGRES_DB", "gitpub_test");

    let container = postgres_image.start().await.expect("Failed to start container");
    let port = container.get_host_port_ipv4(5432).await.expect("Failed to get port");
    let db_url = format!("postgresql://postgres:postgres@127.0.0.1:{}/gitpub_test", port);

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let db = Database::new(&db_url)
        .await
        .expect("Failed to connect to test database");

    (container, db)
}

fn create_test_app(db: Arc<Database>) -> Router {
    use axum::{
        extract::State,
        http::StatusCode as AxumStatusCode,
        routing::{get, post},
        Json,
    };
    use gitpub_core::User;
    use serde::Serialize;

    #[derive(Clone)]
    struct AppState {
        db: Arc<Database>,
    }

    #[derive(Serialize)]
    struct RepositoryListResponse {
        repositories: Vec<serde_json::Value>,
    }

    async fn health_check() -> &'static str {
        "OK"
    }

    async fn list_repositories(
        State(_state): State<Arc<AppState>>,
        _auth: gitpub_server::auth::RequireAuth,
    ) -> Json<RepositoryListResponse> {
        Json(RepositoryListResponse {
            repositories: vec![],
        })
    }

    async fn register(
        State(state): State<Arc<AppState>>,
        Json(req): Json<gitpub_server::auth::RegisterRequest>,
    ) -> Result<(AxumStatusCode, Json<gitpub_server::auth::LoginResponse>), gitpub_server::auth::AuthError> {
        if let Ok(Some(_)) = state.db.get_user_by_username(&req.username).await {
            return Err(gitpub_server::auth::AuthError::UserAlreadyExists);
        }

        if let Ok(Some(_)) = state.db.get_user_by_email(&req.email).await {
            return Err(gitpub_server::auth::AuthError::UserAlreadyExists);
        }

        let password_hash = gitpub_server::auth::hash_password(&req.password)?;
        let user = User::new(req.username.clone(), req.email.clone(), password_hash);

        state.db.insert_user(&user).await.map_err(|e| {
            if e.to_string().contains("duplicate key") || e.to_string().contains("UNIQUE constraint") {
                gitpub_server::auth::AuthError::UserAlreadyExists
            } else {
                gitpub_server::auth::AuthError::InternalError
            }
        })?;

        let token = gitpub_server::auth::generate_jwt(&user)?;

        Ok((
            AxumStatusCode::CREATED,
            Json(gitpub_server::auth::LoginResponse {
                token,
                user: user.into(),
            }),
        ))
    }

    async fn login(
        State(state): State<Arc<AppState>>,
        Json(req): Json<gitpub_server::auth::LoginRequest>,
    ) -> Result<Json<gitpub_server::auth::LoginResponse>, gitpub_server::auth::AuthError> {
        let user = state
            .db
            .get_user_by_username(&req.username)
            .await
            .map_err(|_| gitpub_server::auth::AuthError::InternalError)?
            .ok_or(gitpub_server::auth::AuthError::InvalidCredentials)?;

        let is_valid = gitpub_server::auth::verify_password(&req.password, &user.password_hash)?;
        if !is_valid {
            return Err(gitpub_server::auth::AuthError::InvalidCredentials);
        }

        let token = gitpub_server::auth::generate_jwt(&user)?;

        Ok(Json(gitpub_server::auth::LoginResponse {
            token,
            user: user.into(),
        }))
    }

    async fn get_current_user(
        State(state): State<Arc<AppState>>,
        auth: gitpub_server::auth::RequireAuth,
    ) -> Result<Json<gitpub_server::auth::UserInfo>, gitpub_server::auth::AuthError> {
        let user = state
            .db
            .get_user_by_username(&auth.claims.username)
            .await
            .map_err(|_| gitpub_server::auth::AuthError::InternalError)?
            .ok_or(gitpub_server::auth::AuthError::InvalidToken)?;

        Ok(Json(user.into()))
    }

    std::env::set_var(
        "JWT_SECRET",
        "test_secret_key_that_is_at_least_32_bytes_long",
    );

    let state = Arc::new(AppState { db });

    Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(get_current_user))
        .route("/api/repositories", get(list_repositories))
        .with_state(state)
}

#[tokio::test]
async fn test_register_persists_to_database() {
    let (_container, db) = setup_test_db().await;
    let db_arc = Arc::new(db);
    let app = create_test_app(db_arc.clone());

    let body = serde_json::json!({
        "username": "newuser",
        "email": "newuser@example.com",
        "password": "securepassword123"
    });

    let response = app
        .clone()
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

    let user_in_db = db_arc
        .get_user_by_username("newuser")
        .await
        .expect("Query should succeed");
    assert!(user_in_db.is_some());
    let user = user_in_db.unwrap();
    assert_eq!(user.username, "newuser");
    assert_eq!(user.email, "newuser@example.com");
    assert!(!user.password_hash.is_empty());
}

#[tokio::test]
async fn test_register_duplicate_username_returns_conflict() {
    let (_container, db) = setup_test_db().await;
    let db_arc = Arc::new(db);
    let app = create_test_app(db_arc.clone());

    let body1 = serde_json::json!({
        "username": "duplicate",
        "email": "user1@example.com",
        "password": "password123"
    });

    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response1.status(), StatusCode::CREATED);

    let body2 = serde_json::json!({
        "username": "duplicate",
        "email": "user2@example.com",
        "password": "password456"
    });

    let response2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_register_duplicate_email_returns_conflict() {
    let (_container, db) = setup_test_db().await;
    let db_arc = Arc::new(db);
    let app = create_test_app(db_arc.clone());

    let body1 = serde_json::json!({
        "username": "user1",
        "email": "duplicate@example.com",
        "password": "password123"
    });

    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response1.status(), StatusCode::CREATED);

    let body2 = serde_json::json!({
        "username": "user2",
        "email": "duplicate@example.com",
        "password": "password456"
    });

    let response2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&body2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_concurrent_registrations_same_username() {
    let (_container, db) = setup_test_db().await;
    let db_arc = Arc::new(db);

    let body1_str = serde_json::to_string(&serde_json::json!({
        "username": "concurrent_user",
        "email": "concurrent1@example.com",
        "password": "password123"
    })).unwrap();

    let body2_str = serde_json::to_string(&serde_json::json!({
        "username": "concurrent_user",
        "email": "concurrent2@example.com",
        "password": "password456"
    })).unwrap();

    let db1 = db_arc.clone();
    let db2 = db_arc.clone();

    let handle1 = tokio::spawn(async move {
        create_test_app(db1)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(body1_str))
                    .unwrap(),
            )
            .await
            .unwrap()
    });

    let handle2 = tokio::spawn(async move {
        create_test_app(db2)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(body2_str))
                    .unwrap(),
            )
            .await
            .unwrap()
    });

    let (result1, result2) = tokio::join!(handle1, handle2);
    let response1 = result1.unwrap();
    let response2 = result2.unwrap();

    let statuses = vec![response1.status(), response2.status()];
    assert!(
        statuses.contains(&StatusCode::CREATED) && statuses.contains(&StatusCode::CONFLICT),
        "One request should succeed (201) and one should fail (409), got {:?}",
        statuses
    );
}

#[tokio::test]
async fn test_sql_injection_username() {
    let (_container, db) = setup_test_db().await;
    let db_arc = Arc::new(db);
    let app = create_test_app(db_arc.clone());

    let body = serde_json::json!({
        "username": "'; DROP TABLE users; --",
        "email": "hacker@example.com",
        "password": "password123"
    });

    let response = app
        .clone()
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

    assert!(
        response.status() == StatusCode::CREATED || response.status() == StatusCode::BAD_REQUEST,
        "SQL injection should be safely handled"
    );
}

#[tokio::test]
async fn test_sql_injection_email() {
    let (_container, db) = setup_test_db().await;
    let db_arc = Arc::new(db);
    let app = create_test_app(db_arc.clone());

    let body = serde_json::json!({
        "username": "normaluser",
        "email": "test@example.com' OR '1'='1",
        "password": "password123"
    });

    let response = app
        .clone()
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

    assert!(
        response.status() == StatusCode::CREATED || response.status() == StatusCode::BAD_REQUEST,
        "SQL injection should be safely handled"
    );
}

#[tokio::test]
async fn test_login_after_registration() {
    let (_container, db) = setup_test_db().await;
    let db_arc = Arc::new(db);
    let app = create_test_app(db_arc.clone());

    let register_body = serde_json::json!({
        "username": "loginuser",
        "email": "login@example.com",
        "password": "mypassword123"
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
    assert_eq!(register_response.status(), StatusCode::CREATED);

    let login_body = serde_json::json!({
        "username": "loginuser",
        "password": "mypassword123"
    });

    let login_response = app
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

    assert_eq!(login_response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(login_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(!json["token"].as_str().unwrap().is_empty());
}
