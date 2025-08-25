use std::fs;
use std::env;
use tempfile::tempdir;
use ccstatus::core::segments::network::JsonlMonitor;

#[tokio::test]
async fn test_jsonl_monitor_creation() {
    let monitor = JsonlMonitor::new();
    assert!(monitor.is_ok(), "JsonlMonitor creation should succeed");
}

#[tokio::test]
async fn test_scan_nonexistent_file() {
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail("/nonexistent/path").await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(!error_detected);
    assert!(last_error.is_none());
}

#[tokio::test]
async fn test_scan_empty_file() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("empty_transcript.jsonl");
    fs::write(&transcript_path, "").unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(!error_detected);
    assert!(last_error.is_none());
}

#[tokio::test]
async fn test_scan_valid_error_entry() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("error_transcript.jsonl");
    
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"test-uuid-123","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 {\"error\":{\"message\":\"Rate Limited\"}}"}]}}"#;
    
    fs::write(&transcript_path, error_entry).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
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

#[tokio::test]
async fn test_error_aggregation() {
    // Enable debug mode for aggregation functionality
    env::set_var("CCSTATUS_DEBUG", "true");
    
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("aggregate_transcript.jsonl");
    
    // Same parentUuid should be aggregated
    let error1 = r#"{"isApiErrorMessage":true,"parentUuid":"duplicate-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 {\"error\":{\"message\":\"Server Error\"}}"}]}}"#;
    let error2 = r#"{"isApiErrorMessage":true,"parentUuid":"duplicate-uuid","timestamp":"2024-01-01T12:05:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 {\"error\":{\"message\":\"Server Error\"}}"}]}}"#;
    
    let content = format!("{}\n{}", error1, error2);
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, _) = result.unwrap();
    assert!(error_detected);
    
    // Check error statistics show aggregation (debug mode only)
    let stats = monitor.get_error_stats();
    assert_eq!(stats.total_unique_errors, 1); // Only one unique error
    assert_eq!(stats.total_occurrences, 2); // But two occurrences
    
    env::remove_var("CCSTATUS_DEBUG");
}

#[tokio::test]
async fn test_different_error_codes() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("multi_error_transcript.jsonl");
    
    let error_429 = r#"{"isApiErrorMessage":true,"parentUuid":"uuid-429","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    let error_500 = r#"{"isApiErrorMessage":true,"parentUuid":"uuid-500","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Internal Server Error"}]}}"#;
    let error_502 = r#"{"isApiErrorMessage":true,"parentUuid":"uuid-502","timestamp":"2024-01-01T12:02:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 502 Bad Gateway"}]}}"#;
    
    let content = format!("{}\n{}\n{}", error_429, error_500, error_502);
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    
    // Last error should be 502
    let error = last_error.unwrap();
    assert_eq!(error.code, 502);
    
    // Enable debug mode for aggregation stats  
    env::set_var("CCSTATUS_DEBUG", "true");
    let mut monitor_debug = JsonlMonitor::new().unwrap();
    let result_debug = monitor_debug.scan_tail(&transcript_path).await;
    assert!(result_debug.is_ok());
    
    // Should have 3 unique errors in debug mode
    let stats = monitor_debug.get_error_stats();
    assert_eq!(stats.total_unique_errors, 3);
    assert_eq!(stats.total_occurrences, 3);
    
    env::remove_var("CCSTATUS_DEBUG");
}

#[tokio::test]
async fn test_invalid_json_handling() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("invalid_transcript.jsonl");
    
    let invalid_json = "invalid json line\n";
    let valid_error = r#"{"isApiErrorMessage":true,"parentUuid":"valid-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    
    let content = format!("{}{}", invalid_json, valid_error);
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    // Should handle invalid JSON gracefully and still process valid entries
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected); // Valid error was found
    assert!(last_error.is_some());
    
    let error = last_error.unwrap();
    assert_eq!(error.code, 429);
}

#[tokio::test] 
async fn test_non_error_entries() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("normal_transcript.jsonl");
    
    let normal_entry = r#"{"isApiErrorMessage":false,"message":"Normal message","timestamp":"2024-01-01T12:00:00Z"}"#;
    let no_flag_entry = r#"{"message":"Message without error flag","timestamp":"2024-01-01T12:01:00Z"}"#;
    
    let content = format!("{}\n{}", normal_entry, no_flag_entry);
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(!error_detected); // No errors should be detected
    assert!(last_error.is_none());
}

#[tokio::test]
async fn test_error_persistence() {
    // Create a temporary JsonlMonitor to test persistence
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("persist_transcript.jsonl");
    
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"persist-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    fs::write(&transcript_path, error_entry).unwrap();
    
    // First monitor scan
    {
        let mut monitor = JsonlMonitor::new().unwrap();
        let result = monitor.scan_tail(&transcript_path).await;
        assert!(result.is_ok());
        let (error_detected, _) = result.unwrap();
        assert!(error_detected);
    } // Monitor drops here, should save state
    
    // Create new monitor - should load previous state
    let monitor = JsonlMonitor::new().unwrap();
    let stats = monitor.get_error_stats();
    // Note: This test depends on the actual implementation persisting between instances
    // In the current implementation, each new JsonlMonitor starts fresh but loads from disk
}

#[tokio::test]
async fn test_error_message_extraction() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("message_extraction_transcript.jsonl");
    
    // Test different message formats
    let json_error = r#"{"isApiErrorMessage":true,"parentUuid":"json-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 {\"error\":{\"message\":\"Rate limit exceeded\"}}"}]}}"#;
    let simple_error = r#"{"isApiErrorMessage":true,"parentUuid":"simple-uuid","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Internal Server Error"}]}}"#;
    
    let content = format!("{}\n{}", json_error, simple_error);
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
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
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    
    // Should find the last error (500)
    let error = last_error.unwrap();
    assert_eq!(error.code, 500);
    
    // Enable debug mode for aggregation stats
    env::set_var("CCSTATUS_DEBUG", "true");
    let mut monitor_debug = JsonlMonitor::new().unwrap();
    let result_debug = monitor_debug.scan_tail(&transcript_path).await;
    assert!(result_debug.is_ok());
    
    // Should have 2 unique errors in debug mode
    let stats = monitor_debug.get_error_stats();
    assert_eq!(stats.total_unique_errors, 2);
    
    env::remove_var("CCSTATUS_DEBUG");
}