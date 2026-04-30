use anyhow::Result;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{Deserialize, Serialize};

/// Sliplane API client
pub struct SliplaneClient {
    api_url: String,
    api_key: Option<String>,
    client: ClientWithMiddleware,
}

impl SliplaneClient {
    pub fn new(api_url: String) -> Self {
        // Configure retry policy: max 3 retries with exponential backoff
        // Initial delay: 1s, max delay: 10s
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(
                std::time::Duration::from_secs(1),
                std::time::Duration::from_secs(10),
            )
            .build_with_max_retries(3);

        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            api_url,
            api_key: None,
            client,
        }
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub async fn deploy(&self, config: &DeploymentConfig) -> Result<DeploymentResult> {
        let url = format!("{}/deployments", self.api_url);

        let mut request = self.client.post(&url).json(&config);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Deployment failed with status: {}", response.status());
        }

        let result: DeploymentResult = response.json().await?;
        Ok(result)
    }

    pub async fn get_deployment_status(&self, deployment_id: &str) -> Result<DeploymentStatus> {
        let url = format!("{}/deployments/{}/status", self.api_url, deployment_id);

        let mut request = self.client.get(&url);

        if let Some(ref key) = self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get deployment status: {}", response.status());
        }

        let status: DeploymentStatus = response.json().await?;
        Ok(status)
    }
}

/// Deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub repository_name: String,
    pub branch: String,
    pub environment: Environment,
    pub auto_scale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

/// Deployment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub deployment_id: String,
    pub status: DeploymentStatus,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStatus {
    Pending,
    Building,
    Running,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = SliplaneClient::new("https://api.sliplane.io".to_string());
        assert_eq!(client.api_url, "https://api.sliplane.io");
    }

    #[test]
    fn test_client_with_api_key() {
        let client = SliplaneClient::new("https://api.sliplane.io".to_string())
            .with_api_key("test-key".to_string());
        assert_eq!(client.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_deployment_config() {
        let config = DeploymentConfig {
            repository_name: "test-repo".to_string(),
            branch: "main".to_string(),
            environment: Environment::Development,
            auto_scale: true,
        };
        assert_eq!(config.repository_name, "test-repo");
    }

    #[tokio::test]
    async fn test_deploy_with_retry_on_500_error() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // First two requests fail with 500, third succeeds
        Mock::given(method("POST"))
            .and(path("/deployments"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_string("Internal Server Error")
                    .insert_header("content-type", "text/plain"),
            )
            .up_to_n_times(2)
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/deployments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "deployment_id": "test-123",
                "status": "Pending",
                "url": null
            })))
            .mount(&mock_server)
            .await;

        let client = SliplaneClient::new(mock_server.uri()).with_api_key("test-key".to_string());

        let config = DeploymentConfig {
            repository_name: "test-repo".to_string(),
            branch: "main".to_string(),
            environment: Environment::Development,
            auto_scale: true,
        };

        let result = client.deploy(&config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().deployment_id, "test-123");
    }

    #[tokio::test]
    async fn test_deploy_with_retry_on_connection_failure() {
        // Use an unreachable port to simulate connection failure
        let client = SliplaneClient::new("http://localhost:1".to_string())
            .with_api_key("test-key".to_string());

        let config = DeploymentConfig {
            repository_name: "test-repo".to_string(),
            branch: "main".to_string(),
            environment: Environment::Development,
            auto_scale: true,
        };

        let result = client.deploy(&config).await;
        // Should fail after retries
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_deployment_status_with_retry() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // First request fails with 503, second succeeds
        Mock::given(method("GET"))
            .and(path("/deployments/test-123/status"))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_string("Service Unavailable")
                    .insert_header("content-type", "text/plain"),
            )
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/deployments/test-123/status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!("Running")))
            .mount(&mock_server)
            .await;

        let client = SliplaneClient::new(mock_server.uri()).with_api_key("test-key".to_string());

        let result = client.get_deployment_status("test-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_no_retry_on_4xx_errors() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // 404 should not be retried
        Mock::given(method("POST"))
            .and(path("/deployments"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_string("Not Found")
                    .insert_header("content-type", "text/plain"),
            )
            .expect(1) // Should only be called once (no retries)
            .mount(&mock_server)
            .await;

        let client = SliplaneClient::new(mock_server.uri()).with_api_key("test-key".to_string());

        let config = DeploymentConfig {
            repository_name: "test-repo".to_string(),
            branch: "main".to_string(),
            environment: Environment::Development,
            auto_scale: true,
        };

        let result = client.deploy(&config).await;
        assert!(result.is_err());
    }
}
