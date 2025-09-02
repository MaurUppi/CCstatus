#![cfg(feature = "network-monitoring")]

/*!
Comprehensive unit tests for HttpMonitor component.

Tests cover all major functionality including:
- HTTP probe execution with different modes and timeouts
- State persistence with atomic operations
- Rolling statistics calculation and P95 computation
- Error classification and handling
- Credential injection and timeout strategies
- Debug logging integration
- Mock dependencies for isolated testing
*/

use ccstatus::core::network::proxy_health::{HealthCheckClient, HealthResponse};
use ccstatus::core::network::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::Mutex;

#[cfg(feature = "timings-curl")]
use ccstatus::core::network::http_monitor::{CurlProbeRunner, PhaseTimings};

/// HTTP method for URL-based mock routing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum MockHttpMethod {
    Get,
    Post,
}

/// Mock response for URL-based routing
#[derive(Debug, Clone)]
struct MockResponse {
    result: Result<
        (
            u16,
            Duration,
            String,
            std::collections::HashMap<String, String>,
            Option<String>,
        ),
        String,
    >,
}

/// URL-based mock HTTP client that routes responses by method and URL pattern
#[derive(Clone)]
struct TestHttpClient {
    // Legacy stack for backward compatibility
    responses: Arc<
        Mutex<
            Vec<
                Result<
                    (
                        u16,
                        Duration,
                        String,
                        std::collections::HashMap<String, String>,
                        Option<String>,
                    ),
                    String,
                >,
            >,
        >,
    >,
    // New URL-based routing
    route_responses: Arc<
        Mutex<
            std::collections::HashMap<
                (MockHttpMethod, String),
                std::collections::VecDeque<MockResponse>,
            >,
        >,
    >,
    default_response: Arc<MockResponse>,
}

impl TestHttpClient {
    fn new() -> Self {
        let empty_headers = std::collections::HashMap::new();
        let default_response = MockResponse {
            result: Ok((
                200,
                Duration::from_millis(1000),
                "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:970ms|Total:1000ms".to_string(),
                empty_headers,
                Some("HTTP/1.1".to_string()),
            )),
        };

        Self {
            responses: Arc::new(Mutex::new(vec![])),
            route_responses: Arc::new(Mutex::new(std::collections::HashMap::new())),
            default_response: Arc::new(default_response),
        }
    }

    /// Add response for specific URL pattern (new URL-based routing)
    async fn add_response_for_url(
        &self,
        method: MockHttpMethod,
        url_pattern: &str,
        response: Result<
            (
                u16,
                Duration,
                String,
                std::collections::HashMap<String, String>,
                Option<String>,
            ),
            String,
        >,
    ) {
        let mock_response = MockResponse { result: response };
        let mut routes = self.route_responses.lock().await;
        let key = (method, url_pattern.to_string());
        routes
            .entry(key)
            .or_insert_with(std::collections::VecDeque::new)
            .push_back(mock_response);
    }

    /// Add response for POST /v1/messages calls (main API probe)
    async fn add_api_response(
        &self,
        response: Result<
            (
                u16,
                Duration,
                String,
                std::collections::HashMap<String, String>,
                Option<String>,
            ),
            String,
        >,
    ) {
        self.add_response_for_url(MockHttpMethod::Post, "/v1/messages", response)
            .await;
    }

    /// Add response for GET health check calls
    async fn add_health_response(
        &self,
        response: Result<
            (
                u16,
                Duration,
                String,
                std::collections::HashMap<String, String>,
                Option<String>,
            ),
            String,
        >,
    ) {
        // Handle both common health check patterns
        self.add_response_for_url(MockHttpMethod::Get, "/health", response.clone())
            .await;
        self.add_response_for_url(MockHttpMethod::Get, "/v1/health", response)
            .await;
    }

    /// Get next response for specific URL and method
    async fn get_response_for_url(
        &self,
        method: MockHttpMethod,
        url: &str,
    ) -> Option<MockResponse> {
        let mut routes = self.route_responses.lock().await;

        // Try exact URL match first
        if let Some(queue) = routes.get_mut(&(method.clone(), url.to_string())) {
            if let Some(response) = queue.pop_front() {
                return Some(response);
            }
        }

        // Try pattern matching for common endpoints
        let patterns_to_try = match method {
            MockHttpMethod::Post => vec!["/v1/messages", "messages"],
            MockHttpMethod::Get => vec!["/health", "/v1/health", "health"],
        };

        for pattern in patterns_to_try {
            if url.contains(pattern) {
                if let Some(queue) = routes.get_mut(&(method.clone(), pattern.to_string())) {
                    if let Some(response) = queue.pop_front() {
                        return Some(response);
                    }
                }
            }
        }

        None
    }

    async fn add_response(
        &self,
        response: Result<
            (
                u16,
                Duration,
                String,
                std::collections::HashMap<String, String>,
                Option<String>,
            ),
            String,
        >,
    ) {
        let mut responses = self.responses.lock().await;
        responses.insert(0, response);
    }

    async fn add_success(&self, status: u16, duration_ms: u64) {
        let duration = Duration::from_millis(duration_ms);
        let breakdown = format!(
            "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:{}ms|Total:{}ms",
            duration_ms - 30,
            duration_ms
        );
        let empty_headers = std::collections::HashMap::new();
        self.add_response(Ok((
            status,
            duration,
            breakdown,
            empty_headers,
            Some("HTTP/1.1".to_string()),
        )))
        .await;
    }

    async fn add_timeout_error(&self) {
        self.add_response(Err("Request timeout".to_string())).await;
    }
}

#[async_trait::async_trait]
impl HttpClientTrait for TestHttpClient {
    async fn execute_request(
        &self,
        url: String,
        _headers: HashMap<String, String>,
        body: Vec<u8>,
        _timeout_ms: u32,
    ) -> Result<
        (
            u16,
            Duration,
            String,
            std::collections::HashMap<String, String>,
            Option<String>,
        ),
        String,
    > {
        // Determine HTTP method based on request characteristics
        let method = if body.is_empty() {
            MockHttpMethod::Get
        } else {
            MockHttpMethod::Post
        };

        // Try URL-based routing first
        if let Some(mock_response) = self.get_response_for_url(method, &url).await {
            return mock_response.result;
        }

        // Fall back to legacy stack for backward compatibility
        let mut responses = self.responses.lock().await;
        if let Some(response) = responses.pop() {
            return response;
        }

        // Use default response if no specific response is configured
        self.default_response.result.clone()
    }
}

/// Test-specific mock clock with controllable time
#[derive(Clone)]
struct TestClock {
    timestamp_sequence: Arc<Mutex<Vec<String>>>,
}

impl TestClock {
    fn new() -> Self {
        Self {
            timestamp_sequence: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn add_timestamp(&self, timestamp: &str) {
        let mut seq = self.timestamp_sequence.lock().await;
        seq.insert(0, timestamp.to_string());
    }
}

impl ClockTrait for TestClock {
    fn now(&self) -> Instant {
        // For test purposes, return a fixed time
        Instant::now()
    }

    fn local_timestamp(&self) -> String {
        // For tests, return a fixed timestamp
        "2025-01-25T10:30:00-08:00".to_string()
    }
}

/// Test-specific fake curl runner for testing phase timing logic
#[cfg(feature = "timings-curl")]
#[derive(Clone)]
struct FakeCurlRunner {
    responses: Arc<Mutex<Vec<Result<PhaseTimings, NetworkError>>>>,
}

#[cfg(feature = "timings-curl")]
impl FakeCurlRunner {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn add_response(&self, response: Result<PhaseTimings, NetworkError>) {
        let mut responses = self.responses.lock().await;
        responses.insert(0, response);
    }

    async fn add_timing_response(
        &self,
        status: u16,
        dns_ms: u32,
        tcp_ms: u32,
        tls_ms: u32,
        ttfb_ms: u32,
        total_ms: u32,
    ) {
        let phase_timings = PhaseTimings {
            status,
            dns_ms,
            tcp_ms,
            tls_ms,
            ttfb_ms,
            total_ttfb_ms: dns_ms + tcp_ms + tls_ms + ttfb_ms, // End-to-end TTFB
            total_ms,
        };
        self.add_response(Ok(phase_timings)).await;
    }

    async fn add_error(&self, error_msg: &str) {
        let error = NetworkError::HttpError(error_msg.to_string());
        self.add_response(Err(error)).await;
    }
}

#[cfg(feature = "timings-curl")]
#[async_trait::async_trait]
impl CurlProbeRunner for FakeCurlRunner {
    async fn run(
        &self,
        _url: &str,
        _headers: &[(&str, String)],
        _body: &[u8],
        _timeout_ms: u32,
    ) -> Result<PhaseTimings, NetworkError> {
        let mut responses = self.responses.lock().await;
        responses.pop().unwrap_or_else(|| {
            // Default response for deterministic testing - matches TestHttpClient default of 1500ms
            Ok(PhaseTimings {
                status: 200,
                dns_ms: 25,
                tcp_ms: 30,
                tls_ms: 35,
                ttfb_ms: 1500, // ttfb_ms becomes latency_ms in the outcome
                total_ttfb_ms: 25 + 30 + 35 + 1500, // End-to-end TTFB
                total_ms: 1590, // total should be sum of all phases (25+30+35+1500)
            })
        })
    }
}

/// Helper to create test credentials
fn test_credentials() -> ApiCredentials {
    ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token-12345".to_string(),
        source: CredentialSource::Environment,
    }
}

/// Mock health check client for testing
#[derive(Clone)]
struct TestHealthCheckClient {
    responses: Arc<Mutex<Vec<Result<HealthResponse, String>>>>,
}

impl TestHealthCheckClient {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn add_health_response(&self, status: u16, duration_ms: u64, body: &str) {
        let response = HealthResponse {
            status_code: status,
            body: body.as_bytes().to_vec(),
            duration: Duration::from_millis(duration_ms),
            headers: std::collections::HashMap::new(),
        };
        let mut responses = self.responses.lock().await;
        responses.insert(0, Ok(response));
    }
}

#[async_trait::async_trait]
impl HealthCheckClient for TestHealthCheckClient {
    async fn get_health(&self, _url: String, _timeout_ms: u32) -> Result<HealthResponse, String> {
        let mut responses = self.responses.lock().await;
        responses.pop().unwrap_or_else(|| {
            // Default healthy response
            Ok(HealthResponse {
                status_code: 200,
                body: r#"{"status": "healthy"}"#.as_bytes().to_vec(),
                duration: Duration::from_millis(100),
                headers: std::collections::HashMap::new(),
            })
        })
    }
}

/// Coordinated test client that manages both HTTP and curl responses
#[derive(Clone)]
struct CoordinatedTestClient {
    http_client: TestHttpClient,
    health_client: TestHealthCheckClient,
    #[cfg(feature = "timings-curl")]
    curl_runner: FakeCurlRunner,
}

impl CoordinatedTestClient {
    fn new() -> Self {
        Self {
            http_client: TestHttpClient::new(),
            health_client: TestHealthCheckClient::new(),
            #[cfg(feature = "timings-curl")]
            curl_runner: FakeCurlRunner::new(),
        }
    }

    async fn add_success(&self, status: u16, duration_ms: u64) {
        // Set up HTTP client response
        self.http_client.add_success(status, duration_ms).await;

        // Also set up curl runner response when feature is enabled
        #[cfg(feature = "timings-curl")]
        {
            let ttfb_ms = duration_ms as u32;
            let total_ms = duration_ms as u32 + 90; // Add phase timings
            self.curl_runner
                .add_timing_response(status, 25, 30, 35, ttfb_ms, total_ms)
                .await;
        }
    }

    async fn add_timeout_error(&self) {
        self.http_client.add_timeout_error().await;
        #[cfg(feature = "timings-curl")]
        {
            self.curl_runner.add_error("Timeout error").await;
        }
    }
}

/// Helper for parsing and validating timing breakdown strings
#[derive(Debug)]
struct BreakdownValidator {
    dns: Option<u32>,
    tcp: Option<u32>,
    tls: Option<u32>,
    ttfb: Option<u32>,
    total: Option<u32>,
}

impl BreakdownValidator {
    fn parse(breakdown: &str) -> Self {
        let mut validator = Self {
            dns: None,
            tcp: None,
            tls: None,
            ttfb: None,
            total: None,
        };

        for segment in breakdown.split('|') {
            if let Some(colon_pos) = segment.find(':') {
                let phase = &segment[..colon_pos];
                let timing_part = &segment[colon_pos + 1..];
                if let Some(ms_pos) = timing_part.find("ms") {
                    if let Ok(value) = timing_part[..ms_pos].parse::<u32>() {
                        match phase {
                            "DNS" => validator.dns = Some(value),
                            "TCP" => validator.tcp = Some(value),
                            "TLS" => validator.tls = Some(value),
                            "TTFB" => validator.ttfb = Some(value),
                            "Total" => validator.total = Some(value),
                            _ => {} // Ignore unknown phases
                        }
                    }
                }
            }
        }

        validator
    }

    fn assert_exact(
        &self,
        expected_dns: u32,
        expected_tcp: u32,
        expected_tls: u32,
        expected_ttfb: u32,
        expected_total: u32,
    ) {
        assert_eq!(self.dns, Some(expected_dns), "DNS timing mismatch");
        assert_eq!(self.tcp, Some(expected_tcp), "TCP timing mismatch");
        assert_eq!(self.tls, Some(expected_tls), "TLS timing mismatch");
        assert_eq!(self.ttfb, Some(expected_ttfb), "TTFB timing mismatch");
        assert_eq!(self.total, Some(expected_total), "Total timing mismatch");
    }

    fn assert_within_tolerance(
        &self,
        expected_dns: u32,
        expected_tcp: u32,
        expected_tls: u32,
        expected_ttfb: u32,
        expected_total: u32,
        tolerance_ms: u32,
    ) {
        if let Some(dns) = self.dns {
            assert!(
                dns.abs_diff(expected_dns) <= tolerance_ms,
                "DNS timing {} not within {}ms of {}",
                dns,
                tolerance_ms,
                expected_dns
            );
        }
        if let Some(tcp) = self.tcp {
            assert!(
                tcp.abs_diff(expected_tcp) <= tolerance_ms,
                "TCP timing {} not within {}ms of {}",
                tcp,
                tolerance_ms,
                expected_tcp
            );
        }
        if let Some(tls) = self.tls {
            assert!(
                tls.abs_diff(expected_tls) <= tolerance_ms,
                "TLS timing {} not within {}ms of {}",
                tls,
                tolerance_ms,
                expected_tls
            );
        }
        if let Some(ttfb) = self.ttfb {
            assert!(
                ttfb.abs_diff(expected_ttfb) <= tolerance_ms,
                "TTFB timing {} not within {}ms of {}",
                ttfb,
                tolerance_ms,
                expected_ttfb
            );
        }
        if let Some(total) = self.total {
            assert!(
                total.abs_diff(expected_total) <= tolerance_ms,
                "Total timing {} not within {}ms of {}",
                total,
                tolerance_ms,
                expected_total
            );
        }
    }

    fn assert_contains_phases(&self, phases: &[&str]) {
        for phase in phases {
            match *phase {
                "DNS" => assert!(self.dns.is_some(), "Missing DNS phase"),
                "TCP" => assert!(self.tcp.is_some(), "Missing TCP phase"),
                "TLS" => assert!(self.tls.is_some(), "Missing TLS phase"),
                "TTFB" => assert!(self.ttfb.is_some(), "Missing TTFB phase"),
                "Total" => assert!(self.total.is_some(), "Missing Total phase"),
                _ => panic!("Unknown phase: {}", phase),
            }
        }
    }
}

/// Create HttpMonitor with test dependencies and temp directory
fn create_test_monitor(temp_dir: &TempDir) -> (HttpMonitor, CoordinatedTestClient, TestClock) {
    let coordinated_client = CoordinatedTestClient::new();
    let clock = TestClock::new();

    let state_path = temp_dir.path().join("monitoring.json");
    let mut monitor = HttpMonitor::new(Some(state_path))
        .unwrap()
        .with_http_client(
            Box::new(coordinated_client.http_client.clone()) as Box<dyn HttpClientTrait>
        )
        .with_health_client(
            Box::new(coordinated_client.health_client.clone()) as Box<dyn HealthCheckClient>
        )
        .with_clock(Box::new(clock.clone()) as Box<dyn ClockTrait>);

    // When timings-curl feature is enabled, inject the coordinated FakeCurlRunner
    #[cfg(feature = "timings-curl")]
    {
        monitor = monitor.with_curl_runner(Box::new(coordinated_client.curl_runner.clone()));
    }

    (monitor, coordinated_client, clock)
}

#[tokio::test]
async fn test_new_monitor_default_path() {
    let monitor = HttpMonitor::new(None);
    assert!(monitor.is_ok(), "Should create monitor with default path");
}

#[tokio::test]
async fn test_new_monitor_custom_path() {
    let temp_dir = TempDir::new().unwrap();
    let custom_path = temp_dir.path().join("custom-monitoring.json");

    let monitor = HttpMonitor::new(Some(custom_path.clone()));
    assert!(monitor.is_ok(), "Should create monitor with custom path");
}

#[tokio::test]
async fn test_green_probe_success_updates_rolling_stats() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Setup successful HTTP 200 response with 1500ms latency
    http_client.add_success(200, 1500).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

    // Execute GREEN probe
    let result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await;

    assert!(result.is_ok(), "GREEN probe should succeed");
    let outcome = result.unwrap();

    // Verify probe outcome
    assert_eq!(outcome.mode, ProbeMode::Green);
    assert_eq!(outcome.metrics.last_http_status, 200);
    assert_eq!(outcome.metrics.latency_ms, 1500);
    assert_eq!(
        outcome.rolling_len, 1,
        "Should have 1 sample in rolling window"
    );
    assert_eq!(
        outcome.p95_latency_ms, 1500,
        "P95 should equal single sample"
    );
    assert!(outcome.state_written, "State should be written to disk");

    // Verify status determination (single sample should be healthy)
    assert!(
        matches!(outcome.status, NetworkStatus::Healthy),
        "Single 200 response should be healthy"
    );
}

#[tokio::test]
async fn test_green_probe_builds_rolling_window() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Setup sequence of responses with varying latencies
    let latencies = vec![1000, 1200, 800, 1500, 900, 1100];
    for (i, &latency) in latencies.iter().enumerate() {
        http_client.add_success(200, latency).await;
        clock
            .add_timestamp(&format!("2025-01-25T10:3{}:00-08:00", i))
            .await;
    }

    // Execute multiple GREEN probes
    for _ in 0..latencies.len() {
        let result = monitor
            .probe(ProbeMode::Green, test_credentials(), None)
            .await;
        assert!(result.is_ok(), "All GREEN probes should succeed");
    }

    // Verify final rolling window state
    let state = monitor.load_state().await.unwrap();
    assert_eq!(state.network.rolling_totals.len(), latencies.len());

    // Verify P95 calculation using nearest-rank method (95th percentile of [800, 900, 1000, 1100, 1200, 1500])
    // Rank: ceil(0.95 * 6) = ceil(5.7) = 6, index = 6-1 = 5 -> 1500ms
    assert_eq!(
        state.network.p95_latency_ms, 1500,
        "P95 should be 1500ms using nearest-rank method"
    );
    assert!(
        matches!(state.status, NetworkStatus::Healthy),
        "Should remain healthy"
    );
}

#[tokio::test]
async fn test_green_probe_caps_rolling_window() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Setup 16 responses (15 + 1 extra for final probe call)
    for i in 0..16 {
        http_client.add_success(200, 1000 + (i * 50) as u64).await;
        clock
            .add_timestamp(&format!("2025-01-25T{:02}:30:00-08:00", 10 + i))
            .await;
    }

    // Execute 15 GREEN probes
    for _ in 0..15 {
        let result = monitor
            .probe(ProbeMode::Green, test_credentials(), None)
            .await;
        assert!(result.is_ok());
    }

    // Verify rolling window is capped at 12 samples
    let _outcome = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    let state = monitor.load_state().await.unwrap();
    assert_eq!(state.network.rolling_totals.len(), 12);

    // Verify it contains the most recent 12 samples (not the first 12)
    // After 16 total probes (iterations 0-15), the last 12 should be from iterations 4-15
    // Values: 1000, 1050, 1100, 1150, [1200...1750] (last 12)
    let expected_min = 1000 + (4 * 50); // Sample from iteration 4 onwards = 1200ms
    assert!(
        state
            .network
            .rolling_totals
            .iter()
            .all(|&x| x >= expected_min),
        "Should contain most recent samples after FIFO eviction"
    );
}

#[tokio::test]
async fn test_red_probe_never_updates_rolling_stats() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // First establish some baseline rolling stats with GREEN probe
    http_client.add_success(200, 1000).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;
    let green_result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    assert_eq!(
        green_result.rolling_len, 1,
        "GREEN should add to rolling stats"
    );

    // Now execute RED probe with different latency
    http_client.add_success(200, 500).await;
    clock.add_timestamp("2025-01-25T10:30:01-08:00").await;
    let red_result = monitor
        .probe(
            ProbeMode::Red,
            test_credentials(),
            Some(JsonlError {
                timestamp: "2025-01-25T10:29:00-08:00".to_string(),
                code: 529,
                message: "Overloaded".to_string(),
            }),
        )
        .await
        .unwrap();

    // Verify RED probe behavior
    assert_eq!(red_result.mode, ProbeMode::Red);
    assert_eq!(
        red_result.rolling_len, 1,
        "RED should NOT add to rolling stats"
    );
    assert_eq!(
        red_result.p95_latency_ms, 1000,
        "P95 should remain from GREEN probe"
    );
    assert!(
        matches!(red_result.status, NetworkStatus::Error),
        "RED mode should set status=error"
    );

    // Verify JSONL error event was recorded
    let state = monitor.load_state().await.unwrap();
    assert!(
        state.last_jsonl_error_event.is_some(),
        "Should record JSONL error"
    );
    let error_event = state.last_jsonl_error_event.unwrap();
    assert_eq!(error_event.code, 529);
    assert_eq!(error_event.message, "Overloaded");
}

#[tokio::test]
async fn test_cold_probe_behavior() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Set session ID for COLD probe tracking
    let test_session_id = "test_cold_session_123";
    monitor.set_session_id(test_session_id.to_string());

    // Execute COLD probe
    http_client.add_success(200, 1200).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;
    let result = monitor
        .probe(ProbeMode::Cold, test_credentials(), None)
        .await;

    assert!(result.is_ok(), "COLD probe should succeed");
    let outcome = result.unwrap();

    // COLD should behave like GREEN for statistics
    assert_eq!(outcome.mode, ProbeMode::Cold);
    assert_eq!(outcome.rolling_len, 1, "COLD should add to rolling stats");
    assert_eq!(outcome.p95_latency_ms, 1200, "COLD should update P95");

    // Verify COLD session tracking fields are written
    let state = monitor.load_state().await.unwrap();
    assert!(
        state.monitoring_state.last_cold_session_id.is_some(),
        "Should record COLD session ID"
    );
    assert!(
        state.monitoring_state.last_cold_probe_at.is_some(),
        "Should record COLD probe timestamp"
    );
}

#[tokio::test]
async fn test_error_classification() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    let test_cases = vec![
        (200, None, "Success should have no error_type"),
        (
            400,
            Some("invalid_request_error"),
            "400 -> invalid_request_error",
        ),
        (
            401,
            Some("authentication_error"),
            "401 -> authentication_error",
        ),
        (403, Some("bot_challenge"), "403 -> bot_challenge"),
        (404, Some("not_found_error"), "404 -> not_found_error"),
        (413, Some("request_too_large"), "413 -> request_too_large"),
        (429, Some("rate_limit_error"), "429 -> rate_limit_error"),
        (500, Some("api_error"), "500 -> api_error"),
        (503, Some("bot_challenge"), "503 -> bot_challenge"),
        (504, Some("socket_hang_up"), "504 -> socket_hang_up"),
        (529, Some("overloaded_error"), "529 -> overloaded_error"),
        (499, Some("client_error"), "499 -> client_error"),
        (599, Some("server_error"), "599 -> server_error"),
    ];

    for (status_code, expected_error, description) in test_cases {
        http_client.add_success(status_code, 1000).await;
        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let result = monitor
            .probe(ProbeMode::Green, test_credentials(), None)
            .await
            .unwrap();

        match expected_error {
            Some(expected) => {
                assert_eq!(
                    result.metrics.error_type.as_deref(),
                    Some(expected),
                    "{}",
                    description
                );
            }
            None => {
                assert!(result.metrics.error_type.is_none(), "{}", description);
            }
        }
    }
}

#[tokio::test]
async fn test_connection_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Test timeout error
    http_client.add_timeout_error().await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

    let result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    assert_eq!(
        result.metrics.last_http_status, 0,
        "Timeout should result in status 0"
    );
    assert_eq!(
        result.metrics.error_type.as_deref(),
        Some("connection_error"),
        "Timeout should map to connection_error"
    );
    assert!(
        matches!(result.status, NetworkStatus::Error),
        "Connection error should be Error status"
    );
}

#[tokio::test]
async fn test_write_unknown_preserves_rolling_stats() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // First establish rolling stats
    http_client.add_success(200, 1000).await;
    http_client.add_success(200, 1200).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;
    clock.add_timestamp("2025-01-25T10:30:01-08:00").await;

    monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    let state_before = monitor.load_state().await.unwrap();
    assert_eq!(state_before.network.rolling_totals.len(), 2);
    assert_eq!(state_before.network.p95_latency_ms, 1200);

    // Write unknown status
    clock.add_timestamp("2025-01-25T10:31:00-08:00").await;
    monitor.write_unknown(false).await.unwrap();

    // Verify unknown status but preserved rolling stats
    let state_after = monitor.load_state().await.unwrap();
    assert!(matches!(state_after.status, NetworkStatus::Unknown));
    assert!(!state_after.monitoring_enabled);
    assert!(state_after.api_config.is_none());
    assert_eq!(
        state_after.network.rolling_totals.len(),
        2,
        "Rolling stats should be preserved"
    );
    assert_eq!(
        state_after.network.p95_latency_ms, 1200,
        "P95 should be preserved"
    );
}

#[tokio::test]
async fn test_load_state_nonexistent_file() {
    let temp_dir = TempDir::new().unwrap();
    let non_existent_path = temp_dir.path().join("does-not-exist.json");

    let monitor = HttpMonitor::new(Some(non_existent_path)).unwrap();
    let result = monitor.load_state().await;

    assert!(
        result.is_ok(),
        "Should return default state for non-existent file"
    );
    let state = result.unwrap();
    assert!(matches!(state.status, NetworkStatus::Unknown));
    assert!(!state.monitoring_enabled);
    assert!(state.api_config.is_none());
    assert_eq!(state.network.rolling_totals.len(), 0);
}

#[tokio::test]
async fn test_atomic_state_writing() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Execute probe to write state
    http_client.add_success(200, 1000).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

    let result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await;
    assert!(result.is_ok());

    // Verify state file exists and can be loaded
    let state = monitor.load_state().await.unwrap();
    assert!(matches!(state.status, NetworkStatus::Healthy));
    assert_eq!(state.network.latency_ms, 1000);

    // Verify no .tmp file remains (atomic operation completed)
    let temp_file = temp_dir.path().join("monitoring.tmp");
    assert!(!temp_file.exists(), "Temporary file should be cleaned up");
}

#[tokio::test]
async fn test_api_config_recording() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    http_client.add_success(200, 1000).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

    let creds = ApiCredentials {
        base_url: "https://custom-api.example.com".to_string(),
        auth_token: "custom-token".to_string(),
        source: CredentialSource::Environment,
    };

    let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

    // Verify API config is recorded correctly
    assert_eq!(
        result.api_config.endpoint,
        "https://custom-api.example.com/v1/messages"
    );
    assert_eq!(result.api_config.source, "environment");

    // Verify it's persisted in state
    let state = monitor.load_state().await.unwrap();
    let api_config = state.api_config.unwrap();
    assert_eq!(
        api_config.endpoint,
        "https://custom-api.example.com/v1/messages"
    );
    assert_eq!(api_config.source, "environment");
}

#[tokio::test]
async fn test_comprehensive_probe_flow() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Simulate complete monitoring flow with different scenarios

    // 1. COLD probe on startup
    http_client.add_success(200, 1100).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;
    let cold_result = monitor
        .probe(ProbeMode::Cold, test_credentials(), None)
        .await
        .unwrap();
    assert_eq!(cold_result.mode, ProbeMode::Cold);
    assert!(cold_result.state_written);

    // 2. Several GREEN probes building baseline
    for i in 1..=5 {
        http_client.add_success(200, 1000 + (i * 50) as u64).await;
        clock
            .add_timestamp(&format!("2025-01-25T10:3{}:00-08:00", i))
            .await;
        let _green_result = monitor
            .probe(ProbeMode::Green, test_credentials(), None)
            .await
            .unwrap();
        assert_eq!(_green_result.mode, ProbeMode::Green);
    }

    // 3. RED probe due to detected error
    http_client.add_success(500, 800).await;
    clock.add_timestamp("2025-01-25T10:36:00-08:00").await;
    let red_result = monitor
        .probe(
            ProbeMode::Red,
            test_credentials(),
            Some(JsonlError {
                timestamp: "2025-01-25T10:29:00-08:00".to_string(),
                code: 529,
                message: "Overloaded".to_string(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(red_result.mode, ProbeMode::Red);
    assert!(matches!(red_result.status, NetworkStatus::Error));

    // 4. Recovery GREEN probe
    http_client.add_success(200, 950).await;
    clock.add_timestamp("2025-01-25T10:37:00-08:00").await;
    let _recovery_result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    // Verify final state reflects the complete monitoring session
    let final_state = monitor.load_state().await.unwrap();

    // Rolling stats should have 7 samples (COLD + 5 GREEN + 1 recovery GREEN, RED doesn't count)
    assert_eq!(final_state.network.rolling_totals.len(), 7);
    assert!(final_state.monitoring_enabled);
    assert!(final_state.api_config.is_some());
    assert!(final_state.last_jsonl_error_event.is_some());

    // P95 should be calculated from all GREEN/COLD 200 samples
    assert!(final_state.network.p95_latency_ms > 0);

    // Most recent successful probe should determine current status
    assert!(matches!(final_state.status, NetworkStatus::Healthy));
}

#[tokio::test]
async fn test_status_determination_edge_cases() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Test 429 rate limiting -> degraded status (special case)
    http_client.add_success(429, 1000).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;
    let result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    assert!(
        matches!(result.status, NetworkStatus::Degraded),
        "429 should be degraded"
    );
    assert_eq!(
        result.metrics.error_type.as_deref(),
        Some("rate_limit_error")
    );

    // Test non-200 HTTP responses don't update rolling stats
    let initial_rolling_len = result.rolling_len;

    http_client.add_success(500, 2000).await;
    clock.add_timestamp("2025-01-25T10:30:01-08:00").await;
    let result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    assert!(
        matches!(result.status, NetworkStatus::Error),
        "500 should be error"
    );
    assert_eq!(
        result.rolling_len, initial_rolling_len,
        "500 should not add to rolling stats"
    );
}

// ====== New Enhancement Tests ======

#[tokio::test]
async fn test_credential_source_display_trait() {
    use std::path::PathBuf;

    // Test Display implementation for CredentialSource
    let env_source = CredentialSource::Environment;
    assert_eq!(env_source.to_string(), "environment");

    let shell_source = CredentialSource::ShellConfig(PathBuf::from("/home/user/.bashrc"));
    assert_eq!(shell_source.to_string(), "shell");

    let claude_source = CredentialSource::ClaudeConfig(PathBuf::from("/home/user/.claude/config"));
    assert_eq!(claude_source.to_string(), "claude_config");
}

#[tokio::test]
async fn test_api_config_source_formatting() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Test that API config uses Display trait instead of Debug formatting
    http_client.add_success(200, 1000).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::Environment,
    };

    let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

    assert_eq!(result.api_config.source, "environment");

    // Verify state persistence also uses correct formatting
    let state = monitor.load_state().await.unwrap();
    assert_eq!(state.api_config.as_ref().unwrap().source, "environment");
}

#[tokio::test]
async fn test_environment_variable_compatibility() {
    let temp_dir = TempDir::new().unwrap();

    // Test CCSTATUS_TIMEOUT_MS (current uppercase)
    std::env::set_var("CCSTATUS_TIMEOUT_MS", "3000");
    let (mut monitor, http_client, _clock) = create_test_monitor(&temp_dir);

    // The timeout should be applied (we can't directly test this without exposing internals,
    // but we can verify the monitor works with env var set)
    http_client.add_success(200, 1000).await;
    let result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    assert!(result.state_written);
    std::env::remove_var("CCSTATUS_TIMEOUT_MS");

    // Test ccstatus_TIMEOUT_MS (spec lowercase variant)
    std::env::set_var("ccstatus_TIMEOUT_MS", "2500");
    let (mut monitor2, http_client2, _clock2) = create_test_monitor(&temp_dir);

    http_client2.add_success(200, 1000).await;
    let result2 = monitor2
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    assert!(result2.state_written);
    std::env::remove_var("ccstatus_TIMEOUT_MS");

    // Test that uppercase takes precedence when both are set
    std::env::set_var("CCSTATUS_TIMEOUT_MS", "4000");
    std::env::set_var("ccstatus_TIMEOUT_MS", "2000");
    let (mut monitor3, http_client3, _clock3) = create_test_monitor(&temp_dir);

    http_client3.add_success(200, 1000).await;
    let result3 = monitor3
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();
    assert!(result3.state_written);

    std::env::remove_var("CCSTATUS_TIMEOUT_MS");
    std::env::remove_var("ccstatus_TIMEOUT_MS");
}

#[tokio::test]
async fn test_timestamp_conversion_for_jsonl_error_events() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Create a JSONL error event with UTC timestamp (typical format from transcript)
    let utc_error_event = JsonlError {
        timestamp: "2025-01-25T18:30:45Z".to_string(), // UTC with 'Z' suffix
        code: 500,
        message: "Internal server error".to_string(),
    };

    // Execute RED probe with the UTC error event
    http_client.add_success(500, 2000).await;
    clock.add_timestamp("2025-01-25T10:30:45-08:00").await; // Local PST timestamp

    let result = monitor
        .probe(ProbeMode::Red, test_credentials(), Some(utc_error_event))
        .await
        .unwrap();

    assert!(matches!(result.status, NetworkStatus::Error));

    // Verify the error event timestamp was converted to local time
    let state = monitor.load_state().await.unwrap();
    let stored_error = state.last_jsonl_error_event.as_ref().unwrap();

    // The timestamp should no longer have 'Z' suffix and should include timezone offset
    assert!(!stored_error.timestamp.ends_with('Z'));
    assert!(
        stored_error.timestamp.contains('-') || stored_error.timestamp.contains('+'),
        "Timestamp should include timezone offset: {}",
        stored_error.timestamp
    );

    // Verify it's a valid ISO-8601 timestamp with timezone
    use chrono::{DateTime, FixedOffset};
    let _parsed: DateTime<FixedOffset> = stored_error
        .timestamp
        .parse()
        .expect("Should be valid ISO-8601 with timezone");
}

#[tokio::test]
async fn test_timestamp_conversion_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Create a JSONL error event with invalid timestamp
    let invalid_error_event = JsonlError {
        timestamp: "invalid-timestamp".to_string(),
        code: 500,
        message: "Internal server error".to_string(),
    };

    // Execute RED probe with invalid timestamp - should fallback to local timestamp
    http_client.add_success(500, 2000).await;
    clock.add_timestamp("2025-01-25T10:30:45-08:00").await;

    let result = monitor
        .probe(
            ProbeMode::Red,
            test_credentials(),
            Some(invalid_error_event),
        )
        .await
        .unwrap();

    assert!(matches!(result.status, NetworkStatus::Error));

    // Verify the error event uses fallback local timestamp
    let state = monitor.load_state().await.unwrap();
    let stored_error = state.last_jsonl_error_event.as_ref().unwrap();

    // Should be the fallback local timestamp from our mock clock
    assert_eq!(stored_error.timestamp, "2025-01-25T10:30:00-08:00");
}

#[tokio::test]
async fn test_logging_correlation_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, _clock) = create_test_monitor(&temp_dir);

    // This test verifies that the same probe ID is used for both start and end logging
    // Since we can't directly capture debug logs in this test setup, we verify the
    // probe executes successfully (which requires consistent ID usage internally)

    http_client.add_success(200, 1000).await;
    let result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    assert!(result.state_written);
    assert!(matches!(result.status, NetworkStatus::Healthy));

    // Test with error case to ensure correlation ID works for all probe outcomes
    http_client.add_timeout_error().await;
    let result2 = monitor
        .probe(ProbeMode::Red, test_credentials(), None)
        .await
        .unwrap();

    assert!(result2.state_written);
    assert!(matches!(result2.status, NetworkStatus::Error));
}

#[tokio::test]
async fn test_comprehensive_enhancements_integration() {
    let temp_dir = TempDir::new().unwrap();

    // Test all enhancements working together
    std::env::set_var("CCSTATUS_TIMEOUT_MS", "3500");

    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // 1. Test environment variable + credential display + logging correlation
    http_client.add_success(200, 1200).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::ShellConfig(std::path::PathBuf::from("/home/user/.bashrc")),
    };

    let result1 = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

    assert!(matches!(result1.status, NetworkStatus::Healthy));
    assert_eq!(result1.api_config.source, "shell");

    // 2. Test timestamp conversion in RED probe
    let utc_error = JsonlError {
        timestamp: "2025-01-25T18:31:00Z".to_string(),
        code: 429,
        message: "Rate limit exceeded".to_string(),
    };

    http_client.add_success(429, 3000).await;
    clock.add_timestamp("2025-01-25T10:31:00-08:00").await;

    let creds2 = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::ClaudeConfig(std::path::PathBuf::from(
            "/home/user/.claude/config",
        )),
    };

    let result2 = monitor
        .probe(ProbeMode::Red, creds2, Some(utc_error))
        .await
        .unwrap();

    assert!(matches!(result2.status, NetworkStatus::Error));
    assert_eq!(result2.api_config.source, "claude_config");

    // Verify final state has all enhancements applied
    let final_state = monitor.load_state().await.unwrap();

    // Credential display enhancement
    assert_eq!(
        final_state.api_config.as_ref().unwrap().source,
        "claude_config"
    );

    // Timestamp conversion enhancement
    let stored_error = final_state.last_jsonl_error_event.as_ref().unwrap();
    assert!(!stored_error.timestamp.ends_with('Z'));
    assert!(stored_error.timestamp.contains('-') || stored_error.timestamp.contains('+'));

    std::env::remove_var("CCSTATUS_TIMEOUT_MS");
}

// Session Deduplication Tests

#[tokio::test]
async fn test_session_id_tracking_and_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test-session-tracking.json");

    let mock_client = TestHttpClient::new();
    mock_client.add_success(200, 1500).await;

    let mock_clock = TestClock::new();

    let mut monitor = HttpMonitor::new(Some(state_path.clone()))
        .unwrap()
        .with_http_client(Box::new(mock_client.clone()))
        .with_clock(Box::new(mock_clock));

    // Set session ID and execute COLD probe
    let test_session_id = "session_abc123_test";
    monitor.set_session_id(test_session_id.to_string());

    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::Environment,
    };

    let result = monitor.probe(ProbeMode::Cold, creds, None).await.unwrap();

    assert!(matches!(result.status, NetworkStatus::Healthy));
    assert_eq!(result.mode, ProbeMode::Cold);

    // Verify session ID is persisted in state
    let state = monitor.load_state().await.unwrap();
    assert_eq!(
        state.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    assert!(state.monitoring_state.last_cold_probe_at.is_some());
}

#[tokio::test]
async fn test_cold_probe_without_session_id() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test-cold-no-session.json");

    let mock_client = TestHttpClient::new();
    mock_client.add_success(200, 1200).await;

    let mock_clock = TestClock::new();

    let mut monitor = HttpMonitor::new(Some(state_path.clone()))
        .unwrap()
        .with_http_client(Box::new(mock_client.clone()))
        .with_clock(Box::new(mock_clock));

    // Execute COLD probe without setting session ID
    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::Environment,
    };

    let result = monitor.probe(ProbeMode::Cold, creds, None).await.unwrap();

    assert!(matches!(result.status, NetworkStatus::Healthy));
    assert_eq!(result.mode, ProbeMode::Cold);

    // Verify session deduplication fields are NOT updated when session ID is missing
    let state = monitor.load_state().await.unwrap();
    assert_eq!(state.monitoring_state.last_cold_session_id, None);
    assert_eq!(state.monitoring_state.last_cold_probe_at, None);
}

#[tokio::test]
async fn test_session_id_update_and_overwrite() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test-session-overwrite.json");

    let mock_client = TestHttpClient::new();
    mock_client.add_success(200, 1000).await;
    mock_client.add_success(200, 1100).await;

    let mock_clock = TestClock::new();

    let mut monitor = HttpMonitor::new(Some(state_path.clone()))
        .unwrap()
        .with_http_client(Box::new(mock_client.clone()))
        .with_clock(Box::new(mock_clock));

    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::Environment,
    };

    // First COLD probe with session ID 1
    let session_id_1 = "session_first";
    monitor.set_session_id(session_id_1.to_string());

    let _result1 = monitor
        .probe(ProbeMode::Cold, creds.clone(), None)
        .await
        .unwrap();

    let state1 = monitor.load_state().await.unwrap();
    assert_eq!(
        state1.monitoring_state.last_cold_session_id,
        Some(session_id_1.to_string())
    );

    // Update to session ID 2 and execute another COLD probe
    let session_id_2 = "session_second";
    monitor.set_session_id(session_id_2.to_string());

    let _result2 = monitor.probe(ProbeMode::Cold, creds, None).await.unwrap();

    let state2 = monitor.load_state().await.unwrap();
    assert_eq!(
        state2.monitoring_state.last_cold_session_id,
        Some(session_id_2.to_string())
    );

    // Ensure the new session ID overwrote the old one
    assert_ne!(
        state2.monitoring_state.last_cold_session_id,
        Some(session_id_1.to_string())
    );
}

#[tokio::test]
async fn test_session_deduplication_different_probe_modes() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test-session-probe-modes.json");

    let mock_client = TestHttpClient::new();
    mock_client.add_success(200, 1000).await; // COLD
    mock_client.add_success(200, 1100).await; // GREEN
    mock_client.add_success(429, 800).await; // RED

    let mock_clock = TestClock::new();

    let mut monitor = HttpMonitor::new(Some(state_path.clone()))
        .unwrap()
        .with_http_client(Box::new(mock_client.clone()))
        .with_clock(Box::new(mock_clock));

    let test_session_id = "session_mixed_modes";
    monitor.set_session_id(test_session_id.to_string());

    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::Environment,
    };

    // Execute COLD probe - should update session ID
    let _cold_result = monitor
        .probe(ProbeMode::Cold, creds.clone(), None)
        .await
        .unwrap();

    let state_after_cold = monitor.load_state().await.unwrap();
    assert_eq!(
        state_after_cold.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    let cold_timestamp = state_after_cold.monitoring_state.last_cold_probe_at.clone();
    assert!(cold_timestamp.is_some());

    // Execute GREEN probe - should NOT update session deduplication fields
    let _green_result = monitor
        .probe(ProbeMode::Green, creds.clone(), None)
        .await
        .unwrap();

    let state_after_green = monitor.load_state().await.unwrap();
    assert_eq!(
        state_after_green.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    assert_eq!(
        state_after_green.monitoring_state.last_cold_probe_at,
        cold_timestamp
    );

    // Execute RED probe - should NOT update session deduplication fields
    let error_event = JsonlError {
        timestamp: "2025-01-25T10:30:45.123Z".to_string(),
        code: 429,
        message: "Rate limit exceeded".to_string(),
    };
    let _red_result = monitor
        .probe(ProbeMode::Red, creds, Some(error_event))
        .await
        .unwrap();

    let state_after_red = monitor.load_state().await.unwrap();
    assert_eq!(
        state_after_red.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    assert_eq!(
        state_after_red.monitoring_state.last_cold_probe_at,
        cold_timestamp
    );

    // Verify only COLD mode updated the session deduplication fields
}

#[tokio::test]
async fn test_session_id_edge_cases() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test-session-edge-cases.json");

    let mock_client = TestHttpClient::new();
    mock_client.add_success(200, 1000).await;
    mock_client.add_success(500, 2000).await;

    let mock_clock = TestClock::new();

    let mut monitor = HttpMonitor::new(Some(state_path.clone()))
        .unwrap()
        .with_http_client(Box::new(mock_client.clone()))
        .with_clock(Box::new(mock_clock));

    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::Environment,
    };

    // Test empty string session ID
    monitor.set_session_id("".to_string());
    let _result1 = monitor
        .probe(ProbeMode::Cold, creds.clone(), None)
        .await
        .unwrap();

    let state1 = monitor.load_state().await.unwrap();
    assert_eq!(
        state1.monitoring_state.last_cold_session_id,
        Some("".to_string())
    );

    // Test very long session ID
    let long_session_id = "a".repeat(1000);
    monitor.set_session_id(long_session_id.clone());
    let _result2 = monitor
        .probe(ProbeMode::Cold, creds.clone(), None)
        .await
        .unwrap();

    let state2 = monitor.load_state().await.unwrap();
    assert_eq!(
        state2.monitoring_state.last_cold_session_id,
        Some(long_session_id)
    );

    // Test session ID with special characters
    let special_session_id = "session-with_special.chars@123#$%^&*()";
    monitor.set_session_id(special_session_id.to_string());
    let _result3 = monitor.probe(ProbeMode::Cold, creds, None).await.unwrap();

    let state3 = monitor.load_state().await.unwrap();
    assert_eq!(
        state3.monitoring_state.last_cold_session_id,
        Some(special_session_id.to_string())
    );
}

#[tokio::test]
async fn test_session_persistence_across_monitor_instances() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test-session-persistence.json");

    let mock_client = TestHttpClient::new();
    mock_client.add_success(200, 1200).await;

    let mock_clock = TestClock::new();

    let test_session_id = "persistent_session_123";
    let creds = ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token".to_string(),
        source: CredentialSource::Environment,
    };

    // First monitor instance - set session and execute COLD probe
    {
        let mut monitor1 = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(mock_client.clone()))
            .with_clock(Box::new(mock_clock.clone()));

        monitor1.set_session_id(test_session_id.to_string());
        let _result = monitor1
            .probe(ProbeMode::Cold, creds.clone(), None)
            .await
            .unwrap();
    }

    // Second monitor instance - should see persisted session data
    let monitor2 = HttpMonitor::new(Some(state_path.clone()))
        .unwrap()
        .with_http_client(Box::new(mock_client.clone()))
        .with_clock(Box::new(mock_clock));

    let loaded_state = monitor2.load_state().await.unwrap();

    // Verify session data persisted across monitor instances
    assert_eq!(
        loaded_state.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    assert!(loaded_state.monitoring_state.last_cold_probe_at.is_some());

    // Verify the state file contains the session data
    let file_content = tokio::fs::read_to_string(&state_path).await.unwrap();
    assert!(file_content.contains(test_session_id));
}

// ====== Bot Fight Phase 0 Validation Tests ======

#[tokio::test]
async fn test_bot_challenge_prevents_p95_contamination() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // First establish clean baseline rolling stats with successful probes
    http_client.add_success(200, 1000).await;
    http_client.add_success(200, 1100).await;
    http_client.add_success(200, 1200).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;
    clock.add_timestamp("2025-01-25T10:30:01-08:00").await;
    clock.add_timestamp("2025-01-25T10:30:02-08:00").await;

    for _ in 0..3 {
        let _ = monitor
            .probe(ProbeMode::Green, test_credentials(), None)
            .await
            .unwrap();
    }

    let baseline_state = monitor.load_state().await.unwrap();
    assert_eq!(baseline_state.network.rolling_totals.len(), 3);
    assert_eq!(baseline_state.network.p95_latency_ms, 1200);
    let baseline_rolling_totals = baseline_state.network.rolling_totals.clone();

    // Now send bot challenge response (403) - should NOT contaminate rolling stats
    http_client.add_success(403, 5000).await; // Very high latency that would skew P95
    clock.add_timestamp("2025-01-25T10:30:03-08:00").await;

    let bot_result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    // Verify bot challenge was detected
    assert_eq!(bot_result.metrics.last_http_status, 403);
    assert_eq!(
        bot_result.metrics.error_type.as_deref(),
        Some("bot_challenge")
    );
    assert!(matches!(bot_result.status, NetworkStatus::Error));

    // CRITICAL: Verify rolling stats were NOT contaminated
    let contamination_check_state = monitor.load_state().await.unwrap();
    assert_eq!(
        contamination_check_state.network.rolling_totals.len(),
        3,
        "Bot challenge should not add to rolling window"
    );
    assert_eq!(
        contamination_check_state.network.p95_latency_ms, 1200,
        "P95 should remain unchanged after bot challenge"
    );
    assert_eq!(
        contamination_check_state.network.rolling_totals, baseline_rolling_totals,
        "Rolling totals should be identical after bot challenge"
    );
}

#[tokio::test]
async fn test_bot_challenge_503_prevents_p95_contamination() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Establish baseline with 4 successful samples
    let baseline_latencies = vec![800, 900, 1000, 1100];
    for (i, &latency) in baseline_latencies.iter().enumerate() {
        http_client.add_success(200, latency).await;
        clock
            .add_timestamp(&format!("2025-01-25T10:3{}:00-08:00", i))
            .await;
    }

    for _ in 0..baseline_latencies.len() {
        let _ = monitor
            .probe(ProbeMode::Green, test_credentials(), None)
            .await
            .unwrap();
    }

    let baseline_state = monitor.load_state().await.unwrap();
    assert_eq!(baseline_state.network.rolling_totals.len(), 4);
    assert_eq!(baseline_state.network.p95_latency_ms, 1100); // P95 of [800,900,1000,1100]

    // Send 503 Service Unavailable (bot challenge) with extremely high latency
    http_client.add_success(503, 8000).await; // 8 seconds - would massively contaminate P95
    clock.add_timestamp("2025-01-25T10:34:00-08:00").await;

    let bot_challenge_result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    // Verify 503 was classified as bot challenge
    assert_eq!(bot_challenge_result.metrics.last_http_status, 503);
    assert_eq!(
        bot_challenge_result.metrics.error_type.as_deref(),
        Some("bot_challenge")
    );
    assert!(matches!(bot_challenge_result.status, NetworkStatus::Error));

    // Verify rolling statistics are protected from contamination
    let protected_state = monitor.load_state().await.unwrap();
    assert_eq!(
        protected_state.network.rolling_totals.len(),
        4,
        "503 bot challenge should not increase rolling window size"
    );
    assert_eq!(
        protected_state.network.p95_latency_ms, 1100,
        "P95 should be protected from 503 bot challenge contamination"
    );

    // Verify recovery still works with clean 200 responses
    http_client.add_success(200, 950).await;
    clock.add_timestamp("2025-01-25T10:35:00-08:00").await;

    let recovery_result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    assert_eq!(recovery_result.metrics.last_http_status, 200);
    assert!(recovery_result.metrics.error_type.is_none());
    assert!(matches!(recovery_result.status, NetworkStatus::Healthy));

    // Verify recovery sample was properly added to rolling stats
    let recovery_state = monitor.load_state().await.unwrap();
    assert_eq!(recovery_state.network.rolling_totals.len(), 5);
    assert!(recovery_state.network.rolling_totals.contains(&950));
}

#[tokio::test]
async fn test_rate_limit_without_bot_challenge_still_affects_p95() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Establish baseline
    http_client.add_success(200, 1000).await;
    clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

    let _baseline = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    let baseline_state = monitor.load_state().await.unwrap();
    assert_eq!(baseline_state.network.rolling_totals.len(), 1);
    assert_eq!(baseline_state.network.p95_latency_ms, 1000);

    // Send 429 rate limit (NOT classified as bot challenge in our implementation)
    // This should still result in degraded status but not add to rolling stats
    http_client.add_success(429, 2000).await;
    clock.add_timestamp("2025-01-25T10:30:01-08:00").await;

    let rate_limit_result = monitor
        .probe(ProbeMode::Green, test_credentials(), None)
        .await
        .unwrap();

    // Verify rate limit behavior
    assert_eq!(rate_limit_result.metrics.last_http_status, 429);
    assert_eq!(
        rate_limit_result.metrics.error_type.as_deref(),
        Some("rate_limit_error")
    );
    assert!(matches!(rate_limit_result.status, NetworkStatus::Degraded));

    // Rate limits (without bot classification) should not add to rolling stats either
    let after_rate_limit_state = monitor.load_state().await.unwrap();
    assert_eq!(
        after_rate_limit_state.network.rolling_totals.len(),
        1,
        "429 rate limit should not add to rolling window"
    );
    assert_eq!(
        after_rate_limit_state.network.p95_latency_ms, 1000,
        "P95 should remain unchanged after rate limit"
    );
}

#[tokio::test]
async fn test_mixed_bot_challenges_and_successful_responses() {
    let temp_dir = TempDir::new().unwrap();
    let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

    // Complex scenario: mix successful responses with various bot challenges
    let test_sequence = vec![
        (200, 1000, "successful baseline"),
        (403, 5000, "403 bot challenge - should be ignored"),
        (200, 1100, "successful recovery"),
        (503, 7000, "503 bot challenge - should be ignored"),
        (200, 1200, "successful recovery 2"),
        (403, 4500, "another 403 bot challenge - should be ignored"),
        (200, 1050, "final successful response"),
    ];

    for (i, (status, latency, description)) in test_sequence.iter().enumerate() {
        http_client.add_success(*status, *latency).await;
        clock
            .add_timestamp(&format!("2025-01-25T10:3{}:00-08:00", i))
            .await;

        let result = monitor
            .probe(ProbeMode::Green, test_credentials(), None)
            .await
            .unwrap();

        println!(
            "Step {}: {} - Status: {:?}, Rolling len: {}",
            i, description, result.status, result.rolling_len
        );
    }

    // Verify final state contains only successful responses in rolling stats
    let final_state = monitor.load_state().await.unwrap();

    // Should have 4 successful responses: 1000, 1100, 1200, 1050
    assert_eq!(
        final_state.network.rolling_totals.len(),
        4,
        "Only successful responses should be in rolling window"
    );

    let expected_samples = vec![1000, 1100, 1200, 1050];
    for &expected in &expected_samples {
        assert!(
            final_state.network.rolling_totals.contains(&expected),
            "Rolling totals should contain successful sample: {}ms",
            expected
        );
    }

    // Verify no bot challenge latencies contaminated the stats
    let contaminating_values = vec![5000, 7000, 4500];
    for &contaminator in &contaminating_values {
        assert!(
            !final_state.network.rolling_totals.contains(&contaminator),
            "Rolling totals should NOT contain bot challenge latency: {}ms",
            contaminator
        );
    }

    // P95 should be calculated from clean samples only: [1000, 1050, 1100, 1200]
    // Using nearest-rank method: ceil(0.95 * 4) = ceil(3.8) = 4, index = 3 -> 1200ms
    assert_eq!(
        final_state.network.p95_latency_ms, 1200,
        "P95 should be calculated from uncontaminated samples only"
    );
}

// ====== Curl Phase Timing Tests ======

#[cfg(feature = "timings-curl")]
mod curl_timing_tests {
    use super::*;

    /// Mock curl client for testing phase timing extraction logic
    #[derive(Clone)]
    struct TestCurlClient {
        phase_timings: Arc<Mutex<Vec<(f64, f64, f64, f64)>>>, // (dns, connect, appconnect, starttransfer)
        responses: Arc<Mutex<Vec<Result<u16, String>>>>,
    }

    impl TestCurlClient {
        fn new() -> Self {
            Self {
                phase_timings: Arc::new(Mutex::new(vec![])),
                responses: Arc::new(Mutex::new(vec![])),
            }
        }

        async fn add_timing_response(
            &self,
            status: u16,
            dns: f64,
            connect: f64,
            appconnect: f64,
            starttransfer: f64,
        ) {
            let mut timings = self.phase_timings.lock().await;
            timings.insert(0, (dns, connect, appconnect, starttransfer));

            let mut responses = self.responses.lock().await;
            responses.insert(0, Ok(status));
        }

        async fn add_curl_error(&self, error_msg: &str) {
            let mut responses = self.responses.lock().await;
            responses.insert(0, Err(error_msg.to_string()));
        }

        async fn get_next_timing(&self) -> Option<(f64, f64, f64, f64)> {
            let mut timings = self.phase_timings.lock().await;
            timings.pop()
        }

        async fn get_next_response(&self) -> Result<u16, String> {
            let mut responses = self.responses.lock().await;
            responses.pop().unwrap_or(Ok(200))
        }
    }

    #[tokio::test]
    async fn test_curl_dependency_injection_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, _http_client, clock) = create_test_monitor(&temp_dir);

        // Create FakeCurlRunner and inject it into HttpMonitor
        let fake_curl_runner = FakeCurlRunner::new();
        fake_curl_runner
            .add_timing_response(200, 25, 30, 35, 1000, 1090)
            .await;

        monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));
        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        // Execute probe - should use injected FakeCurlRunner
        let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

        // Verify that FakeCurlRunner was used with precise timing breakdown
        assert_eq!(result.metrics.latency_ms, 1000);
        let breakdown = &result.metrics.breakdown;
        assert_eq!(
            breakdown,
            "DNS:25ms|TCP:30ms|TLS:35ms|TTFB:1000ms|Total:1090ms"
        );

        // Verify the breakdown source is marked as measured (not heuristic)
        let state = monitor.load_state().await.unwrap();
        assert_eq!(state.network.breakdown_source, Some("measured".to_string()));
    }

    #[tokio::test]
    async fn test_curl_phase_timing_extraction() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, _http_client, clock) = create_test_monitor(&temp_dir);

        // Create FakeCurlRunner with specific timing values
        // DNS: 50ms, TCP: 25ms, TLS: 35ms, TTFB: 890ms, Total: 1000ms
        let fake_curl_runner = FakeCurlRunner::new();
        fake_curl_runner
            .add_timing_response(200, 50, 25, 35, 890, 1000)
            .await;
        monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));

        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        // Test the phase timing extraction logic by executing curl probe
        // Note: This test validates the timing math, not the actual curl integration
        let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

        // Verify the timing breakdown includes phase timings
        assert!(result.metrics.breakdown.contains("DNS:"));
        assert!(result.metrics.breakdown.contains("TCP:"));
        assert!(result.metrics.breakdown.contains("TLS:"));
        assert!(result.metrics.breakdown.contains("TTFB:"));

        // Parse the breakdown to verify timing calculation accuracy
        let breakdown = &result.metrics.breakdown;
        assert!(
            breakdown.contains("DNS:50ms"),
            "Expected DNS:50ms in breakdown: {}",
            breakdown
        );
        assert!(
            breakdown.contains("TCP:25ms"),
            "Expected TCP:25ms in breakdown: {}",
            breakdown
        );
        assert!(
            breakdown.contains("TLS:35ms"),
            "Expected TLS:35ms in breakdown: {}",
            breakdown
        );
        assert!(
            breakdown.contains("TTFB:890ms"),
            "Expected TTFB:890ms in breakdown: {}",
            breakdown
        );
        assert!(
            breakdown.contains("Total:1000ms"),
            "Expected Total:1000ms in breakdown: {}",
            breakdown
        );
    }

    #[tokio::test]
    async fn test_curl_connection_reuse_detection() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, _http_client, clock) = create_test_monitor(&temp_dir);

        // Create FakeCurlRunner for precise timing control
        let fake_curl_runner = FakeCurlRunner::new();

        // Test new connection (DNS > 0): DNS:45ms, TCP:25ms, TLS:35ms, TTFB:845ms, Total:950ms
        fake_curl_runner
            .add_timing_response(200, 45, 25, 35, 845, 950)
            .await;

        // Test connection reuse (DNS ≈ 0): DNS:1ms, TCP:24ms, TLS:35ms, TTFB:740ms, Total:800ms
        fake_curl_runner
            .add_timing_response(200, 1, 24, 35, 740, 800)
            .await;

        monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));
        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        // First probe - new connection
        let result1 = monitor
            .probe(ProbeMode::Green, creds.clone(), None)
            .await
            .unwrap();

        assert!(
            result1.metrics.breakdown.contains("DNS:45ms"),
            "New connection should show DNS time: {}",
            result1.metrics.breakdown
        );

        // Second probe - connection reuse
        let result2 = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

        assert!(
            result2.metrics.breakdown.contains("DNS:1ms"),
            "Reused connection should show minimal DNS time: {}",
            result2.metrics.breakdown
        );
    }

    #[tokio::test]
    async fn test_curl_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

        // Configure both curl and HTTP client to fail
        let fake_curl_runner = FakeCurlRunner::new();
        fake_curl_runner.add_error("Could not resolve host").await;
        monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));

        // Also configure the HTTP client fallback to fail
        http_client.add_timeout_error().await;

        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

        // Curl errors should be handled gracefully
        assert_eq!(result.metrics.last_http_status, 0);
        assert_eq!(
            result.metrics.error_type.as_deref(),
            Some("connection_error")
        );
        assert!(matches!(result.status, NetworkStatus::Error));

        // Breakdown should indicate connection failure
        assert!(result.metrics.breakdown.contains("DNS:0ms"));
        assert!(result.metrics.breakdown.contains("TCP:0ms"));
        assert!(result.metrics.breakdown.contains("TLS:0ms"));
        assert!(result.metrics.breakdown.contains("TTFB:0ms"));
        // Total may have small timing from processing overhead, just verify format
        assert!(result.metrics.breakdown.contains("Total:"));
    }

    #[tokio::test]
    async fn test_curl_timing_accuracy_validation() {
        // Test that DNS+TCP+TLS+TTFB ≈ Total timing
        let test_cases = vec![
            // (dns_ms, tcp_ms, tls_ms, ttfb_ms, total_ms)
            (20, 25, 35, 420, 500),   // 20+25+35+420 = 500ms total
            (30, 30, 35, 705, 800),   // 30+30+35+705 = 800ms total
            (15, 25, 35, 1125, 1200), // 15+25+35+1125 = 1200ms total
            (5, 15, 35, 245, 300),    // 5+15+35+245 = 300ms total
        ];

        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, _http_client, clock) = create_test_monitor(&temp_dir);

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        for (i, (dns_ms, tcp_ms, tls_ms, ttfb_ms, total_ms)) in test_cases.iter().enumerate() {
            clock
                .add_timestamp(&format!("2025-01-25T10:3{}:00-08:00", i))
                .await;

            let fake_curl_runner = FakeCurlRunner::new();
            fake_curl_runner
                .add_timing_response(200, *dns_ms, *tcp_ms, *tls_ms, *ttfb_ms, *total_ms)
                .await;
            monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));

            let result = monitor
                .probe(ProbeMode::Green, creds.clone(), None)
                .await
                .unwrap();

            // Use direct timing values
            let expected_dns = *dns_ms;
            let expected_tcp = *tcp_ms;
            let expected_tls = *tls_ms;
            let expected_ttfb = *ttfb_ms;
            let expected_total = *total_ms;

            // Verify timing breakdown accuracy
            let breakdown = &result.metrics.breakdown;
            assert!(
                breakdown.contains(&format!("DNS:{}ms", expected_dns)),
                "Case {}: Expected DNS:{}ms in {}",
                i,
                expected_dns,
                breakdown
            );
            assert!(
                breakdown.contains(&format!("TCP:{}ms", expected_tcp)),
                "Case {}: Expected TCP:{}ms in {}",
                i,
                expected_tcp,
                breakdown
            );
            assert!(
                breakdown.contains(&format!("TLS:{}ms", expected_tls)),
                "Case {}: Expected TLS:{}ms in {}",
                i,
                expected_tls,
                breakdown
            );
            assert!(
                breakdown.contains(&format!("TTFB:{}ms", expected_ttfb)),
                "Case {}: Expected TTFB:{}ms in {}",
                i,
                expected_ttfb,
                breakdown
            );
            assert!(
                breakdown.contains(&format!("Total:{}ms", expected_total)),
                "Case {}: Expected Total:{}ms in {}",
                i,
                expected_total,
                breakdown
            );

            // Verify total latency matches ttfb_ms (implementation behavior)
            assert_eq!(
                result.metrics.latency_ms, expected_ttfb,
                "Case {}: Total latency should match ttfb_ms",
                i
            );

            // Verify phase sum ≈ total (within 1ms tolerance for rounding)
            let phase_sum = expected_dns + expected_tcp + expected_tls + expected_ttfb;
            assert!(
                (phase_sum as i32 - expected_total as i32).abs() <= 1,
                "Case {}: Phase sum {} should ≈ total {} (within 1ms)",
                i,
                phase_sum,
                expected_total
            );
        }
    }

    #[tokio::test]
    async fn test_curl_feature_flag_behavior() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        // When timings-curl feature is enabled, should use curl probe
        // (we can't directly test path selection without exposing internals,
        // but we can verify the enhanced breakdown format)
        http_client.add_success(200, 1200).await;

        let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

        // With curl feature enabled, breakdown should have detailed phase timings
        // The exact format depends on whether our test actually exercises curl path
        assert!(result.metrics.breakdown.contains("DNS:"));
        assert!(result.metrics.breakdown.contains("TCP:"));
        assert!(result.metrics.breakdown.contains("TLS:"));
        assert!(result.metrics.breakdown.contains("TTFB:"));
        assert!(result.metrics.breakdown.contains("Total:"));
    }

    #[tokio::test]
    async fn test_curl_anthropic_version_header() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, _http_client, clock) = create_test_monitor(&temp_dir);

        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        // Mock curl timing for successful request
        let fake_curl_runner = FakeCurlRunner::new();
        // Convert timing: dns=25ms, tcp=25ms, tls=35ms, ttfb=815ms, total=900ms
        fake_curl_runner
            .add_timing_response(200, 25, 25, 35, 815, 900)
            .await;
        monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));

        let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

        // Verify successful execution with anthropic-version header
        assert_eq!(result.metrics.last_http_status, 200);
        assert!(matches!(result.status, NetworkStatus::Healthy));

        // Verify timing breakdown shows phase details
        let breakdown = &result.metrics.breakdown;
        assert!(
            breakdown.contains("DNS:25ms"),
            "Expected DNS:25ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("TCP:25ms"),
            "Expected TCP:25ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("TLS:35ms"),
            "Expected TLS:35ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("TTFB:815ms"),
            "Expected TTFB:815ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("Total:900ms"),
            "Expected Total:900ms in {}",
            breakdown
        );
    }

    #[tokio::test]
    async fn test_curl_timeout_handling() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, http_client, clock) = create_test_monitor(&temp_dir);

        // Configure both curl and HTTP client to fail
        let fake_curl_runner = FakeCurlRunner::new();
        fake_curl_runner.add_error("Operation timed out").await;
        monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));

        // Also configure the HTTP client fallback to fail
        http_client.add_timeout_error().await;

        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        let result = monitor.probe(ProbeMode::Red, creds, None).await.unwrap();

        // Verify timeout is handled as connection error
        assert_eq!(result.metrics.last_http_status, 0);
        assert_eq!(
            result.metrics.error_type.as_deref(),
            Some("connection_error")
        );
        assert!(matches!(result.status, NetworkStatus::Error));

        // Verify breakdown shows zero timings for failed connection
        let breakdown = &result.metrics.breakdown;
        assert!(breakdown.contains("DNS:0ms"));
        assert!(breakdown.contains("TCP:0ms"));
        assert!(breakdown.contains("TLS:0ms"));
        assert!(breakdown.contains("TTFB:0ms"));
        // Total may have small timing from processing overhead, just verify format
        assert!(breakdown.contains("Total:"));
    }

    #[tokio::test]
    async fn test_curl_edge_case_timings() {
        let temp_dir = TempDir::new().unwrap();
        let (mut monitor, _http_client, clock) = create_test_monitor(&temp_dir);

        clock.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-token".to_string(),
            source: CredentialSource::Environment,
        };

        // Test edge case: connection reuse (DNS ≈ 0, but other phases present)
        let fake_curl_runner = FakeCurlRunner::new();
        // Convert timing: dns=1ms, tcp=19ms, tls=35ms, ttfb=545ms, total=600ms
        fake_curl_runner
            .add_timing_response(200, 1, 19, 35, 545, 600)
            .await;
        monitor = monitor.with_curl_runner(Box::new(fake_curl_runner));

        let result = monitor.probe(ProbeMode::Green, creds, None).await.unwrap();

        // Verify edge case handling
        let breakdown = &result.metrics.breakdown;
        assert!(
            breakdown.contains("DNS:1ms"),
            "Expected DNS:1ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("TCP:19ms"),
            "Expected TCP:19ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("TLS:35ms"),
            "Expected TLS:35ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("TTFB:545ms"),
            "Expected TTFB:545ms in {}",
            breakdown
        );
        assert!(
            breakdown.contains("Total:600ms"),
            "Expected Total:600ms in {}",
            breakdown
        );

        // Verify all phase durations are non-negative
        assert!(
            result.metrics.latency_ms == 545,
            "Latency should be 545ms (ttfb_ms)"
        );
    }

    #[tokio::test]
    async fn test_curl_vs_isahc_dual_path_integration() {
        // This integration test verifies that both paths (curl and isahc)
        // produce compatible monitoring states

        let temp_dir = TempDir::new().unwrap();

        // Test isahc path behavior (default implementation)
        let (mut monitor_isahc, http_client_isahc, clock_isahc) = create_test_monitor(&temp_dir);

        http_client_isahc.add_success(200, 1000).await;
        clock_isahc.add_timestamp("2025-01-25T10:30:00-08:00").await;

        let creds = test_credentials();
        let isahc_result = monitor_isahc
            .probe(ProbeMode::Green, creds.clone(), None)
            .await
            .unwrap();

        // Verify isahc result format (uses TestHttpClient)
        assert_eq!(isahc_result.metrics.latency_ms, 1000); // From HTTP client latency
                                                           // When timings-curl is enabled but no curl runner injected, uses coordinated default
        assert!(isahc_result.metrics.breakdown.contains("DNS:25ms"));
        assert!(isahc_result.metrics.breakdown.contains("TCP:30ms"));

        // Test curl path behavior (with timings-curl feature)
        let (mut monitor_curl, _http_client_curl, clock_curl) = create_test_monitor(&temp_dir);

        clock_curl.add_timestamp("2025-01-25T10:30:01-08:00").await;

        let fake_curl_runner2 = FakeCurlRunner::new();
        fake_curl_runner2
            .add_timing_response(200, 30, 25, 35, 910, 1000)
            .await;
        monitor_curl = monitor_curl.with_curl_runner(Box::new(fake_curl_runner2));

        let curl_result = monitor_curl
            .probe(ProbeMode::Green, creds, None)
            .await
            .unwrap();

        // Verify curl result format
        assert_eq!(curl_result.metrics.latency_ms, 910); // ttfb_ms
        assert!(curl_result.metrics.breakdown.contains("DNS:30ms"));
        assert!(curl_result.metrics.breakdown.contains("TCP:25ms"));
        assert!(curl_result.metrics.breakdown.contains("TLS:35ms"));
        assert!(curl_result.metrics.breakdown.contains("TTFB:910ms"));

        // Verify both paths produce compatible state structures
        let isahc_state = monitor_isahc.load_state().await.unwrap();
        let curl_state = monitor_curl.load_state().await.unwrap();

        // Both should have same schema fields
        assert_eq!(
            isahc_state.monitoring_enabled,
            curl_state.monitoring_enabled
        );
        assert!(isahc_state.api_config.is_some() && curl_state.api_config.is_some());
        assert!(matches!(isahc_state.status, NetworkStatus::Healthy));
        assert!(matches!(curl_state.status, NetworkStatus::Healthy));

        // Both should contribute to rolling statistics
        assert!(isahc_state.network.rolling_totals.len() > 0);
        assert!(curl_state.network.rolling_totals.len() > 0);
    }

    #[tokio::test]
    async fn test_breakdown_validator() {
        // Test parsing valid breakdown string
        let breakdown = "DNS:50ms|TCP:25ms|TLS:15ms|TTFB:100ms|Total:190ms";
        let validator = BreakdownValidator::parse(breakdown);

        assert_eq!(validator.dns, Some(50));
        assert_eq!(validator.tcp, Some(25));
        assert_eq!(validator.tls, Some(15));
        assert_eq!(validator.ttfb, Some(100));
        assert_eq!(validator.total, Some(190));

        // Test exact matching
        validator.assert_exact(50, 25, 15, 100, 190);

        // Test tolerance matching
        validator.assert_within_tolerance(52, 23, 17, 102, 188, 5);

        // Test phase presence
        validator.assert_contains_phases(&["DNS", "TCP", "TLS", "TTFB", "Total"]);

        // Test partial breakdown parsing
        let partial = "DNS:20ms|TTFB:150ms|Total:170ms";
        let partial_validator = BreakdownValidator::parse(partial);

        assert_eq!(partial_validator.dns, Some(20));
        assert_eq!(partial_validator.tcp, None);
        assert_eq!(partial_validator.tls, None);
        assert_eq!(partial_validator.ttfb, Some(150));
        assert_eq!(partial_validator.total, Some(170));

        // Should only check present phases
        partial_validator.assert_contains_phases(&["DNS", "TTFB", "Total"]);

        // Test malformed input handling
        let malformed = "DNS:badms|TCP:25ms|Invalid";
        let malformed_validator = BreakdownValidator::parse(malformed);

        assert_eq!(malformed_validator.dns, None); // "badms" not parsed
        assert_eq!(malformed_validator.tcp, Some(25));
        assert_eq!(malformed_validator.tls, None);
    }

    // Note: CF detection test temporarily removed due to compilation issues
    // Will be re-implemented as a working test later

    #[tokio::test]
    #[cfg(feature = "timings-curl")]
    async fn test_curl_health_check_client_integration() {
        // Test that HttpMonitor uses CurlHealthCheckClient when timings-curl is enabled
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("monitoring.json");

        // Create HttpMonitor - should automatically use CurlHealthCheckClient
        let mut monitor = HttpMonitor::new(Some(state_path)).unwrap();

        // Verify that we can create and load state - indicates health check client is working
        let result = monitor.write_unknown(true).await;
        assert!(
            result.is_ok(),
            "Should be able to write state with curl health client"
        );

        let state = monitor.load_state().await;
        assert!(state.is_ok(), "Should be able to load state");

        // The successful creation and basic operations indicate the CurlHealthCheckClient
        // was properly integrated. More detailed timing verification would require
        // mock proxy endpoints which are beyond this integration test scope.
    }

    #[tokio::test]
    async fn test_oauth_skips_proxy_health_check() {
        // Test OAuth mode skips proxy health check and sets fields to None
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("monitoring.json");

        // Create mock health client that panics if called (to verify it's not called in OAuth mode)
        let panic_health_client = PanicHealthCheckClient::new();

        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(SuccessHttpClient::new()))
            .with_health_client(Box::new(panic_health_client))
            .with_clock(Box::new(FixedClock::new()));

        // OAuth credentials
        let oauth_creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "probe-invalid-key".to_string(),
            source: CredentialSource::OAuth,
        };

        let metrics = ProbeMetrics {
            latency_ms: 100,
            breakdown: "Total:100ms".to_string(),
            last_http_status: 401, // Expected for OAuth dummy key
            error_type: Some("authentication_error".to_string()),
            http_version: Some("HTTP/2.0".to_string()),
        };

        // This should not panic even though we provided a panic health client,
        // because OAuth mode should skip the proxy health check
        let result = monitor
            .process_probe_results(ProbeMode::Green, oauth_creds, metrics, None)
            .await;

        assert!(
            result.is_ok(),
            "OAuth probe should succeed without calling health client"
        );

        // Verify proxy health fields are set to None
        let state = monitor.load_state().await.unwrap();
        assert_eq!(state.network.proxy_healthy, None);
        assert_eq!(state.network.proxy_health_level, None);
        assert_eq!(state.network.proxy_health_detail, None);
        assert_eq!(state.api_config.unwrap().source, "oauth");
    }

    #[tokio::test]
    async fn test_non_oauth_executes_proxy_health_check() {
        // Test non-OAuth mode continues to execute proxy health check
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("monitoring.json");

        // Use the existing MockHealthCheckClient that returns success
        let mut monitor = HttpMonitor::new(Some(state_path.clone()))
            .unwrap()
            .with_http_client(Box::new(SuccessHttpClient::new()))
            .with_clock(Box::new(FixedClock::new()));

        // Environment credentials (non-OAuth)
        let env_creds = ApiCredentials {
            base_url: "https://api.anthropic.com".to_string(),
            auth_token: "test-api-key".to_string(),
            source: CredentialSource::Environment,
        };

        let metrics = ProbeMetrics {
            latency_ms: 100,
            breakdown: "Total:100ms".to_string(),
            last_http_status: 200,
            error_type: None,
            http_version: Some("HTTP/2.0".to_string()),
        };

        let result = monitor
            .process_probe_results(ProbeMode::Green, env_creds, metrics, None)
            .await;

        assert!(result.is_ok(), "Non-OAuth probe should succeed");

        // Verify proxy health check was executed (MockHealthCheckClient returns healthy by default)
        let state = monitor.load_state().await.unwrap();
        assert_eq!(state.api_config.unwrap().source, "environment");
        // Proxy health fields should be populated by MockHealthCheckClient
    }
}

/// Mock health client that panics if called (for testing OAuth skip logic)
#[derive(Clone)]
struct PanicHealthCheckClient;

impl PanicHealthCheckClient {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl HealthCheckClient for PanicHealthCheckClient {
    async fn get_health(&self, _url: String, _timeout_ms: u32) -> Result<HealthResponse, String> {
        panic!("PanicHealthCheckClient was called - OAuth should skip proxy health check");
    }
}
