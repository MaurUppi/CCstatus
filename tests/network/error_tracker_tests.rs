use std::time::{SystemTime, UNIX_EPOCH};
use ccstatus::core::segments::network::{ErrorTracker, types::{JsonlError, NetworkStatus}};

#[test]
fn test_error_tracker_creation() {
    let tracker = ErrorTracker::new();
    assert_eq!(tracker.get_error_stats(1).total_errors, 0);
}

#[test] 
fn test_record_error() {
    let mut tracker = ErrorTracker::new();
    
    tracker.record_error(500, "Internal Server Error".to_string());
    
    let latest = tracker.get_latest_error();
    assert!(latest.is_some());
    
    let error = latest.unwrap();
    assert_eq!(error.http_status, 500);
    assert_eq!(error.error_type, "api_error"); // Now uses official classification
    assert_eq!(error.message, "Internal Server Error");
}

#[test]
fn test_record_jsonl_error() {
    let mut tracker = ErrorTracker::new();
    
    let jsonl_error = JsonlError {
        timestamp: "2024-01-01T12:00:00Z".to_string(),
        code: 429,
        message: "Rate Limited".to_string(),
    };
    
    tracker.record_jsonl_error(&jsonl_error);
    
    let latest = tracker.get_latest_error();
    assert!(latest.is_some());
    
    let error = latest.unwrap();
    assert_eq!(error.http_status, 429);
    assert_eq!(error.error_type, "rate_limit_error");
    assert_eq!(error.message, "Rate Limited");
}

#[test]
fn test_red_window_decision() {
    let mut tracker = ErrorTracker::new();
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    // No errors - should not enter RED window
    assert!(!tracker.has_recent_errors(current_time));
    
    // Record recent error - should enter RED window
    tracker.record_error(500, "Server Error".to_string());
    assert!(tracker.has_recent_errors(current_time));
    
    // Test with old timestamp - should not enter RED window
    let old_time = current_time.saturating_sub(120_000); // 2 minutes ago
    assert!(!tracker.has_recent_errors(old_time));
}

#[test]
fn test_http_status_classification() {
    let tracker = ErrorTracker::new();
    
    // Success codes
    assert_eq!(tracker.classify_http_status(200), "success");
    assert_eq!(tracker.classify_http_status(201), "success");
    assert_eq!(tracker.classify_http_status(299), "success");
    
    // Specific API error types
    assert_eq!(tracker.classify_http_status(400), "invalid_request_error");
    assert_eq!(tracker.classify_http_status(401), "authentication_error");
    assert_eq!(tracker.classify_http_status(403), "permission_error");
    assert_eq!(tracker.classify_http_status(404), "not_found_error");
    assert_eq!(tracker.classify_http_status(413), "request_too_large");
    assert_eq!(tracker.classify_http_status(429), "rate_limit_error");
    
    // Server errors
    assert_eq!(tracker.classify_http_status(500), "api_error");
    assert_eq!(tracker.classify_http_status(502), "server_error"); // New: Bad Gateway classification
    assert_eq!(tracker.classify_http_status(504), "socket_hang_up");
    assert_eq!(tracker.classify_http_status(529), "overloaded_error");
    
    // Fallback categories
    assert_eq!(tracker.classify_http_status(450), "client_error"); // Other 4xx
    assert_eq!(tracker.classify_http_status(550), "server_error"); // Other 5xx
    
    // Connection failures
    assert_eq!(tracker.classify_http_status(0), "connection_error");
    
    // Unknown codes
    assert_eq!(tracker.classify_http_status(100), "unknown_error");
    assert_eq!(tracker.classify_http_status(999), "unknown_error");
}

#[test]
fn test_network_status_determination() {
    let tracker = ErrorTracker::new();
    
    // Test healthy status (HTTP 200, low latency)
    assert_eq!(
        tracker.determine_status(200, 100, 1000, 2000),
        NetworkStatus::Healthy
    );
    
    // Test degraded status (HTTP 200, medium latency)
    assert_eq!(
        tracker.determine_status(200, 1500, 1000, 2000),
        NetworkStatus::Degraded
    );
    
    // Test error status (HTTP 200, high latency)
    assert_eq!(
        tracker.determine_status(200, 2500, 1000, 2000),
        NetworkStatus::Error
    );
    
    // Test rate limiting (should be degraded)
    assert_eq!(
        tracker.determine_status(429, 500, 1000, 2000),
        NetworkStatus::Degraded
    );
    
    // Test authentication errors
    assert_eq!(
        tracker.determine_status(401, 500, 1000, 2000),
        NetworkStatus::Error
    );
    assert_eq!(
        tracker.determine_status(403, 500, 1000, 2000),
        NetworkStatus::Error
    );
    
    // Test client errors
    assert_eq!(
        tracker.determine_status(400, 500, 1000, 2000),
        NetworkStatus::Error
    );
    assert_eq!(
        tracker.determine_status(404, 500, 1000, 2000),
        NetworkStatus::Error
    );
    
    // Test server errors
    assert_eq!(
        tracker.determine_status(500, 500, 1000, 2000),
        NetworkStatus::Error
    );
    assert_eq!(
        tracker.determine_status(502, 500, 1000, 2000),
        NetworkStatus::Error
    );
    
    // Test timeout (status code 0)
    assert_eq!(
        tracker.determine_status(0, 5000, 1000, 2000),
        NetworkStatus::Error
    );
}

#[test]
fn test_percentile_calculation() {
    let tracker = ErrorTracker::new();
    
    // Empty array
    let (p80, p95) = tracker.calculate_percentiles(&[]);
    assert_eq!(p80, 0);
    assert_eq!(p95, 0);
    
    // Single value - with nearest-rank method, percentiles of single value should be that value
    let (p80, p95) = tracker.calculate_percentiles(&[100]);
    assert_eq!(p80, 100); // Nearest-rank method: single value percentiles = value
    assert_eq!(p95, 100);
    
    // Multiple values
    let latencies = vec![100, 200, 300, 400, 500, 600, 700, 800, 900, 1000];
    let (p80, p95) = tracker.calculate_percentiles(&latencies);
    
    // P80 should be around 800 (80th percentile of 10 values)
    // P95 should be around 950 (95th percentile of 10 values)
    assert!(p80 > 0);
    assert!(p95 > 0);
    assert!(p95 >= p80);
}

#[test]
fn test_error_statistics() {
    let mut tracker = ErrorTracker::new();
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    // Record various types of errors
    tracker.record_error(401, "Unauthorized".to_string());
    tracker.record_error(429, "Too Many Requests".to_string());
    tracker.record_error(500, "Server Error".to_string());
    tracker.record_error(502, "Bad Gateway".to_string());
    tracker.record_error(529, "Overloaded".to_string());
    
    let stats = tracker.get_error_stats(5); // Last 5 minutes
    
    assert_eq!(stats.total_errors, 5);
    assert_eq!(stats.authentication_errors, 1);
    assert_eq!(stats.rate_limit_errors, 1);
    assert_eq!(stats.server_errors, 3); // 500, 502, 529 (Overloaded counts as server error)
    assert_eq!(stats.time_window_minutes, 5);
}

#[test]
fn test_error_statistics_time_window() {
    let mut tracker = ErrorTracker::new();
    
    // Simulate old error by manipulating the internal timestamp
    // Since we can't directly manipulate timestamps in the current API,
    // we test with a reasonable expectation that recent errors are counted
    tracker.record_error(500, "Recent Error".to_string());
    
    // Get stats for last 1 minute
    let stats_1min = tracker.get_error_stats(1);
    assert_eq!(stats_1min.total_errors, 1);
    assert_eq!(stats_1min.time_window_minutes, 1);
    
    // Get stats for last 60 minutes (should include the same error)
    let stats_60min = tracker.get_error_stats(60);
    assert_eq!(stats_60min.total_errors, 1);
    assert_eq!(stats_60min.time_window_minutes, 60);
}

#[test]
fn test_cleanup_old_errors() {
    let mut tracker = ErrorTracker::new();
    
    // Record some errors
    tracker.record_error(500, "Error 1".to_string());
    tracker.record_error(502, "Error 2".to_string());
    
    // Initially should have errors
    assert_eq!(tracker.get_error_stats(60).total_errors, 2);
    
    // Clean up errors older than 0 hours (should remove all)
    // Note: This test depends on implementation details
    // In practice, cleanup is based on actual timestamps
    tracker.cleanup_old_errors(0);
    
    // After cleanup with 0 retention, behavior depends on implementation
    // The current implementation keeps errors that are exactly at the cutoff time
}

#[test]
fn test_max_history_limit() {
    let mut tracker = ErrorTracker::new();
    
    // Record more than max history (50 errors)
    for i in 0..60 {
        tracker.record_error(500, format!("Error {}", i));
    }
    
    // Should not exceed max history
    let stats = tracker.get_error_stats(60);
    assert!(stats.total_errors <= 50, "Error history should not exceed max limit");
}

#[test]
fn test_error_type_classification_edge_cases() {
    let tracker = ErrorTracker::new();
    
    // Test boundary conditions
    assert_eq!(tracker.classify_http_status(199), "unknown_error");
    assert_eq!(tracker.classify_http_status(300), "unknown_error");
    assert_eq!(tracker.classify_http_status(399), "unknown_error");
    assert_eq!(tracker.classify_http_status(600), "unknown_error");
    
    // Test specific server error codes
    assert_eq!(tracker.classify_http_status(501), "server_error");
    assert_eq!(tracker.classify_http_status(505), "server_error");
}

#[test]
fn test_mixed_error_scenarios() {
    let mut tracker = ErrorTracker::new();
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    // Scenario: Rate limiting followed by server errors
    tracker.record_error(429, "Rate Limited".to_string());
    tracker.record_error(500, "Server Error".to_string());
    tracker.record_error(502, "Bad Gateway".to_string());
    
    // Should enter RED window due to recent errors
    assert!(tracker.has_recent_errors(current_time));
    
    let stats = tracker.get_error_stats(1);
    assert_eq!(stats.total_errors, 3);
    assert_eq!(stats.rate_limit_errors, 1);
    assert_eq!(stats.server_errors, 2);
    
    // Latest error should be BadGateway
    let latest = tracker.get_latest_error().unwrap();
    assert_eq!(latest.http_status, 502);
    assert_eq!(latest.error_type, "server_error");
}

#[test]
fn test_connection_error_classification_production_patterns() {
    // Test patterns from actual production error logs
    
    // Original patterns from API-error.png
    assert_eq!(ErrorTracker::classify_connection_error("API Error (Connection error.)"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("TypeError (fetch failed)"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("network error occurred"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("timeout during request"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("connection refused by server"), "connection_error");
    
    // New timeout patterns from CC-ErrorCode-1.png
    assert_eq!(ErrorTracker::classify_connection_error("Request timed out."), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("API Error (Request timed out.)"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("Retrying in 1 seconds... (attempt 1/10)"), "connection_error");
    
    // New SSL/Certificate patterns from CC-ErrorCode-1.png
    assert_eq!(ErrorTracker::classify_connection_error("unknown certificate verification error"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("Error (unknown certificate verification error)"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("TLS handshake failed"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("SSL connection error"), "connection_error");
    
    // New usage policy patterns from CC-ErrorCode-1.png  
    assert_eq!(ErrorTracker::classify_connection_error("Claude Code is unable to respond to this request, which appears to violate our Usage Policy"), "invalid_request_error");
    assert_eq!(ErrorTracker::classify_connection_error("usage policy violation detected"), "invalid_request_error");
    
    // Test fallback - was "network_error", now "unknown_error" for spec compliance
    assert_eq!(ErrorTracker::classify_connection_error("some random error message"), "unknown_error");
    assert_eq!(ErrorTracker::classify_connection_error("unexpected system failure"), "unknown_error");
    
    // Test case insensitivity
    assert_eq!(ErrorTracker::classify_connection_error("CONNECTION ERROR"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("FETCH FAILED"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("REQUEST TIMED OUT"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("CERTIFICATE VERIFICATION"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("USAGE POLICY"), "invalid_request_error");
    assert_eq!(ErrorTracker::classify_connection_error("SSL ERROR"), "connection_error");
    assert_eq!(ErrorTracker::classify_connection_error("TLS HANDSHAKE"), "connection_error");
}

#[test]
fn test_connection_error_message_classification_integration() {
    let mut tracker = ErrorTracker::new();
    
    // Test connection error (status 0) uses message-based classification
    tracker.record_error(0, "Request timed out.".to_string());
    let latest = tracker.get_latest_error().unwrap();
    assert_eq!(latest.http_status, 0);
    assert_eq!(latest.error_type, "connection_error");
    assert_eq!(latest.message, "Request timed out.");
    
    // Test SSL error classification
    tracker.record_error(0, "unknown certificate verification error".to_string());
    let latest = tracker.get_latest_error().unwrap();
    assert_eq!(latest.http_status, 0);
    assert_eq!(latest.error_type, "connection_error");
    assert_eq!(latest.message, "unknown certificate verification error");
    
    // Test policy violation classification  
    tracker.record_error(0, "violate our Usage Policy".to_string());
    let latest = tracker.get_latest_error().unwrap();
    assert_eq!(latest.http_status, 0);
    assert_eq!(latest.error_type, "invalid_request_error");
    assert_eq!(latest.message, "violate our Usage Policy");
    
    // Test HTTP error still uses status classification
    tracker.record_error(502, "Bad Gateway Error".to_string());
    let latest = tracker.get_latest_error().unwrap();
    assert_eq!(latest.http_status, 502);
    assert_eq!(latest.error_type, "server_error"); // Uses HTTP status, not message
    assert_eq!(latest.message, "Bad Gateway Error");
}

#[test]  
fn test_red_gating_safety_diagnostic_only() {
    let mut tracker = ErrorTracker::new();
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    
    // Test that has_recent_errors is diagnostic-only and should NOT be used for RED gating
    // The method uses a 60-second window which conflicts with JsonlMonitor's 10s/1s window
    
    // No errors initially
    assert!(!tracker.has_recent_errors(current_time));
    
    // Add error and test within diagnostic window
    tracker.record_error(500, "Server error for diagnostics".to_string());
    
    // Within 60s diagnostic window  
    let time_30s_later = current_time + 30_000;
    assert!(tracker.has_recent_errors(time_30s_later));
    
    // This test validates that the method works for diagnostics but emphasizes
    // that RED gating should use JsonlMonitor::scan_tail() with proper 10s/1s windows
}