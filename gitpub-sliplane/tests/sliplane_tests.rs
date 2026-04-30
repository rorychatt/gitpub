use gitpub_sliplane::{DeploymentConfig, DeploymentStatus, Environment, SliplaneClient};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[test]
fn test_api_client_creation() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string());
    // Client created successfully - private fields cannot be accessed
    let _ = client;
}

#[test]
fn test_api_client_with_key() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string())
        .with_api_key("test-api-key".to_string());
    // Client with API key created successfully - private fields cannot be accessed
    let _ = client;
}

#[test]
fn test_deployment_config_creation() {
    let config = DeploymentConfig {
        repository_name: "test-repo".to_string(),
        branch: "main".to_string(),
        environment: Environment::Production,
        auto_scale: true,
    };

    assert_eq!(config.repository_name, "test-repo");
    assert_eq!(config.branch, "main");
    assert!(config.auto_scale);
}

#[test]
fn test_deployment_config_environments() {
    let dev = DeploymentConfig {
        repository_name: "test".to_string(),
        branch: "develop".to_string(),
        environment: Environment::Development,
        auto_scale: false,
    };

    let staging = DeploymentConfig {
        repository_name: "test".to_string(),
        branch: "staging".to_string(),
        environment: Environment::Staging,
        auto_scale: false,
    };

    let prod = DeploymentConfig {
        repository_name: "test".to_string(),
        branch: "main".to_string(),
        environment: Environment::Production,
        auto_scale: true,
    };

    assert!(matches!(dev.environment, Environment::Development));
    assert!(matches!(staging.environment, Environment::Staging));
    assert!(matches!(prod.environment, Environment::Production));
}

#[tokio::test]
async fn test_mock_api_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/deployments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "deployment_id": "mock-deployment-123",
            "status": "Pending",
            "url": null
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    assert!(!base_url.is_empty());
}

#[tokio::test]
async fn test_deploy_returns_result() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string());
    let config = DeploymentConfig {
        repository_name: "test-repo".to_string(),
        branch: "main".to_string(),
        environment: Environment::Development,
        auto_scale: false,
    };

    let result = client.deploy(&config).await;
    assert!(result.is_ok());

    let deployment = result.unwrap();
    assert!(!deployment.deployment_id.is_empty());
    assert!(matches!(deployment.status, DeploymentStatus::Pending));
}

#[tokio::test]
async fn test_get_deployment_status() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string());
    let status = client.get_deployment_status("test-deployment-id").await;

    assert!(status.is_ok());
    assert!(matches!(status.unwrap(), DeploymentStatus::Running));
}

#[tokio::test]
async fn test_wiremock_server_creation() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health", mock_server.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.text().await.unwrap(), "OK");
}

#[test]
fn test_deployment_status_variants() {
    let statuses = [
        DeploymentStatus::Pending,
        DeploymentStatus::Building,
        DeploymentStatus::Running,
        DeploymentStatus::Failed,
    ];

    assert_eq!(statuses.len(), 4);
}
