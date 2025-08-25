use std::fs;
use std::env;
use std::io::Write;
use tempfile::tempdir;
use ccstatus::core::segments::network::JsonlMonitor;

// =============================================================================
// TAIL READING FUNCTIONALITY TESTS
// =============================================================================

#[tokio::test]
async fn test_tail_reading_small_file() {
    // Test that small files are read entirely
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("small_file.jsonl");
    
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"small-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    fs::write(&transcript_path, error_entry).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    assert_eq!(last_error.unwrap().code, 429);
}

#[tokio::test]
async fn test_tail_reading_large_file() {
    // Test that large files only read the tail portion
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("large_file.jsonl");
    
    // Create a file larger than default 64KB tail size
    let mut file = fs::File::create(&transcript_path).unwrap();
    
    // Write 100KB of normal entries (non-errors)
    let normal_entry = r#"{"isApiErrorMessage":false,"message":"Normal log entry","timestamp":"2024-01-01T10:00:00Z"}"#;
    for i in 0..2000 { // ~100KB of normal entries
        writeln!(file, "{}", normal_entry.replace("10:00:00Z", &format!("10:{:02}:{:02}Z", i / 60, i % 60))).unwrap();
    }
    
    // Add an error at the end (within tail range)
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"tail-error","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    writeln!(file, "{}", error_entry).unwrap();
    
    drop(file); // Close file
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected); // Should find the error in the tail
    assert!(last_error.is_some());
    assert_eq!(last_error.unwrap().code, 500);
}

#[tokio::test]
async fn test_tail_reading_with_custom_size() {
    // Test configurable tail size via environment variable
    env::set_var("CCSTATUS_JSONL_TAIL_KB", "1"); // Very small tail size
    
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("custom_tail.jsonl");
    
    let mut file = fs::File::create(&transcript_path).unwrap();
    
    // Write entries that exceed 1KB
    let large_entry = "x".repeat(500); // 500 chars
    for i in 0..10 {
        writeln!(file, r#"{{"message":"Large entry {}: {}","timestamp":"2024-01-01T11:{:02}:00Z"}}"#, i, large_entry, i).unwrap();
    }
    
    // Add error at the end
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"custom-tail","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    writeln!(file, "{}", error_entry).unwrap();
    
    drop(file);
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    
    env::remove_var("CCSTATUS_JSONL_TAIL_KB");
}

#[tokio::test]
async fn test_tail_reading_line_boundary_detection() {
    // Test that tail reading respects line boundaries
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("line_boundary.jsonl");
    
    let mut file = fs::File::create(&transcript_path).unwrap();
    
    // Create content that will test line boundary detection
    let normal_entry = r#"{"message":"Boundary test","timestamp":"2024-01-01T11:00:00Z"}"#;
    for i in 0..100 {
        writeln!(file, "{}", normal_entry.replace("11:00:00Z", &format!("11:{:02}:00Z", i))).unwrap();
    }
    
    // Add error entry that should be included in tail
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"boundary-test","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 502 Bad Gateway"}]}}"#;
    writeln!(file, "{}", error_entry).unwrap();
    
    drop(file);
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    assert_eq!(last_error.unwrap().code, 502);
}

// =============================================================================
// DEBUG MODE AND PERSISTENCE TESTS
// =============================================================================

#[tokio::test]
async fn test_debug_mode_aggregation() {
    env::set_var("CCSTATUS_DEBUG", "true");
    
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("debug_aggregation.jsonl");
    
    // Same parentUuid should be aggregated in debug mode
    let error1 = r#"{"isApiErrorMessage":true,"parentUuid":"debug-uuid","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    let error2 = r#"{"isApiErrorMessage":true,"parentUuid":"debug-uuid","timestamp":"2024-01-01T12:05:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    
    let content = format!("{}\n{}", error1, error2);
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, _) = result.unwrap();
    assert!(error_detected);
    
    // Check error statistics show aggregation in debug mode
    let stats = monitor.get_error_stats();
    assert_eq!(stats.total_unique_errors, 1); // Only one unique error
    assert_eq!(stats.total_occurrences, 2); // But two occurrences
    
    env::remove_var("CCSTATUS_DEBUG");
}

#[tokio::test]
async fn test_normal_mode_no_aggregation() {
    env::remove_var("CCSTATUS_DEBUG");
    
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("normal_no_agg.jsonl");
    
    let error1 = r#"{"isApiErrorMessage":true,"parentUuid":"normal-1","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 500 Server Error"}]}}"#;
    let error2 = r#"{"isApiErrorMessage":true,"parentUuid":"normal-2","timestamp":"2024-01-01T12:01:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    
    let content = format!("{}\n{}", error1, error2);
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected); // RED gate detection works
    assert!(last_error.is_some());
    assert_eq!(last_error.unwrap().code, 429); // Last error
    
    // No aggregation stats in normal mode
    let stats = monitor.get_error_stats();
    assert_eq!(stats.total_unique_errors, 0);
    assert_eq!(stats.total_occurrences, 0);
}

// =============================================================================
// ROBUSTNESS AND ERROR HANDLING TESTS
// =============================================================================

#[tokio::test]
async fn test_oversized_line_handling() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("oversized_lines.jsonl");
    
    let mut file = fs::File::create(&transcript_path).unwrap();
    
    // Create an oversized line (> 64KB)
    let huge_text = "x".repeat(70000); // 70KB line
    writeln!(file, r#"{{"message":"Huge line: {}","timestamp":"2024-01-01T11:00:00Z"}}"#, huge_text).unwrap();
    
    // Add normal error that should be processed
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"after-huge","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    writeln!(file, "{}", error_entry).unwrap();
    
    drop(file);
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    // Should handle oversized line gracefully and still find the valid error
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    assert_eq!(last_error.unwrap().code, 429);
}

#[tokio::test]
async fn test_malformed_json_resilience() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("malformed_json.jsonl");
    
    let content = r#"invalid json line 1
{"malformed": json without closing brace
{"isApiErrorMessage":false,"message":"Valid non-error"}
not json at all
{"isApiErrorMessage":true,"parentUuid":"valid-after-malformed","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 503 Service Unavailable"}]}}
another malformed { "json"
{"isApiErrorMessage":false,"message":"Another valid entry"}"#;
    
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    // Should skip malformed JSON and process valid error
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    assert_eq!(last_error.unwrap().code, 503);
}

// =============================================================================
// RED GATE CONTROL SPECIFIC TESTS
// =============================================================================

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
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    
    // Test explicit tuple binding for RED gate control
    let (error_detected, last_error_event) = result.unwrap();
    
    // error_detected should be true (triggers RED state)
    assert!(error_detected, "Should detect errors for RED gate control");
    
    // last_error_event should contain most recent error details
    assert!(last_error_event.is_some(), "Should provide last error event details");
    let error = last_error_event.unwrap();
    
    // Should be the LAST error (429), not the first (500)
    assert_eq!(error.code, 429, "Should return most recent error");
    assert_eq!(error.message, "Rate Limited", "Should extract correct message");
    assert_eq!(error.timestamp, "2024-01-01T12:02:00Z", "Should use transcript timestamp");
}

#[tokio::test]
async fn test_red_gate_no_errors_detected() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("no_errors.jsonl");
    
    let content = r#"{"isApiErrorMessage":false,"message":"Normal message 1","timestamp":"2024-01-01T11:00:00Z"}
{"message":"No error flag","timestamp":"2024-01-01T11:01:00Z"}
{"isApiErrorMessage":false,"message":"Normal message 2","timestamp":"2024-01-01T11:02:00Z"}"#;
    
    fs::write(&transcript_path, content).unwrap();
    
    let mut monitor = JsonlMonitor::new().unwrap();
    let result = monitor.scan_tail(&transcript_path).await;
    
    assert!(result.is_ok());
    let (error_detected, last_error_event) = result.unwrap();
    
    // No RED gate trigger
    assert!(!error_detected, "Should not detect errors when none present");
    assert!(last_error_event.is_none(), "Should not provide error details when none found");
}

#[tokio::test]
async fn test_environment_variable_configuration() {
    // Test various environment variable configurations
    
    // Test invalid tail size (should default to 64)
    env::set_var("CCSTATUS_JSONL_TAIL_KB", "invalid");
    let monitor1 = JsonlMonitor::new().unwrap();
    // Should handle invalid value gracefully
    
    // Test zero tail size (should default)
    env::set_var("CCSTATUS_JSONL_TAIL_KB", "0");
    let monitor2 = JsonlMonitor::new().unwrap();
    
    // Test very large tail size
    env::set_var("CCSTATUS_JSONL_TAIL_KB", "10000"); // 10MB
    let monitor3 = JsonlMonitor::new().unwrap();
    
    env::remove_var("CCSTATUS_JSONL_TAIL_KB");
    
    // All monitors should create successfully
    // (Full testing would require actual file operations which are covered in other tests)
}

// =============================================================================
// PERFORMANCE AND MEMORY TESTS
// =============================================================================

#[tokio::test]
async fn test_memory_efficiency_large_files() {
    let temp_dir = tempdir().unwrap();
    let transcript_path = temp_dir.path().join("memory_test.jsonl");
    
    // Create a very large file (several MB)
    let mut file = fs::File::create(&transcript_path).unwrap();
    
    let normal_entry = r#"{"isApiErrorMessage":false,"message":"Memory test entry with some content to make it reasonably sized","timestamp":"2024-01-01T10:00:00Z","sessionId":"memory-test","details":"This is additional content to increase the size of each JSON line for memory testing purposes."}"#;
    
    // Write ~5MB of content
    for i in 0..10000 {
        writeln!(file, "{}", normal_entry.replace("10:00:00Z", &format!("10:{:02}:{:02}Z", i / 3600, (i / 60) % 60))).unwrap();
    }
    
    // Add error at the end (within tail range)
    let error_entry = r#"{"isApiErrorMessage":true,"parentUuid":"memory-test-error","timestamp":"2024-01-01T12:00:00Z","sessionId":"session-123","cwd":"/test/path","message":{"content":[{"text":"API Error: 429 Rate Limited"}]}}"#;
    writeln!(file, "{}", error_entry).unwrap();
    
    drop(file);
    
    let mut monitor = JsonlMonitor::new().unwrap();
    
    // This should complete quickly and not consume excessive memory
    let start = std::time::Instant::now();
    let result = monitor.scan_tail(&transcript_path).await;
    let duration = start.elapsed();
    
    assert!(result.is_ok());
    let (error_detected, last_error) = result.unwrap();
    assert!(error_detected);
    assert!(last_error.is_some());
    
    // Should complete reasonably quickly (tail reading vs full file)
    assert!(duration.as_secs() < 5, "Tail reading should be fast even for large files");
}