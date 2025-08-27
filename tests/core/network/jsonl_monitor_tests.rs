use ccstatus::core::network::{get_debug_logger, EnhancedDebugLogger, JsonlMonitor};
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

// =============================================================================
// ENHANCEMENT TEST CASES: Fallback Detection for Missing isApiErrorMessage Flag
// =============================================================================

/// Test fallback detection when isApiErrorMessage is false but API error text is present
#[tokio::test]
async fn test_fallback_detection_flag_false() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("fallback_false_flag.jsonl");

    let entry = r#"{"isApiErrorMessage":false,"parentUuid":"fallback-test","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate limit exceeded"}]}}"#;
    fs::write(&transcript_path, entry).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect error via fallback path when flag is false"
    );
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 429);
    assert_eq!(error.timestamp, "2024-01-01T12:00:00Z");
}

/// Test fallback detection when isApiErrorMessage is missing
#[tokio::test]
async fn test_fallback_detection_flag_missing() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("fallback_missing_flag.jsonl");

    let entry = r#"{"parentUuid":"fallback-missing","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Internal server error"}]}}"#;
    fs::write(&transcript_path, entry).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect error via fallback path when flag is missing"
    );
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 500);
}

/// Test case-insensitive API error pattern matching
#[tokio::test]
async fn test_case_insensitive_error_detection() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("case_insensitive_test.jsonl");

    // Test various case combinations
    let entries = vec![
        r#"{"isApiErrorMessage":false,"parentUuid":"case-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#,
        r#"{"parentUuid":"case-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error: 500 Server Error"}]}}"#,
        r#"{"isApiErrorMessage":false,"parentUuid":"case-3","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"api error: 502 Bad Gateway"}]}}"#,
        r#"{"parentUuid":"case-4","timestamp":"2024-01-01T12:03:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"Api Error: 503 Service Unavailable"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect errors with various case patterns"
    );
    assert!(last_error.is_some());

    // Should return the last error (503)
    let error = last_error.unwrap();
    assert_eq!(error.code, 503);
    assert_eq!(error.timestamp, "2024-01-01T12:03:00Z");
}

/// Test API error without explicit HTTP code (should default to code 0)
#[tokio::test]
async fn test_api_error_no_code() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("no_code_test.jsonl");

    let entries = vec![
        r#"{"isApiErrorMessage":false,"parentUuid":"no-code-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error occurred"}]}}"#,
        r#"{"parentUuid":"no-code-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"api error - something went wrong"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect API errors without explicit codes"
    );
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(
        error.code, 0,
        "Should default to code 0 when no explicit code"
    );
    assert_eq!(
        error.message, "API Error",
        "Should provide generic error message"
    );
}

/// Test mixed content arrays where error text is in different positions
#[tokio::test]
async fn test_mixed_content_arrays() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("mixed_content_test.jsonl");

    // Error text in second content item
    let entry1 = r#"{"isApiErrorMessage":false,"parentUuid":"mixed-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"Normal text"},{"text":"API Error: 429 Rate Limited"},{"text":"More text"}]}}"#;
    // Error text in third content item
    let entry2 = r#"{"parentUuid":"mixed-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"First item"},{"text":"Second item"},{"text":"api error: 500 server failure"}]}}"#;

    let content = format!("{}\n{}", entry1, entry2);
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect error text in any content array position"
    );
    assert!(last_error.is_some());

    // Should return the last error (500)
    let error = last_error.unwrap();
    assert_eq!(error.code, 500);
}

/// Test that fallback detection coexists with existing flag-based detection
#[tokio::test]
async fn test_fallback_and_flag_coexistence() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("coexistence_test.jsonl");

    // Mix of flag-based and fallback detection
    let flag_based = r#"{"isApiErrorMessage":true,"parentUuid":"flag-based","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    let fallback_false = r#"{"isApiErrorMessage":false,"parentUuid":"fallback-false","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error: 500 Server Error"}]}}"#;
    let fallback_missing = r#"{"parentUuid":"fallback-missing","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"api error: 502 Bad Gateway"}]}}"#;
    let normal = r#"{"isApiErrorMessage":false,"parentUuid":"normal","timestamp":"2024-01-01T12:03:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"Normal message"}]}}"#;

    let content = format!(
        "{}\n{}\n{}\n{}",
        flag_based, fallback_false, fallback_missing, normal
    );
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect both flag-based and fallback errors"
    );
    assert!(last_error.is_some());

    // Should return the last error (502 from fallback detection)
    let error = last_error.unwrap();
    assert_eq!(error.code, 502);
}

/// Test that non-API error text is not falsely detected
#[tokio::test]
async fn test_false_positive_prevention() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("false_positive_test.jsonl");

    let entries = vec![
        r#"{"isApiErrorMessage":false,"parentUuid":"false-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"This is not an API error message"}]}}"#,
        r#"{"parentUuid":"false-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"Something about API error in the middle"}]}}"#,
        r#"{"isApiErrorMessage":false,"parentUuid":"false-3","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"Error: Not an API error"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        !error_detected,
        "Should not falsely detect non-API error text"
    );
    assert!(last_error.is_none());
}

/// Test edge case with empty content arrays
#[tokio::test]
async fn test_empty_content_arrays() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("empty_content_test.jsonl");

    let entries = vec![
        r#"{"isApiErrorMessage":false,"parentUuid":"empty-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[]}}"#,
        r#"{"parentUuid":"empty-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{}}"#,
        r#"{"isApiErrorMessage":false,"parentUuid":"missing-message","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path"}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(
        !error_detected,
        "Should handle empty/missing content gracefully"
    );
    assert!(last_error.is_none());
}

/// Test UTF-8 safety in fallback detection logging with unicode characters
#[tokio::test]
async fn test_utf8_safety_fallback_detection() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("utf8_fallback_test.jsonl");

    // Test with emoji and unicode characters that would cause panic if sliced unsafely at byte 50
    let entries = vec![
        // 48 ASCII chars + emoji at position 49-50 boundary (would panic with &text[..50])
        r#"{"isApiErrorMessage":false,"parentUuid":"utf8-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: Rate limited - too many requests 😢🚨"}]}}"#,
        // Long unicode text with multibyte chars that cross 50-char boundary
        r#"{"parentUuid":"utf8-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error: 429 - Émojis and ünïcödé characters everywhere! 🔥💯✨🌟💎🎯"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    // Enable debug logging to trigger the UTF-8 safety code paths
    let debug_logger = Some(Arc::new(get_debug_logger()));
    let monitor = JsonlMonitor::with_debug_logger(debug_logger);
    let result = monitor.scan_tail(&transcript_path).await;

    // Should succeed without panicking on UTF-8 boundaries
    assert!(
        result.is_ok(),
        "Should handle UTF-8 characters safely in debug logging"
    );
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect API errors with unicode characters"
    );
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 429);
}

/// Test UTF-8 safety in malformed JSON logging with unicode error messages
#[tokio::test]
async fn test_utf8_safety_malformed_json() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("utf8_malformed_test.jsonl");

    // Create malformed JSON that will trigger the JSON parsing error path
    // with unicode characters that would cause panic if sliced unsafely at byte 100
    let malformed_entries = vec![
        // Invalid JSON with unicode chars positioned to cross 100-byte boundary
        r#"{"isApiErrorMessage":false,"parentUuid":"malformed-1","timestamp":"2024-01-01T12:00:00Z", "invalid": "This is göing tö bë a löng ërrör mëssägë with ünïcödë 🌈✨💫🎨🔥 that will cross boundaries", "missing_quote: true}"#,
        // Another malformed entry
        r#"{"parentUuid":"malformed-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123" "cwd":"/test/path","message":{"content":[{"text":"API Error: 500"}]}"#,
    ];

    let content = malformed_entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    // Enable debug logging to trigger the UTF-8 safety code paths
    let debug_logger = Some(Arc::new(get_debug_logger()));
    let monitor = JsonlMonitor::with_debug_logger(debug_logger);
    let result = monitor.scan_tail(&transcript_path).await;

    // Should succeed without panicking on UTF-8 boundaries in error logging
    assert!(
        result.is_ok(),
        "Should handle UTF-8 characters safely in malformed JSON error logging"
    );
    let (error_detected, _last_error) = result.unwrap();
    // Should not detect errors since JSON parsing failed
    assert!(
        !error_detected,
        "Should not detect errors in malformed JSON entries"
    );
}

/// Test UTF-8 safety with extreme unicode cases
#[tokio::test]
async fn test_utf8_safety_extreme_cases() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("utf8_extreme_test.jsonl");

    // Test with various unicode scenarios that could cause boundary issues
    let entries = vec![
        // Emoji at exact truncation boundaries
        r#"{"parentUuid":"extreme-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error: 429 🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨🚨"}]}}"#,
        // Mixed multibyte characters
        r#"{"isApiErrorMessage":false,"parentUuid":"extreme-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 - Ω∑π∆∫∂µ≈≤≥∞ Chinese: 你好世界 Japanese: こんにちは Arabic: مرحبا"}]}}"#,
        // Four-byte UTF-8 characters (mathematical symbols, etc.)
        r#"{"parentUuid":"extreme-3","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error: 503 Mathematical: 𝕏𝔸𝔹ℂ𝔻𝔼𝔽𝔾ℍ𝕀𝕁𝕂𝕃𝕄ℕ𝕆"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    // Enable debug logging to exercise UTF-8 safety paths
    let debug_logger = Some(Arc::new(get_debug_logger()));
    let monitor = JsonlMonitor::with_debug_logger(debug_logger);
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok(), "Should handle extreme UTF-8 cases safely");
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect API errors with extreme unicode"
    );
    assert!(last_error.is_some());
}

/// Test Phase 2 enhancement: whitespace tolerant API error detection
#[tokio::test]
async fn test_whitespace_tolerant_detection() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("whitespace_test.jsonl");

    // Test various whitespace combinations
    let entries = vec![
        // Multiple spaces
        r#"{"isApiErrorMessage":false,"parentUuid":"ws-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API   error: 429 Rate Limited"}]}}"#,
        // Tab character
        r#"{"parentUuid":"ws-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API\terror: 500 Server Error"}]}}"#,
        // Mixed whitespace (spaces and tabs)
        r#"{"isApiErrorMessage":false,"parentUuid":"ws-3","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API \t error: 502 Bad Gateway"}]}}"#,
        // NBSP (non-breaking space) - test this separately first
        r#"{"parentUuid":"ws-4","timestamp":"2024-01-01T12:03:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error: 503 Service Unavailable"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(
        result.is_ok(),
        "Should handle whitespace variations gracefully"
    );
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect API errors with various whitespace patterns"
    );
    assert!(last_error.is_some());

    // Should return the last error (503)
    let error = last_error.unwrap();
    assert_eq!(error.code, 503);
    assert_eq!(error.timestamp, "2024-01-01T12:03:00Z");
}

/// Test Phase 2 enhancement: colon-optional code extraction
#[tokio::test]
async fn test_colon_optional_code_extraction() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("colon_optional_test.jsonl");

    // Test API error patterns without colons
    let entries = vec![
        // Simple format: "API error 429 message"
        r#"{"isApiErrorMessage":false,"parentUuid":"co-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error 429 rate limited"}]}}"#,
        // With more text after code
        r#"{"parentUuid":"co-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error 500 internal server error occurred"}]}}"#,
        // Mixed case with whitespace variation
        r#"{"isApiErrorMessage":false,"parentUuid":"co-3","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"api  error 502 bad gateway response"}]}}"#,
        // Edge case: multiple numbers (should pick first valid HTTP code)
        r#"{"parentUuid":"co-4","timestamp":"2024-01-01T12:03:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error 503 happened at 1430 hours"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok(), "Should handle colon-optional patterns");
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected, "Should detect API errors without colons");
    assert!(last_error.is_some());

    // Should return the last error (503)
    let error = last_error.unwrap();
    assert_eq!(error.code, 503);
    assert_eq!(error.message, "Service Unavailable");
}

/// Test Phase 2 enhancement: edge cases for pattern matching improvements
#[tokio::test]
async fn test_pattern_matching_edge_cases() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("edge_cases_test.jsonl");

    // Test edge cases that should NOT be detected
    let non_matching_entries = vec![
        // Not starting with "api"
        r#"{"isApiErrorMessage":false,"parentUuid":"edge-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"The API error 429 was resolved"}]}}"#,
        // Missing "error" after whitespace
        r#"{"parentUuid":"edge-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API   call failed with 500"}]}}"#,
        // Invalid HTTP codes (out of range)
        r#"{"isApiErrorMessage":false,"parentUuid":"edge-3","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API error 99 invalid code"}]}}"#,
        r#"{"parentUuid":"edge-4","timestamp":"2024-01-01T12:03:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"api error 700 out of range"}]}}"#,
    ];

    // And one valid entry to ensure detection still works
    let valid_entry = r#"{"isApiErrorMessage":false,"parentUuid":"edge-5","timestamp":"2024-01-01T12:04:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API   error 404 not found"}]}}"#;

    let mut all_entries = non_matching_entries;
    all_entries.push(valid_entry);
    let content = all_entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok(), "Should handle edge cases gracefully");
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected, "Should detect only the valid API error");
    assert!(last_error.is_some());

    // Should return the valid error (404)
    let error = last_error.unwrap();
    assert_eq!(error.code, 404);
    assert_eq!(error.message, "API Error"); // No standard message for 404
}

/// Test Phase 2 combined: whitespace tolerance with colon-optional extraction
#[tokio::test]
async fn test_combined_phase2_enhancements() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("combined_test.jsonl");

    // Test combinations of both enhancements
    let entries = vec![
        // Colon-based with whitespace variations
        r#"{"isApiErrorMessage":false,"parentUuid":"comb-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API  error: 429"}]}}"#,
        // Colon-optional with whitespace variations
        r#"{"parentUuid":"comb-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"api\terror 500 server failure"}]}}"#,
        // Complex whitespace with colon-optional
        r#"{"isApiErrorMessage":false,"parentUuid":"comb-3","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API \t  error 502 gateway issues"}]}}"#,
        // NBSP with colon-optional
        r#"{"parentUuid":"comb-4","timestamp":"2024-01-01T12:03:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API\u{00a0}error 503 temporarily unavailable"}]}}"#,
    ];

    let content = entries.join("\n");
    fs::write(&transcript_path, content).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok(), "Should handle combined enhancements");
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected, "Should detect all enhanced patterns");
    assert!(last_error.is_some());

    // Should return the last error (503)
    let error = last_error.unwrap();
    assert_eq!(error.code, 503);
    assert_eq!(error.message, "Service Unavailable");
}

/// Test NBSP (non-breaking space) handling specifically
#[tokio::test]
async fn test_nbsp_whitespace_handling() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("nbsp_test.jsonl");

    // Test NBSP character specifically
    let entry = r#"{"isApiErrorMessage":false,"parentUuid":"nbsp-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API\u{00a0}error: 503 Service Unavailable"}]}}"#;

    fs::write(&transcript_path, entry).unwrap();

    let monitor = JsonlMonitor::new();
    let result = monitor.scan_tail(&transcript_path).await;

    assert!(result.is_ok(), "Should handle NBSP character gracefully");
    let (error_detected, last_error) = result.unwrap();
    assert!(
        error_detected,
        "Should detect API error with NBSP character"
    );
    assert!(last_error.is_some());

    let error = last_error.unwrap();
    assert_eq!(error.code, 503);
    assert_eq!(error.message, "Service Unavailable");
}
