# Sliplane Integration Guide

## Overview

gitpub provides seamless integration with Sliplane for automatic deployment and hosting of your repositories.

## Setup

### 1. Get Sliplane API Credentials

1. Sign up for a Sliplane account at https://sliplane.io
2. Navigate to your account settings
3. Generate an API key

### 2. Configure gitpub

Set your Sliplane API credentials:

```bash
export SLIPLANE_API_URL="https://api.sliplane.io"
export SLIPLANE_API_KEY="your-api-key-here"
```

### 3. Configure Deployment

Create a deployment configuration for your repository:

```rust
use gitpub_sliplane::{DeploymentConfig, Environment, SliplaneClient};

let client = SliplaneClient::new(std::env::var("SLIPLANE_API_URL")?)
    .with_api_key(std::env::var("SLIPLANE_API_KEY")?);

let config = DeploymentConfig {
    repository_name: "my-app".to_string(),
    branch: "main".to_string(),
    environment: Environment::Production,
    auto_scale: true,
};

let result = client.deploy(&config).await?;
```

## Deployment Environments

gitpub supports three deployment environments:

- **Development**: For testing and development
- **Staging**: Pre-production environment
- **Production**: Live production deployment

## Auto-Scaling

Enable auto-scaling in your deployment configuration:

```rust
let config = DeploymentConfig {
    auto_scale: true,
    // ... other config
};
```

Sliplane will automatically scale your application based on load.

## Deployment Status

Check deployment status:

```rust
let status = client.get_deployment_status(&deployment_id).await?;

match status {
    DeploymentStatus::Pending => println!("Deployment queued"),
    DeploymentStatus::Building => println!("Building application"),
    DeploymentStatus::Running => println!("Deployment successful"),
    DeploymentStatus::Failed => println!("Deployment failed"),
}
```

## Webhook Integration

gitpub can automatically trigger deployments on push:

1. Configure webhook in repository settings
2. Set webhook URL to your Sliplane endpoint
3. Select "push" events
4. Deployments will trigger automatically

## Best Practices

- Use separate environments for development and production
- Enable auto-scaling for production deployments
- Monitor deployment status via the Sliplane dashboard
- Set up alerts for deployment failures

## Troubleshooting

### Deployment Fails

- Check API credentials are correct
- Verify repository permissions
- Review Sliplane logs for errors

### Connection Issues

- Verify SLIPLANE_API_URL is correct
- Check network connectivity
- Ensure API key has required permissions
