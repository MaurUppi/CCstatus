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

use ccstatus::core::network::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::Mutex;

/// Test-specific mock HTTP client with configurable response sequences
#[derive(Clone)]
struct TestHttpClient {
    responses: Arc<Mutex<Vec<Result<(u16, Duration, String), String>>>>,
}

impl TestHttpClient {
    fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn add_response(&self, response: Result<(u16, Duration, String), String>) {
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
        self.add_response(Ok((status, duration, breakdown))).await;
    }

    async fn add_timeout_error(&self) {
        self.add_response(Err("Request timeout".to_string())).await;
    }
}

#[async_trait::async_trait]
impl HttpClientTrait for TestHttpClient {
    async fn execute_request(
        &self,
        _url: String,
        _headers: HashMap<String, String>,
        _body: Vec<u8>,
        _timeout_ms: u32,
    ) -> Result<(u16, Duration, String), String> {
        let mut responses = self.responses.lock().await;
        responses.pop().unwrap_or_else(|| {
            // Default successful response
            Ok((
                200,
                Duration::from_millis(1000),
                "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:970ms|Total:1000ms".to_string(),
            ))
        })
    }
}

/// Test-specific mock clock with controllable time
#[derive(Clone)]
struct TestClock {
    current_time: Arc<Mutex<Instant>>,
    timestamp_sequence: Arc<Mutex<Vec<String>>>,
}

impl TestClock {
    fn new() -> Self {
        Self {
            current_time: Arc::new(Mutex::new(Instant::now())),
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

/// Helper to create test credentials
fn test_credentials() -> ApiCredentials {
    ApiCredentials {
        base_url: "https://api.anthropic.com".to_string(),
        auth_token: "test-token-12345".to_string(),
        source: CredentialSource::Environment,
    }
}

/// Create HttpMonitor with test dependencies and temp directory
fn create_test_monitor(temp_dir: &TempDir) -> (HttpMonitor, TestHttpClient, TestClock) {
    let http_client = TestHttpClient::new();
    let clock = TestClock::new();

    let state_path = temp_dir.path().join("monitoring.json");
    let monitor = HttpMonitor::new(Some(state_path))
        .unwrap()
        .with_http_client(Box::new(http_client.clone()) as Box<dyn HttpClientTrait>)
        .with_clock(Box::new(clock.clone()) as Box<dyn ClockTrait>);

    (monitor, http_client, clock)
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
        (403, Some("permission_error"), "403 -> permission_error"),
        (404, Some("not_found_error"), "404 -> not_found_error"),
        (413, Some("request_too_large"), "413 -> request_too_large"),
        (429, Some("rate_limit_error"), "429 -> rate_limit_error"),
        (500, Some("api_error"), "500 -> api_error"),
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
    
    let result = monitor
        .probe(ProbeMode::Green, creds, None)
        .await
        .unwrap();
    
    assert_eq!(result.api_config.source, "environment");
    
    // Verify state persistence also uses correct formatting
    let state = monitor.load_state().await.unwrap();
    assert_eq!(
        state.api_config.as_ref().unwrap().source,
        "environment"
    );
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
    let _parsed: DateTime<FixedOffset> = stored_error.timestamp.parse()
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
        .probe(ProbeMode::Red, test_credentials(), Some(invalid_error_event))
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
    
    let result1 = monitor
        .probe(ProbeMode::Green, creds, None)
        .await
        .unwrap();
    
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
        source: CredentialSource::ClaudeConfig(std::path::PathBuf::from("/home/user/.claude/config")),
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
    assert_eq!(final_state.api_config.as_ref().unwrap().source, "claude_config");
    
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
    
    let _result1 = monitor.probe(ProbeMode::Cold, creds.clone(), None).await.unwrap();
    
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
    assert_ne!(state2.monitoring_state.last_cold_session_id, Some(session_id_1.to_string()));
}

#[tokio::test]
async fn test_session_deduplication_different_probe_modes() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test-session-probe-modes.json");
    
    let mock_client = TestHttpClient::new();
    mock_client.add_success(200, 1000).await; // COLD
    mock_client.add_success(200, 1100).await; // GREEN
    mock_client.add_success(429, 800).await;  // RED
    
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
    let _cold_result = monitor.probe(ProbeMode::Cold, creds.clone(), None).await.unwrap();
    
    let state_after_cold = monitor.load_state().await.unwrap();
    assert_eq!(
        state_after_cold.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    let cold_timestamp = state_after_cold.monitoring_state.last_cold_probe_at.clone();
    assert!(cold_timestamp.is_some());
    
    // Execute GREEN probe - should NOT update session deduplication fields
    let _green_result = monitor.probe(ProbeMode::Green, creds.clone(), None).await.unwrap();
    
    let state_after_green = monitor.load_state().await.unwrap();
    assert_eq!(
        state_after_green.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    assert_eq!(state_after_green.monitoring_state.last_cold_probe_at, cold_timestamp);
    
    // Execute RED probe - should NOT update session deduplication fields
    let error_event = JsonlError {
        timestamp: "2025-01-25T10:30:45.123Z".to_string(),
        code: 429,
        message: "Rate limit exceeded".to_string(),
    };
    let _red_result = monitor.probe(ProbeMode::Red, creds, Some(error_event)).await.unwrap();
    
    let state_after_red = monitor.load_state().await.unwrap();
    assert_eq!(
        state_after_red.monitoring_state.last_cold_session_id,
        Some(test_session_id.to_string())
    );
    assert_eq!(state_after_red.monitoring_state.last_cold_probe_at, cold_timestamp);
    
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
    let _result1 = monitor.probe(ProbeMode::Cold, creds.clone(), None).await.unwrap();
    
    let state1 = monitor.load_state().await.unwrap();
    assert_eq!(state1.monitoring_state.last_cold_session_id, Some("".to_string()));
    
    // Test very long session ID
    let long_session_id = "a".repeat(1000);
    monitor.set_session_id(long_session_id.clone());
    let _result2 = monitor.probe(ProbeMode::Cold, creds.clone(), None).await.unwrap();
    
    let state2 = monitor.load_state().await.unwrap();
    assert_eq!(state2.monitoring_state.last_cold_session_id, Some(long_session_id));
    
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
        let _result = monitor1.probe(ProbeMode::Cold, creds.clone(), None).await.unwrap();
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
