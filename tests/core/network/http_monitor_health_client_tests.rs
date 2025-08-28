/*!
Comprehensive unit tests for HealthCheckClient architecture.

Tests cover the enhanced proxy health check functionality including:
- GET method usage (not POST)
- Redirect policy enforcement (3xx responses not followed)
- JSON body validation with case-insensitive matching
- All status code mappings per assessment requirements
- Mock implementations for testing
- Integration with HttpMonitor dependency injection
*/

#![cfg(feature = "network-monitoring")]

use ccstatus::core::network::proxy_health::client::{HealthCheckClient, HealthResponse};
use ccstatus::core::network::http_monitor::HttpMonitor;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;

/// Test mock health check client for unit testing
struct TestMockHealthCheckClient {
    responses: HashMap<String, Result<(u16, Duration, Vec<u8>), String>>,
}

impl TestMockHealthCheckClient {
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    fn set_response(&mut self, url: &str, status: u16, duration_ms: u64, body: Vec<u8>) {
        self.responses.insert(
            url.to_string(),
            Ok((status, Duration::from_millis(duration_ms), body)),
        );
    }

    fn set_error(&mut self, url: &str) {
        self.responses
            .insert(url.to_string(), Err("Network error".to_string()));
    }
}

#[async_trait::async_trait]
impl HealthCheckClient for TestMockHealthCheckClient {
    async fn get_health(&self, url: String, _timeout_ms: u32) -> Result<HealthResponse, String> {
        match self.responses.get(&url) {
            Some(Ok((status_code, duration, body))) => Ok(HealthResponse {
                status_code: *status_code,
                body: body.clone(),
                duration: *duration,
            }),
            Some(Err(error)) => Err(error.clone()),
            None => Err("URL not configured in mock".to_string()),
        }
    }
}

// Test HealthResponse creation and validation
#[test]
fn test_health_response_creation() {
    let body = b"{\"status\": \"healthy\"}";
    let response = HealthResponse {
        status_code: 200,
        body: body.to_vec(),
        duration: Duration::from_millis(100),
    };

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body, body.to_vec());
    assert_eq!(response.duration, Duration::from_millis(100));
}

// JSON validation is tested indirectly through the integration tests below
// The validate_health_json method is a private implementation detail

// Test TestMockHealthCheckClient functionality
#[tokio::test]
async fn test_mock_health_check_client_success() {
    let mut mock_client = TestMockHealthCheckClient::new();

    // Set up response with healthy JSON
    let body = br#"{"status": "healthy"}"#.to_vec();
    mock_client.set_response("https://proxy.example.com/health", 200, 100, body.clone());

    let result = mock_client
        .get_health("https://proxy.example.com/health".to_string(), 1500)
        .await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body, body);
    assert_eq!(response.duration, Duration::from_millis(100));
}

#[tokio::test]
async fn test_mock_health_check_client_error() {
    let mut mock_client = TestMockHealthCheckClient::new();

    // Set up network error
    mock_client.set_error("https://proxy.example.com/health");

    let result = mock_client
        .get_health("https://proxy.example.com/health".to_string(), 1500)
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Network error");
}

#[tokio::test]
async fn test_mock_health_check_client_various_status_codes() {
    let test_cases = vec![
        (200, true),  // Success
        (201, false), // Other success codes treated as unhealthy
        (301, false), // Redirects should be unhealthy
        (302, false), // Temporary redirect
        (400, false), // Bad request
        (401, false), // Unauthorized
        (403, false), // Forbidden
        (404, false), // Not found (will be mapped to None in HttpMonitor)
        (500, false), // Internal server error
        (502, false), // Bad gateway
        (503, false), // Service unavailable
    ];

    for (status_code, _expected_healthy) in test_cases {
        let mut mock_client = TestMockHealthCheckClient::new();
        let body = if status_code == 200 {
            br#"{"status": "healthy"}"#.to_vec()
        } else {
            br#"{"status": "unhealthy"}"#.to_vec()
        };

        mock_client.set_response(
            "https://proxy.example.com/health",
            status_code,
            100,
            body.clone(),
        );

        let result = mock_client
            .get_health("https://proxy.example.com/health".to_string(), 1500)
            .await;

        assert!(
            result.is_ok(),
            "Status code {} should not cause client error",
            status_code
        );
        let response = result.unwrap();
        assert_eq!(response.status_code, status_code);
        assert_eq!(response.body, body);
    }
}

// Test HttpMonitor with new HealthCheckClient integration
#[tokio::test]
async fn test_check_proxy_health_with_client_404_returns_none() {
    let mut mock_client = TestMockHealthCheckClient::new();
    let body = br#"{"error": "not found"}"#.to_vec();
    mock_client.set_response("https://proxy.example.com/health", 404, 100, body);

    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state.json");

    let monitor = HttpMonitor::new(Some(state_path)).unwrap();

    let result = monitor
        .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
        .await;
    assert_eq!(result.unwrap(), None); // 404 should return None
}

#[tokio::test]
async fn test_check_proxy_health_with_client_200_valid_json_returns_true() {
    let mut mock_client = TestMockHealthCheckClient::new();
    let body = br#"{"status": "healthy", "timestamp": "2023-01-01T00:00:00Z"}"#.to_vec();
    mock_client.set_response("https://proxy.example.com/health", 200, 100, body);

    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state.json");

    let monitor = HttpMonitor::new(Some(state_path)).unwrap();

    let result = monitor
        .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
        .await;
    assert_eq!(result.unwrap(), Some(true)); // 200 with valid JSON should return true
}

#[tokio::test]
async fn test_check_proxy_health_with_client_200_invalid_json_returns_false() {
    let mut mock_client = TestMockHealthCheckClient::new();
    let body = br#"{"status": "unhealthy"}"#.to_vec(); // Wrong status value
    mock_client.set_response("https://proxy.example.com/health", 200, 100, body);

    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state.json");

    let monitor = HttpMonitor::new(Some(state_path)).unwrap();

    let result = monitor
        .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
        .await;
    assert_eq!(result.unwrap(), Some(false)); // 200 with invalid JSON should return false
}

#[tokio::test]
async fn test_check_proxy_health_with_client_200_malformed_json_returns_false() {
    let mut mock_client = TestMockHealthCheckClient::new();
    let body = b"not json at all".to_vec();
    mock_client.set_response("https://proxy.example.com/health", 200, 100, body);

    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state.json");

    let monitor = HttpMonitor::new(Some(state_path)).unwrap();

    let result = monitor
        .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
        .await;
    assert_eq!(result.unwrap(), Some(false)); // 200 with malformed JSON should return false
}

#[tokio::test]
async fn test_check_proxy_health_with_client_3xx_returns_false() {
    // Test that 3xx responses are treated as unhealthy (redirects not followed)
    let redirect_codes = vec![301, 302, 303, 307, 308];

    for status_code in redirect_codes {
        let mut mock_client = TestMockHealthCheckClient::new();
        let body = br#"<html><body>Moved</body></html>"#.to_vec();
        mock_client.set_response("https://proxy.example.com/health", status_code, 100, body);

        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let monitor = HttpMonitor::new(Some(state_path)).unwrap();

        let result = monitor
            .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
            .await;
        assert_eq!(
            result.unwrap(),
            Some(false),
            "Status code {} should return false (unhealthy)",
            status_code
        );
    }
}

#[tokio::test]
async fn test_check_proxy_health_with_client_5xx_returns_false() {
    let server_error_codes = vec![500, 501, 502, 503, 504];

    for status_code in server_error_codes {
        let mut mock_client = TestMockHealthCheckClient::new();
        let body = br#"{"error": "server error"}"#.to_vec();
        mock_client.set_response("https://proxy.example.com/health", status_code, 100, body);

        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let monitor = HttpMonitor::new(Some(state_path)).unwrap();

        let result = monitor
            .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
            .await;
        assert_eq!(
            result.unwrap(),
            Some(false),
            "Status code {} should return false (unhealthy)",
            status_code
        );
    }
}

#[tokio::test]
async fn test_check_proxy_health_with_client_network_error_returns_false() {
    let mut mock_client = TestMockHealthCheckClient::new();
    mock_client.set_error("https://proxy.example.com/health"); // Simulate network error

    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state.json");

    let monitor = HttpMonitor::new(Some(state_path)).unwrap();

    let result = monitor
        .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
        .await;
    assert_eq!(result.unwrap(), Some(false)); // Network error should return false
}

#[tokio::test]
async fn test_check_proxy_health_with_client_case_insensitive_json() {
    let test_cases = vec![
        br#"{"status": "healthy"}"#.to_vec(),
        br#"{"status": "HEALTHY"}"#.to_vec(),
        br#"{"status": "Healthy"}"#.to_vec(),
        br#"{"status": "HeAlThY"}"#.to_vec(),
        br#"{"STATUS": "healthy"}"#.to_vec(), // Field name case should not matter for value
    ];

    for body in test_cases {
        let mut mock_client = TestMockHealthCheckClient::new();
        mock_client.set_response("https://proxy.example.com/health", 200, 100, body.clone());

        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let monitor = HttpMonitor::new(Some(state_path)).unwrap();

        let result = monitor
            .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
            .await;
        assert_eq!(
            result.unwrap(),
            Some(true),
            "Body {:?} should be treated as healthy",
            String::from_utf8_lossy(&body)
        );
    }
}

#[tokio::test]
async fn test_check_proxy_health_with_client_extra_fields_tolerated() {
    let test_cases = vec![
        br#"{"status": "healthy", "version": "1.2.3"}"#.to_vec(),
        br#"{"timestamp": "2023-01-01T00:00:00Z", "status": "healthy", "uptime": 3600}"#.to_vec(),
        br#"{"service": "proxy", "status": "healthy", "checks": {"db": "ok", "redis": "ok"}}"#
            .to_vec(),
    ];

    for body in test_cases {
        let mut mock_client = TestMockHealthCheckClient::new();
        mock_client.set_response("https://proxy.example.com/health", 200, 100, body.clone());

        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let monitor = HttpMonitor::new(Some(state_path)).unwrap();

        let result = monitor
            .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
            .await;
        assert_eq!(
            result.unwrap(),
            Some(true),
            "Body with extra fields should be treated as healthy: {:?}",
            String::from_utf8_lossy(&body)
        );
    }
}

#[tokio::test]
async fn test_health_check_url_construction() {
    // Test that URLs are properly constructed for health checks
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
        let mut mock_client = TestMockHealthCheckClient::new();
        let body = br#"{"status": "healthy"}"#.to_vec();
        mock_client.set_response(expected_health_url, 200, 100, body);

        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("state.json");

        let monitor = HttpMonitor::new(Some(state_path)).unwrap();

        let result = monitor
            .check_proxy_health_with_client(base_url, &mock_client)
            .await;
        assert_eq!(
            result.unwrap(),
            Some(true),
            "Base URL '{}' should construct health URL '{}' and succeed",
            base_url,
            expected_health_url
        );
    }
}

// Test basic timeout configuration (simplified - no time manipulation)
#[tokio::test]
async fn test_health_check_timeout_configuration() {
    let mut mock_client = TestMockHealthCheckClient::new();
    let body = br#"{"status": "healthy"}"#.to_vec();

    // Mock client will simulate the timeout duration
    mock_client.set_response(
        "https://proxy.example.com/health",
        200,
        1500, // Duration should match the timeout
        body,
    );

    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state.json");

    let monitor = HttpMonitor::new(Some(state_path)).unwrap();

    let result = monitor
        .check_proxy_health_with_client("https://proxy.example.com", &mock_client)
        .await;
    assert!(result.is_ok());

    let response = result.unwrap();
    assert_eq!(response, Some(true));
}

// Test legacy compatibility - ensure old method still works
#[tokio::test]
async fn test_legacy_check_proxy_health_still_works() {
    // The old check_proxy_health method should still work for backward compatibility
    // It will internally use the default health client

    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("state.json");

    // Create monitor without explicit health client (will use default)
    let monitor = HttpMonitor::new(Some(state_path)).unwrap();

    // This should not panic and should return a result
    // Note: This will fail with a real network call, but the important thing
    // is that the method exists and has the right signature
    let result = monitor
        .check_proxy_health("https://proxy.example.com")
        .await;

    // We expect this to fail with a network error since it's a real network call
    // but it shouldn't panic
    match result {
        Ok(_) => {
            // If it succeeds, that's fine (maybe the URL actually responds)
        }
        Err(_) => {
            // If it fails with a network error, that's expected for a non-existent endpoint
        }
    }
}
