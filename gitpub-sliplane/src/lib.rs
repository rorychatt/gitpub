use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Sliplane API client
pub struct SliplaneClient {
    api_url: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl SliplaneClient {
    pub fn new(api_url: String) -> Self {
        Self {
            api_url,
            api_key: None,
            client: reqwest::Client::new(),
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
}
