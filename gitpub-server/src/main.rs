use axum::{extract::State, routing::{get, post}, Json, Router};
use gitpub_core::Database;
use serde::Serialize;
use std::{env, sync::Arc};

mod auth;
mod routes;

use auth::AuthUser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    if env::var("JWT_SECRET").is_err() {
        anyhow::bail!("JWT_SECRET environment variable must be set");
    }

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/gitpub".to_string());

    let db = Database::new(&database_url).await?;
    let state = Arc::new(db);

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/auth/register", post(routes::auth::register))
        .route("/api/auth/login", post(routes::auth::login))
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
    AuthUser(user): AuthUser,
    State(_db): State<Arc<Database>>,
) -> Json<RepositoryListResponse> {
    tracing::info!("Listing repositories for user: {}", user.username);
    Json(RepositoryListResponse {
        repositories: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        assert_eq!(response, "OK");
    }

    #[tokio::test]
    async fn test_protected_endpoint_without_auth() {
        std::env::set_var("JWT_SECRET", "test_secret_key_at_least_32_characters_long");
        std::env::set_var("DATABASE_URL", "postgres://localhost/gitpub_test");

        let db = match Database::new(&std::env::var("DATABASE_URL").unwrap()).await {
            Ok(db) => Arc::new(db),
            Err(_) => return,
        };

        let app = Router::new()
            .route("/api/repositories", get(list_repositories))
            .with_state(db);

        let request = Request::builder()
            .uri("/api/repositories")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_protected_endpoint_with_auth() {
        std::env::set_var("JWT_SECRET", "test_secret_key_at_least_32_characters_long");
        std::env::set_var("DATABASE_URL", "postgres://localhost/gitpub_test");

        let db = match Database::new(&std::env::var("DATABASE_URL").unwrap()).await {
            Ok(db) => Arc::new(db),
            Err(_) => return,
        };

        let token = auth::create_jwt("user123", "testuser").expect("Failed to create JWT");

        let app = Router::new()
            .route("/api/repositories", get(list_repositories))
            .with_state(db);

        let request = Request::builder()
            .uri("/api/repositories")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
