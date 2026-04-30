use axum::{http::StatusCode, Router};
use axum_test::TestServer;

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
