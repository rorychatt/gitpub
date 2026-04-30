use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Sliplane API client
pub struct SliplaneClient {
    api_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl SliplaneClient {
    pub fn new(api_url: String) -> Self {
        // Use default timeouts: 10s connect, 30s request
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client with default timeouts");

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

    pub fn with_timeouts(
        api_url: String,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Self {
        let client = reqwest::ClientBuilder::new()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .expect("Failed to build HTTP client with custom timeouts");

        Self {
            api_url,
            api_key: None,
            client,
        }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub repository_name: String,
    pub branch: String,
    pub environment: Environment,
    pub auto_scale: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

/// Deployment result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub deployment_id: String,
    pub status: DeploymentStatus,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    #[test]
    fn test_client_with_default_timeouts() {
        let client = SliplaneClient::new("https://api.sliplane.io".to_string());
        assert_eq!(client.api_url, "https://api.sliplane.io");
        // Client is created successfully with default timeouts
    }

    #[test]
    fn test_client_with_custom_timeouts() {
        let client = SliplaneClient::with_timeouts(
            "https://api.sliplane.io".to_string(),
            Duration::from_secs(5),
            Duration::from_secs(15),
        );
        assert_eq!(client.api_url, "https://api.sliplane.io");
        // Client is created successfully with custom timeouts
    }

    #[test]
    fn test_client_with_custom_timeouts_and_api_key() {
        let client = SliplaneClient::with_timeouts(
            "https://api.sliplane.io".to_string(),
            Duration::from_secs(5),
            Duration::from_secs(15),
        )
        .with_api_key("test-key".to_string());
        assert_eq!(client.api_key, Some("test-key".to_string()));
    }
}
