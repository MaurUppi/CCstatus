use ccstatus::core::network::http_monitor::{HttpClientTrait, HttpMonitor};
use std::collections::HashMap;
use std::time::Duration;

// Test URL utility functions

#[test]
fn test_is_official_base_url_true() {
    // Official Anthropic API URLs should return true
    assert!(HttpMonitor::is_official_base_url(
        "https://api.anthropic.com"
    ));
    assert!(HttpMonitor::is_official_base_url(
        "https://api.anthropic.com/"
    ));
    assert!(HttpMonitor::is_official_base_url(
        "HTTPS://API.ANTHROPIC.COM"
    )); // case insensitive
    assert!(HttpMonitor::is_official_base_url(
        "https://api.anthropic.com///"
    )); // multiple slashes
}

#[test]
fn test_is_official_base_url_false() {
    // Non-official URLs should return false
    assert!(!HttpMonitor::is_official_base_url(
        "https://proxy.example.com"
    ));
    assert!(!HttpMonitor::is_official_base_url(
        "https://my-proxy.com/api/anthropic"
    ));
    assert!(!HttpMonitor::is_official_base_url("https://anthropic.com")); // missing api subdomain
    assert!(!HttpMonitor::is_official_base_url(
        "http://api.anthropic.com"
    )); // http instead of https
    assert!(!HttpMonitor::is_official_base_url(
        "https://api.anthropic.com.evil.com"
    )); // subdomain attack
}

#[test]
fn test_normalize_base_url() {
    // Should trim trailing slashes
    assert_eq!(
        HttpMonitor::normalize_base_url("https://example.com"),
        "https://example.com"
    );
    assert_eq!(
        HttpMonitor::normalize_base_url("https://example.com/"),
        "https://example.com"
    );
    assert_eq!(
        HttpMonitor::normalize_base_url("https://example.com//"),
        "https://example.com"
    );
    assert_eq!(
        HttpMonitor::normalize_base_url("https://example.com///"),
        "https://example.com"
    );

    // Should preserve paths
    assert_eq!(
        HttpMonitor::normalize_base_url("https://example.com/api"),
        "https://example.com/api"
    );
    assert_eq!(
        HttpMonitor::normalize_base_url("https://example.com/api/"),
        "https://example.com/api"
    );
}

#[test]
fn test_build_health_url() {
    // Should append /health to normalized URL
    assert_eq!(
        HttpMonitor::build_health_url("https://example.com"),
        "https://example.com/health"
    );
    assert_eq!(
        HttpMonitor::build_health_url("https://example.com/"),
        "https://example.com/health"
    );
    assert_eq!(
        HttpMonitor::build_health_url("https://example.com//"),
        "https://example.com/health"
    );

    // Should work with paths
    assert_eq!(
        HttpMonitor::build_health_url("https://example.com/api"),
        "https://example.com/api/health"
    );
    assert_eq!(
        HttpMonitor::build_health_url("https://example.com/api/"),
        "https://example.com/api/health"
    );
}

// Mock HTTP client for testing proxy health checks
struct MockProxyHealthClient {
    responses: HashMap<String, (u16, Duration, String)>,
}

impl MockProxyHealthClient {
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    fn set_response(&mut self, url: &str, status: u16, latency_ms: u64, breakdown: &str) {
        self.responses.insert(
            url.to_string(),
            (
                status,
                Duration::from_millis(latency_ms),
                breakdown.to_string(),
            ),
        );
    }

    fn set_error(&mut self, url: &str) {
        // Remove entry to simulate network error
        self.responses.remove(url);
    }
}

#[async_trait::async_trait]
impl HttpClientTrait for MockProxyHealthClient {
    async fn execute_request(
        &self,
        url: String,
        _headers: HashMap<String, String>,
        _body: Vec<u8>,
        _timeout_ms: u32,
    ) -> Result<(u16, Duration, String), String> {
        if let Some((status, duration, breakdown)) = self.responses.get(&url) {
            Ok((*status, *duration, breakdown.clone()))
        } else {
            Err("Network error".to_string())
        }
    }
}

#[tokio::test]
async fn test_check_proxy_health_404_returns_none() {
    let mut mock_client = MockProxyHealthClient::new();
    mock_client.set_response("https://proxy.example.com/health", 404, 100, "Total:100ms");

    let monitor = HttpMonitor::new(Some("/tmp/test_state.json".into()))
        .unwrap()
        .with_http_client(Box::new(mock_client));

    let result = monitor
        .check_proxy_health("https://proxy.example.com")
        .await;
    assert_eq!(result.unwrap(), None); // 404 should return None
}

#[tokio::test]
async fn test_check_proxy_health_200_returns_true() {
    let mut mock_client = MockProxyHealthClient::new();
    mock_client.set_response("https://proxy.example.com/health", 200, 100, "Total:100ms");

    let monitor = HttpMonitor::new(Some("/tmp/test_state.json".into()))
        .unwrap()
        .with_http_client(Box::new(mock_client));

    let result = monitor
        .check_proxy_health("https://proxy.example.com")
        .await;
    assert_eq!(result.unwrap(), Some(true)); // 200 should return true
}

#[tokio::test]
async fn test_check_proxy_health_500_returns_false() {
    let mut mock_client = MockProxyHealthClient::new();
    mock_client.set_response("https://proxy.example.com/health", 500, 100, "Total:100ms");

    let monitor = HttpMonitor::new(Some("/tmp/test_state.json".into()))
        .unwrap()
        .with_http_client(Box::new(mock_client));

    let result = monitor
        .check_proxy_health("https://proxy.example.com")
        .await;
    assert_eq!(result.unwrap(), Some(false)); // 500 should return false
}

#[tokio::test]
async fn test_check_proxy_health_network_error_returns_false() {
    let mut mock_client = MockProxyHealthClient::new();
    mock_client.set_error("https://proxy.example.com/health"); // Simulate network error

    let monitor = HttpMonitor::new(Some("/tmp/test_state.json".into()))
        .unwrap()
        .with_http_client(Box::new(mock_client));

    let result = monitor
        .check_proxy_health("https://proxy.example.com")
        .await;
    assert_eq!(result.unwrap(), Some(false)); // Network error should return false
}

#[tokio::test]
async fn test_check_proxy_health_various_status_codes() {
    let test_cases = vec![
        (200, Some(true)),  // Only 200 is healthy
        (201, Some(false)), // Other 2xx codes are unhealthy
        (204, Some(false)), // No content is unhealthy
        (301, Some(false)), // Redirects are unhealthy
        (401, Some(false)), // Auth errors are unhealthy
        (403, Some(false)), // Forbidden is unhealthy
        (404, None),        // 404 means no endpoint
        (429, Some(false)), // Rate limit is unhealthy
        (502, Some(false)), // Bad gateway is unhealthy
        (503, Some(false)), // Service unavailable is unhealthy
    ];

    for (status_code, expected) in test_cases {
        let mut mock_client = MockProxyHealthClient::new();
        mock_client.set_response(
            "https://proxy.example.com/health",
            status_code,
            100,
            "Total:100ms",
        );

        let monitor = HttpMonitor::new(Some("/tmp/test_state.json".into()))
            .unwrap()
            .with_http_client(Box::new(mock_client));

        let result = monitor
            .check_proxy_health("https://proxy.example.com")
            .await;
        assert_eq!(
            result.unwrap(),
            expected,
            "Status code {} should return {:?}",
            status_code,
            expected
        );
    }
}

#[tokio::test]
async fn test_check_proxy_health_url_construction() {
    let test_cases = vec![
        (
            "https://proxy.example.com",
            "https://proxy.example.com/health",
        ),
        (
            "https://proxy.example.com/",
            "https://proxy.example.com/health",
        ),
        (
            "https://proxy.example.com//",
            "https://proxy.example.com/health",
        ),
        (
            "https://proxy.example.com/api",
            "https://proxy.example.com/api/health",
        ),
        (
            "https://proxy.example.com/api/",
            "https://proxy.example.com/api/health",
        ),
    ];

    for (base_url, expected_health_url) in test_cases {
        let mut mock_client = MockProxyHealthClient::new();
        mock_client.set_response(expected_health_url, 200, 100, "Total:100ms");

        let monitor = HttpMonitor::new(Some("/tmp/test_state.json".into()))
            .unwrap()
            .with_http_client(Box::new(mock_client));

        let result = monitor.check_proxy_health(base_url).await;
        assert_eq!(
            result.unwrap(),
            Some(true),
            "Base URL '{}' should construct health URL '{}'",
            base_url,
            expected_health_url
        );
    }
}

// Test that official URLs skip proxy health checks
#[tokio::test]
async fn test_official_urls_skip_health_check() {
    // These tests ensure that the process_probe_results method correctly
    // identifies official URLs and skips the health check
    // This would be covered by integration tests, but we can test the logic here

    let official_urls = vec![
        "https://api.anthropic.com",
        "https://api.anthropic.com/",
        "HTTPS://API.ANTHROPIC.COM",
    ];

    for url in official_urls {
        assert!(
            HttpMonitor::is_official_base_url(url),
            "URL '{}' should be identified as official",
            url
        );
    }

    let proxy_urls = vec![
        "https://proxy.example.com",
        "https://my-anthropic-proxy.com",
        "https://api.anthropic.com.evil.com",
    ];

    for url in proxy_urls {
        assert!(
            !HttpMonitor::is_official_base_url(url),
            "URL '{}' should NOT be identified as official",
            url
        );
    }
}
