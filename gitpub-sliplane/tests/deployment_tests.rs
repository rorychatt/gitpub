use gitpub_sliplane::{
    DeploymentConfig, DeploymentResult, DeploymentStatus, Environment, SliplaneClient,
};

#[test]
fn test_client_creation_with_url() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string());
    let _ = client;
}

#[test]
fn test_client_builder_pattern() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string())
        .with_api_key("my-secret-key".to_string());
    let _ = client;
}

#[test]
fn test_deployment_config_creation() {
    let config = DeploymentConfig {
        repository_name: "my-app".to_string(),
        branch: "main".to_string(),
        environment: Environment::Development,
        auto_scale: false,
    };
    assert_eq!(config.repository_name, "my-app");
    assert_eq!(config.branch, "main");
    assert_eq!(config.environment, Environment::Development);
    assert!(!config.auto_scale);
}

#[test]
fn test_deployment_config_production() {
    let config = DeploymentConfig {
        repository_name: "my-app".to_string(),
        branch: "release/v1.0".to_string(),
        environment: Environment::Production,
        auto_scale: true,
    };
    assert_eq!(config.environment, Environment::Production);
    assert!(config.auto_scale);
}

#[test]
fn test_deployment_config_serialization() {
    let config = DeploymentConfig {
        repository_name: "my-app".to_string(),
        branch: "main".to_string(),
        environment: Environment::Staging,
        auto_scale: false,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: DeploymentConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, deserialized);
}

#[test]
fn test_deployment_result_serialization() {
    let result = DeploymentResult {
        deployment_id: "dep-123".to_string(),
        status: DeploymentStatus::Running,
        url: Some("https://my-app.sliplane.app".to_string()),
    };
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: DeploymentResult = serde_json::from_str(&json).unwrap();
    assert_eq!(result, deserialized);
}

#[test]
fn test_deployment_status_variants() {
    let statuses = vec![
        DeploymentStatus::Pending,
        DeploymentStatus::Building,
        DeploymentStatus::Running,
        DeploymentStatus::Failed,
    ];
    for status in &statuses {
        let json = serde_json::to_string(status).unwrap();
        let deserialized: DeploymentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(*status, deserialized);
    }
}

#[test]
fn test_environment_variants() {
    let envs = vec![
        Environment::Development,
        Environment::Staging,
        Environment::Production,
    ];
    for env in &envs {
        let json = serde_json::to_string(env).unwrap();
        let deserialized: Environment = serde_json::from_str(&json).unwrap();
        assert_eq!(*env, deserialized);
    }
}

#[tokio::test]
async fn test_deploy_returns_pending() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string());
    let config = DeploymentConfig {
        repository_name: "test-repo".to_string(),
        branch: "main".to_string(),
        environment: Environment::Development,
        auto_scale: false,
    };
    let result = client.deploy(&config).await.unwrap();
    assert_eq!(result.status, DeploymentStatus::Pending);
    assert!(!result.deployment_id.is_empty());
}

#[tokio::test]
async fn test_get_deployment_status_returns_running() {
    let client = SliplaneClient::new("https://api.sliplane.io".to_string());
    let status = client.get_deployment_status("dep-123").await.unwrap();
    assert_eq!(status, DeploymentStatus::Running);
}
