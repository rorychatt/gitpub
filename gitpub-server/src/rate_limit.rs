use governor::middleware::NoOpMiddleware;
use std::sync::Arc;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::GlobalKeyExtractor, GovernorLayer,
};

/// Creates a rate limiting layer for authentication endpoints.
/// Limits to 5 requests per minute globally (all IPs combined).
///
/// Note: Using GlobalKeyExtractor for simplicity. In production with a reverse proxy,
/// consider SmartIpKeyExtractor or a custom extractor that properly handles
/// X-Forwarded-For headers.
pub fn create_auth_rate_limiter() -> GovernorLayer<GlobalKeyExtractor, NoOpMiddleware> {
    let governor_conf = GovernorConfigBuilder::default()
        .per_millisecond(60000 / 5) // 1 request per 12 seconds
        .burst_size(5)
        .key_extractor(GlobalKeyExtractor)
        .finish()
        .unwrap();

    GovernorLayer {
        config: Arc::new(governor_conf),
    }
}
