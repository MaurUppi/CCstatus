//! End-to-end integration tests for proxy health functionality
//!
//! These tests verify the complete proxy health workflow from HttpMonitor
//! through to the tri-state health levels and centralized field mapping.

#[cfg(test)]
mod tests {
    use crate::core::network::types::*;
    use crate::core::network::proxy_health::ProxyHealthLevel;
    use crate::core::network::http_monitor::{HttpMonitor, ClockTrait, HttpClientTrait};
    use crate::core::network::proxy_health::{HealthCheckClient, HealthResponse};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    /// Mock clock for consistent test timing
    struct MockClock {
        time: Instant,
        timestamp: String,
    }

    impl MockClock {
        fn new() -> Self {
            Self {
                time: Instant::now(),
                timestamp: "2025-08-28T10:30:00-07:00".to_string(),
            }
        }
    }

    impl ClockTrait for MockClock {
        fn now(&self) -> Instant {
            self.time
        }

        fn local_timestamp(&self) -> String {
            self.timestamp.clone()
        }
    }

    /// Mock HTTP client that always returns success for API calls
    struct MockHttpClient;

    #[async_trait::async_trait]
    impl HttpClientTrait for MockHttpClient {
        async fn execute_request(
            &self,
            _url: String,
            _headers: HashMap<String, String>,
            _body: Vec<u8>,
            _timeout_ms: u32,
        ) -> Result<(u16, Duration, String), String> {
            let duration = Duration::from_millis(250);
            let breakdown = "DNS:10ms|TCP:20ms|TLS:30ms|TTFB:190ms|Total:250ms".to_string();
            Ok((200, duration, breakdown))
        }
    }

    /// Mock health check client with configurable responses
    struct MockHealthCheckClient {
        responses: Arc<Mutex<Vec<Result<HealthResponse, String>>>>,
    }

    impl MockHealthCheckClient {
        fn new(responses: Vec<Result<HealthResponse, String>>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses)),
            }
        }
    }

    #[async_trait::async_trait]
    impl HealthCheckClient for MockHealthCheckClient {
        async fn get_health(&self, _url: String, _timeout_ms: u32) -> Result<HealthResponse, String> {
            let mut responses = self.responses.lock().unwrap();
            if let Some(response) = responses.pop() {
                response
            } else {
                Err("No more mock responses available".to_string())
            }
        }
    }

    /// Create a healthy health response
    fn create_healthy_response() -> HealthResponse {
        use std::collections::HashMap;
        HealthResponse {
            status_code: 200,
            body: r#"{"status": "healthy"}"#.as_bytes().to_vec(),
            duration: Duration::from_millis(100),
            headers: HashMap::new(),
        }
    }

    /// Create a degraded health response (429 rate limited)
    fn create_degraded_response() -> HealthResponse {
        use std::collections::HashMap;
        HealthResponse {
            status_code: 429,
            body: r#"{"status": "degraded", "reason": "rate_limited"}"#.as_bytes().to_vec(),
            duration: Duration::from_millis(150),
            headers: HashMap::new(),
        }
    }

    /// Create an unhealthy health response
    fn create_unhealthy_response() -> HealthResponse {
        use std::collections::HashMap;
        HealthResponse {
            status_code: 500,
            body: r#"{"status": "error"}"#.as_bytes().to_vec(),
            duration: Duration::from_millis(200),
            headers: HashMap::new(),
        }
    }

    /// Create a 404 not found response (no health endpoint)
    fn create_not_found_response() -> HealthResponse {
        use std::collections::HashMap;
        HealthResponse {
            status_code: 404,
            body: b"Not Found".to_vec(),
            duration: Duration::from_millis(50),
            headers: HashMap::new(),
        }
    }

    /// Test credentials for a proxy endpoint
    fn create_proxy_credentials() -> ApiCredentials {
        ApiCredentials {
            base_url: "https://proxy.example.com".to_string(),
            auth_token: "test_token".to_string(),
            source: CredentialSource::Environment,
        }
    }

    /// Test credentials for official Anthropic endpoint
    fn create_official_credentials() -> ApiCredentials {
        ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test_token".to_string(),
            source: CredentialSource::Environment,
        }
    }

    #[tokio::test]
    async fn test_proxy_health_integration_healthy() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_monitoring.json");
        
        let health_responses = vec![Ok(create_healthy_response())];
        let health_client = Box::new(MockHealthCheckClient::new(health_responses));
        
        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(MockHttpClient))
            .with_health_client(health_client)
            .with_clock(Box::new(MockClock::new()));

        // Execute probe with proxy credentials
        let _result = monitor.probe(
            ProbeMode::Green,
            create_proxy_credentials(),
            None,
        ).await.unwrap();

        let result = _result;
        // Verify probe outcome
        assert_eq!(result.status, NetworkStatus::Healthy);
        assert_eq!(result.metrics.last_http_status, 200);

        // Load and verify state file
        let state = monitor.load_state().await.unwrap();
        
        // Verify legacy field is set correctly
        assert_eq!(state.network.proxy_healthy, Some(true));
        
        // Verify new tri-state field is set correctly
        assert_eq!(state.network.get_proxy_health_level(), Some(ProxyHealthLevel::Healthy));
        
        // Verify detail information is present
        assert!(state.network.proxy_health_detail.is_some());
        let detail = state.network.proxy_health_detail.unwrap();
        assert_eq!(detail.primary_url, "https://proxy.example.com/health");
        assert!(detail.success_method.is_some());
    }

    #[tokio::test]
    async fn test_proxy_health_integration_degraded() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_monitoring.json");
        
        let health_responses = vec![Ok(create_degraded_response())];
        let health_client = Box::new(MockHealthCheckClient::new(health_responses));
        
        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(MockHttpClient))
            .with_health_client(health_client)
            .with_clock(Box::new(MockClock::new()));

        // Execute probe
        let result = monitor.probe(
            ProbeMode::Green,
            create_proxy_credentials(),
            None,
        ).await.unwrap();

        // Verify probe outcome
        assert_eq!(result.status, NetworkStatus::Healthy); // API call was successful
        assert_eq!(result.metrics.last_http_status, 200);

        // Load and verify state
        let state = monitor.load_state().await.unwrap();
        
        // Verify tri-state mapping: degraded proxy → false legacy but degraded level
        assert_eq!(state.network.proxy_healthy, Some(false)); // Legacy compatibility
        assert_eq!(state.network.get_proxy_health_level(), Some(ProxyHealthLevel::Degraded));
    }

    #[tokio::test]
    async fn test_proxy_health_integration_unhealthy() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_monitoring.json");
        
        let health_responses = vec![Ok(create_unhealthy_response())];
        let health_client = Box::new(MockHealthCheckClient::new(health_responses));
        
        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(MockHttpClient))
            .with_health_client(health_client)
            .with_clock(Box::new(MockClock::new()));

        // Execute probe
        let _result = monitor.probe(
            ProbeMode::Green,
            create_proxy_credentials(),
            None,
        ).await.unwrap();

        // Load and verify state
        let state = monitor.load_state().await.unwrap();
        
        // Verify tri-state mapping: bad proxy → false legacy but bad level
        assert_eq!(state.network.proxy_healthy, Some(false));
        assert_eq!(state.network.get_proxy_health_level(), Some(ProxyHealthLevel::Bad));
    }

    #[tokio::test]
    async fn test_proxy_health_integration_no_endpoint() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_monitoring.json");
        
        // Need to provide 404 responses for both primary and fallback URLs since fallback is enabled
        let health_responses = vec![
            Ok(create_not_found_response()), // Primary URL: /api/health
            Ok(create_not_found_response()), // Fallback URL: /health
        ];
        let health_client = Box::new(MockHealthCheckClient::new(health_responses));
        
        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(MockHttpClient))
            .with_health_client(health_client)
            .with_clock(Box::new(MockClock::new()));

        // Execute probe
        let _result = monitor.probe(
            ProbeMode::Green,
            create_proxy_credentials(),
            None,
        ).await.unwrap();

        // Load and verify state
        let state = monitor.load_state().await.unwrap();
        
        // Verify no endpoint mapping: None for both fields
        assert_eq!(state.network.proxy_healthy, None);
        assert_eq!(state.network.get_proxy_health_level(), None);
    }

    #[tokio::test]
    async fn test_official_anthropic_endpoint_skips_proxy_check() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_monitoring.json");
        
        // No health responses needed - should not be called
        let health_responses = vec![];
        let health_client = Box::new(MockHealthCheckClient::new(health_responses));
        
        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(MockHttpClient))
            .with_health_client(health_client)
            .with_clock(Box::new(MockClock::new()));

        // Execute probe with official credentials
        let result = monitor.probe(
            ProbeMode::Green,
            create_official_credentials(),
            None,
        ).await.unwrap();

        // Verify probe outcome
        assert_eq!(result.status, NetworkStatus::Healthy);
        assert_eq!(result.metrics.last_http_status, 200);

        // Load and verify state
        let state = monitor.load_state().await.unwrap();
        
        // Verify official endpoint skips proxy checks
        assert_eq!(state.network.proxy_healthy, None);
        assert_eq!(state.network.get_proxy_health_level(), None);
        assert!(state.network.proxy_health_detail.is_none());
    }

    #[tokio::test]
    async fn test_centralized_field_mapping_consistency() {
        // Test the centralized NetworkMetrics::set_proxy_health function
        let mut metrics = NetworkMetrics::default();
        
        // Test healthy mapping
        let detail = crate::core::network::types::ProxyHealthDetail {
            primary_url: "https://proxy.example.com/health".to_string(),
            fallback_url: None,
            redirect_url: None,
            success_method: Some("primary".to_string()),
            checked_at: "2025-08-28T10:30:00-07:00".to_string(),
            response_time_ms: 100,
        };
        
        metrics.set_proxy_health(Some(ProxyHealthLevel::Healthy), Some(detail.clone()));
        assert_eq!(metrics.proxy_healthy, Some(true));
        assert_eq!(metrics.get_proxy_health_level(), Some(ProxyHealthLevel::Healthy));
        
        // Test degraded mapping
        metrics.set_proxy_health(Some(ProxyHealthLevel::Degraded), Some(detail.clone()));
        assert_eq!(metrics.proxy_healthy, Some(false));
        assert_eq!(metrics.get_proxy_health_level(), Some(ProxyHealthLevel::Degraded));
        
        // Test bad mapping
        metrics.set_proxy_health(Some(ProxyHealthLevel::Bad), Some(detail.clone()));
        assert_eq!(metrics.proxy_healthy, Some(false));
        assert_eq!(metrics.get_proxy_health_level(), Some(ProxyHealthLevel::Bad));
        
        // Test none mapping
        metrics.set_proxy_health(None, None);
        assert_eq!(metrics.proxy_healthy, None);
        assert_eq!(metrics.get_proxy_health_level(), None);
    }

    #[tokio::test]
    async fn test_legacy_field_fallback() {
        // Test backward compatibility with legacy proxy_healthy field
        let mut metrics = NetworkMetrics::default();
        
        // Set only legacy field (simulating old state file)
        metrics.proxy_healthy = Some(true);
        metrics.proxy_health_level = None;
        
        // Should fallback to legacy field
        assert_eq!(metrics.get_proxy_health_level(), Some(ProxyHealthLevel::Healthy));
        
        // Set legacy to false
        metrics.proxy_healthy = Some(false);
        assert_eq!(metrics.get_proxy_health_level(), Some(ProxyHealthLevel::Bad)); // Default mapping
        
        // Set both fields - new field should take priority
        metrics.proxy_healthy = Some(true);
        metrics.proxy_health_level = Some(ProxyHealthLevel::Degraded);
        assert_eq!(metrics.get_proxy_health_level(), Some(ProxyHealthLevel::Degraded));
    }

    #[tokio::test]
    async fn test_error_handling_in_integration() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("test_monitoring.json");
        
        // Health client that returns error
        let health_responses = vec![Err("Network error".to_string())];
        let health_client = Box::new(MockHealthCheckClient::new(health_responses));
        
        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(MockHttpClient))
            .with_health_client(health_client)
            .with_clock(Box::new(MockClock::new()));

        // Execute probe
        let result = monitor.probe(
            ProbeMode::Green,
            create_proxy_credentials(),
            None,
        ).await.unwrap();

        // API call should still succeed
        assert_eq!(result.status, NetworkStatus::Healthy);
        assert_eq!(result.metrics.last_http_status, 200);

        // Load and verify state - health check error should result in None
        let state = monitor.load_state().await.unwrap();
        assert_eq!(state.network.proxy_healthy, None);
        assert_eq!(state.network.get_proxy_health_level(), None);
    }
}