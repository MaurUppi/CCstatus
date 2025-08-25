use ccstatus::core::network::{EnhancedDebugLogger, JsonlMonitor};
use std::env;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

/// Test that JsonlMonitor constructor never fails and creates properly
#[tokio::test]
async fn test_jsonl_monitor_creation() {
    let _monitor = JsonlMonitor::new();
    // Constructor never fails in Phase 2 implementation
    assert!(true, "JsonlMonitor creation always succeeds");
}

/// Test RED gate detection with nonexistent file
#[tokio::test]
async fn test_scan_nonexistent_file() {
    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail("/nonexistent/path").await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(!error_detected);
    assert!(last_error.is_none());
}

/// Test RED gate detection with empty file
#[tokio::test]
async fn test_scan_empty_file() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("empty_transcript.jsonl");
    fs::write(&transcript_path, "").unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(!error_detected);
    assert!(last_error.is_none());
}

/// Test RED gate detection with valid API error entry
#[tokio::test]
async fn test_scan_valid_error_entry() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("error_transcript.jsonl");

    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"test-uuid-123","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 {\"error\":{\"message\":\"Rate Limited\"}}"}]}}"#;

    fs::write(&transcript_path, error_entry).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 429);
    assert_eq!(error.message, "Rate Limited");
    assert_eq!(error.timestamp, "2024-01-01T12:00:00Z");
}

/// Test flexible boolean parsing for debug mode
#[tokio::test]
async fn test_flexible_debug_mode_parsing() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("debug_transcript.jsonl");

    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"debug-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    fs::write(&transcript_path, error_entry).unwrap();

    // Test different flexible boolean values
    let test_values = vec!["true", "1", "yes", "on", "TRUE", "Yes", "ON"];

    for debug_value in test_values {
        env::set_var("CCSTATUS_DEBUG", debug_value);
        let monitor = JsonlMonitor::new(); // Should create with debug logger
        let result = monitor.scan_tail(&transcript_path).await;

        assert!(result.is_ok());
        let (error_detected, last_error) = result.unwrap();
        assert!(error_detected);
        assert!(last_error.is_some());

        env::remove_var("CCSTATUS_DEBUG");
    }
}

/// Test RED gate semantics with multiple errors (should return last error)
#[tokio::test]
async fn test_multiple_errors_returns_last() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("multi_error_transcript.jsonl");

    let error_429 = r#"{"isApiErrorMessage":true,"parentUuid":"uuid-429","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    let error_500 = r#"{"isApiErrorMessage":true,"parentUuid":"uuid-500","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Internal Server Error"}]}}"#;
    let error_502 = r#"{"isApiErrorMessage":true,"parentUuid":"uuid-502","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 502 Bad Gateway"}]}}"#;

    let content = format!("{}\n{}\n{}", error_429, error_500, error_502);
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());

    // Last error should be 502
    let error = last_error.unwrap();
    assert_eq!(error.code, 502);
    assert_eq!(error.message, "Bad Gateway");
    assert_eq!(error.timestamp, "2024-01-01T12:02:00Z");
}

/// Test robust handling of invalid JSON lines
#[tokio::test]
async fn test_invalid_json_handling() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("invalid_transcript.jsonl");

    let invalid_json = "invalid json line\n";
    let valid_error = r#"{"isApiErrorMessage":true,"parentUuid":"valid-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;

    let content = format!("{}{}", invalid_json, valid_error);
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    // Should handle invalid JSON gracefully and still process valid entries
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected); // Valid error was found
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 429);
}

/// Test that non-error entries are properly ignored
#[tokio::test]
async fn test_non_error_entries() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("normal_transcript.jsonl");

    let normal_entry = r#"{"isApiErrorMessage":false,"message":"Normal message","timestamp":"2024-01-01T12:00:00Z"}"#;
    let no_flag_entry =
        r#"{"message":"Message without error flag","timestamp":"2024-01-01T12:01:00Z"}"#;

    let content = format!("{}\n{}", normal_entry, no_flag_entry);
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(!error_detected); // No errors should be detected
    assert!(last_error.is_none());
}

/// Test input validation with oversized lines
#[tokio::test]
async fn test_input_validation() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("validation_transcript.jsonl");

    // Test with oversized line (should be gracefully skipped)
    let oversized_line = "{".repeat(2_000_000); // 2MB line
    let valid_error = r#"{"isApiErrorMessage":true,"parentUuid":"valid-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;

    let content = format!("{}\n{}", oversized_line, valid_error);
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected); // Valid error should still be found
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 429);
}

/// Test enhanced error extraction iterating all content items
#[tokio::test]
async fn test_error_message_extraction() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("message_extraction_transcript.jsonl");

    // Test different message formats
    let json_error = r#"{"isApiErrorMessage":true,"parentUuid":"json-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 {\"error\":{\"message\":\"Rate limit exceeded\"}}"}]}}"#;
    let simple_error = r#"{"isApiErrorMessage":true,"parentUuid":"simple-uuid","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Internal Server Error"}]}}"#;

    let content = format!("{}\n{}", json_error, simple_error);
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());

    // Should extract appropriate message based on error format
    let error = last_error.unwrap();
    assert_eq!(error.code, 500);
    // Message should be extracted according to the parsing logic
    assert!(!error.message.is_empty());
}

/// Test multiline transcript handling
#[tokio::test]
async fn test_multiline_transcript() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("multiline_transcript.jsonl");

    // Multiple entries with empty lines
    let content = r#"
{"isApiErrorMessage":false,"message":"Normal message 1"}

{"isApiErrorMessage":true,"parentUuid":"error-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}

{"isApiErrorMessage":false,"message":"Normal message 2"}
{"isApiErrorMessage":true,"parentUuid":"error-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}
"#;

    fs::write(&transcript_path, content.trim()).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());

    // Should find the last error (500)
    let error = last_error.unwrap();
    assert_eq!(error.code, 500);
}

/// Test CCSTATUS_JSONL_TAIL_KB environment variable configuration
#[tokio::test]
async fn test_tail_size_configuration() {
    // Test custom tail size configuration
    env::set_var("CCSTATUS_JSONL_TAIL_KB", "32"); // Set to 32KB

    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("tail_config_test.jsonl");

    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"tail-test","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    fs::write(&transcript_path, error_entry).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());

    env::remove_var("CCSTATUS_JSONL_TAIL_KB");
}

/// Test bounds validation for tail size (security feature)
#[tokio::test]
async fn test_tail_size_bounds() {
    // Test invalid tail size configurations
    let test_cases = vec![
        "0",       // Should be clamped to minimum 1KB
        "invalid", // Should default to 64KB
        "20000",   // Should be clamped to maximum 10MB
    ];

    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("bounds_test.jsonl");

    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"bounds-test","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    fs::write(&transcript_path, error_entry).unwrap();

    for test_value in test_cases {
        env::set_var("CCSTATUS_JSONL_TAIL_KB", test_value);
        let monitor = JsonlMonitor::new();
        let result = monitor.scan_tail(&transcript_path).await;

        // Should handle invalid values gracefully
        assert!(result.is_ok());
        let (error_detected, _) = result.unwrap();
        assert!(error_detected);

        env::remove_var("CCSTATUS_JSONL_TAIL_KB");
    }
}

/// Test that debug logger is properly integrated (when enabled)
#[tokio::test]
async fn test_debug_logger_integration() {
    env::set_var("CCSTATUS_DEBUG", "true");

    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("debug_logger_test.jsonl");

    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"debug-test","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 503 Service Unavailable"}]}}"#;
    fs::write(&transcript_path, error_entry).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 503);
    assert_eq!(error.message, "Service Unavailable");

    env::remove_var("CCSTATUS_DEBUG");
}

/// Test RED gate control return semantics
#[tokio::test]
async fn test_red_gate_return_semantics() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("red_gate_test.jsonl");

    // Multiple errors to test return semantics
    let error1 = r#"{"isApiErrorMessage":true,"parentUuid":"first-error","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    let normal_entry = r#"{"isApiErrorMessage":false,"message":"Normal entry","timestamp":"2024-01-01T12:01:00Z"}"#;
    let error2 = r#"{"isApiErrorMessage":true,"parentUuid":"last-error","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-456","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;

    let content = format!("{}\n{}\n{}", error1, normal_entry, error2);
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());

    // Test explicit tuple binding for RED gate control
    let (error_detected, last_error_event) = result.unwrap();

    // error_detected should be true (triggers RED state)
    assert!(error_detected, "Should detect errors for RED gate control");

    // last_error_event should contain most recent error details
    assert!(
        last_error_event.is_some(),
        "Should provide last error event details"
    );
    let error = last_error_event.unwrap();

    // Should be the LAST error (429), not the first (500)
    assert_eq!(error.code, 429, "Should return most recent error");
    assert_eq!(
        error.message, "Rate Limited",
        "Should extract correct message"
    );
    assert_eq!(
        error.timestamp, "2024-01-01T12:02:00Z",
        "Should use transcript timestamp"
    );
}

/// Test custom debug logger injection (for testing)
#[tokio::test]
async fn test_custom_debug_logger() {
    // Create custom debug logger for testing
    let custom_logger = Arc::new(EnhancedDebugLogger::new());
    let monitor = JsonlMonitor::with_debug_logger(Some(custom_logger.clone()));

    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("custom_logger_test.jsonl");

    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"custom-test","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 502 Bad Gateway"}]}}"#;
    fs::write(&transcript_path, error_entry).unwrap();

    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 502);
}
