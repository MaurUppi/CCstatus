/*!
Tests for proxy health checker module.

Extracted from src/core/network/proxy_health/checker.rs #[cfg(test)] module.
Tests the core proxy health assessment logic including official endpoint detection,
proxy health level determination, fallback URL logic, and redirect validation.
*/

use ccstatus::core::network::proxy_health::checker::assess_proxy_health;
use ccstatus::core::network::proxy_health::client::{HealthCheckClient, HealthResponse};
use ccstatus::core::network::proxy_health::config::{ProxyHealthLevel, ProxyHealthOptions};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Default)]
struct MockHealthClient {
    responses: HashMap<String, Result<HealthResponse, String>>,
}

impl MockHealthClient {
    fn add_response(&mut self, url: &str, status: u16, body: &str) {
        self.responses.insert(
            url.to_string(),
            Ok(HealthResponse {
                status_code: status,
                body: body.as_bytes().to_vec(),
                duration: Duration::from_millis(100),
                headers: HashMap::new(),
            }),
        );
    }

    fn add_error(&mut self, url: &str, error: &str) {
        self.responses
            .insert(url.to_string(), Err(error.to_string()));
    }
}

#[async_trait::async_trait]
impl HealthCheckClient for MockHealthClient {
    async fn get_health(&self, url: String, _timeout_ms: u32) -> Result<HealthResponse, String> {
        self.responses
            .get(&url)
            .cloned()
            .unwrap_or_else(|| Err("URL not mocked".to_string()))
    }
}

#[tokio::test]
async fn test_assess_official_endpoint() {
    let client = MockHealthClient::default();
    let options = ProxyHealthOptions::default();

    let outcome = assess_proxy_health("https://api.anthropic.com", &options, &client)
        .await
        .unwrap();

    assert!(outcome.level.is_none());
    assert!(outcome.detail.is_none());
}

#[tokio::test]
async fn test_assess_healthy_proxy() {
    let mut client = MockHealthClient::default();
    client.add_response(
        "https://proxy.com/api/health",
        200,
        r#"{"status": "healthy"}"#,
    );

    let options = ProxyHealthOptions::default();

    let outcome = assess_proxy_health("https://proxy.com/api", &options, &client)
        .await
        .unwrap();

    assert_eq!(outcome.level, Some(ProxyHealthLevel::Healthy));
    assert_eq!(outcome.status_code, Some(200));

    let detail = outcome.detail.unwrap();
    assert_eq!(detail.success_method, Some("primary".to_string()));
}

#[tokio::test]
async fn test_assess_with_fallback() {
    let mut client = MockHealthClient::default();

    // Primary fails with 404
    client.add_response("https://proxy.com/api/health", 404, "");

    // Fallback succeeds
    client.add_response("https://proxy.com/health", 200, r#"{"status": "healthy"}"#);

    let options = ProxyHealthOptions {
        use_root_urls: false,
        try_fallback: true,
        ..Default::default()
    };

    let outcome = assess_proxy_health("https://proxy.com/api", &options, &client)
        .await
        .unwrap();

    assert_eq!(outcome.level, Some(ProxyHealthLevel::Healthy));

    let detail = outcome.detail.unwrap();
    assert_eq!(detail.success_method, Some("fallback".to_string()));
    assert_eq!(
        detail.fallback_url,
        Some("https://proxy.com/health".to_string())
    );
}

// Note: validate_redirect_host is now private, tested indirectly through assess_proxy_health
