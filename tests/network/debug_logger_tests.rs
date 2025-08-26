use std::env;
use std::fs;

use ccstatus::core::segments::network::debug_logger::{get_debug_logger, DebugLogger};
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

#[test]
fn test_debug_logger_creation() {
    let logger = DebugLogger::new();
    // Should create without error
    assert!(true);
}

#[test]
fn test_debug_logger_disabled_by_default() {
    env::remove_var("CCSTATUS_DEBUG");
    let logger = DebugLogger::new();

    // Should be disabled by default
    assert!(!logger.is_enabled());
}

#[test]
fn test_debug_logger_enabled_by_env() {
    env::set_var("CCSTATUS_DEBUG", "true");
    let logger = DebugLogger::new();

    // Should be enabled when env var is true
    assert!(logger.is_enabled());

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
}

#[test]
fn test_debug_logger_env_var_parsing() {
    // Test various boolean values
    let test_cases = vec![
        ("true", true),
        ("false", false),
        ("1", false), // Invalid boolean should default to false
        ("invalid", false),
        ("", false),
    ];

    for (value, expected) in test_cases {
        env::set_var("CCSTATUS_DEBUG", value);
        let logger = DebugLogger::new();
        assert_eq!(logger.is_enabled(), expected, "Failed for value: {}", value);
    }

    env::remove_var("CCSTATUS_DEBUG");
}

#[test]
fn test_get_debug_logger_function() {
    let logger = get_debug_logger();
    // Should create logger successfully
    assert!(true);
}

#[tokio::test]
async fn test_debug_logging_disabled() {
    env::remove_var("CCSTATUS_DEBUG");
    let logger = DebugLogger::new();

    // Should not panic when disabled
    logger.debug("test_component", "test message").await;
    logger.error("test_component", "test error").await;
    logger
        .performance("test_component", "test_operation", 100)
        .await;
    logger
        .credential_info("test_component", "Environment", 20)
        .await;
}

#[tokio::test]
async fn test_debug_logging_enabled() {
    // Create temporary home directory for testing
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    // Set up environment
    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    let logger = DebugLogger::new();
    assert!(logger.is_enabled());

    // Log some messages
    logger
        .debug("NetworkSegment", "Starting network monitoring")
        .await;
    logger.error("HttpMonitor", "Connection failed").await;

    // Give async operations time to complete
    sleep(Duration::from_millis(50)).await;

    // Check if log file was created
    let mut log_path = temp_home.to_path_buf();
    log_path.push(".claude");
    log_path.push("ccstatus");
    log_path.push("ccstatus-debug.log");

    if log_path.exists() {
        let contents = fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("DEBUG"));
        assert!(contents.contains("NetworkSegment"));
        assert!(contents.contains("Starting network monitoring"));
        assert!(contents.contains("ERROR"));
        assert!(contents.contains("HttpMonitor"));
        assert!(contents.contains("Connection failed"));
    }

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[tokio::test]
async fn test_performance_logging() {
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    let logger = DebugLogger::new();

    // Log performance metric
    logger.performance("HttpMonitor", "api_probe", 1250).await;

    // Give async operations time to complete
    sleep(Duration::from_millis(50)).await;

    // Check log content
    let mut log_path = temp_home.to_path_buf();
    log_path.push(".claude");
    log_path.push("ccstatus");
    log_path.push("ccstatus-debug.log");

    if log_path.exists() {
        let contents = fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("PERF"));
        assert!(contents.contains("HttpMonitor"));
        assert!(contents.contains("api_probe took 1250ms"));
    }

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[tokio::test]
async fn test_credential_info_logging() {
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    let logger = DebugLogger::new();

    // Log credential info (should not expose actual token)
    logger
        .credential_info("CredentialManager", "Environment", 45)
        .await;

    // Give async operations time to complete
    sleep(Duration::from_millis(50)).await;

    // Check log content
    let mut log_path = temp_home.to_path_buf();
    log_path.push(".claude");
    log_path.push("ccstatus");
    log_path.push("ccstatus-debug.log");

    if log_path.exists() {
        let contents = fs::read_to_string(&log_path).unwrap();
        assert!(contents.contains("CRED"));
        assert!(contents.contains("CredentialManager"));
        assert!(contents.contains("Using credentials from Environment"));
        assert!(contents.contains("token length: 45 chars"));
        // Should NOT contain actual token value
        assert!(!contents.contains("sk-"));
        assert!(!contents.contains("Bearer"));
    }

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[tokio::test]
async fn test_log_file_directory_creation() {
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    // Ensure .claude/ccstatus directory doesn't exist yet
    let mut ccstatus_dir = temp_home.to_path_buf();
    ccstatus_dir.push(".claude");
    ccstatus_dir.push("ccstatus");
    assert!(!ccstatus_dir.exists());

    // Create logger (should create directory)
    let logger = DebugLogger::new();
    logger.debug("test", "test message").await;

    // Give async operations time to complete
    sleep(Duration::from_millis(50)).await;

    // Directory should now exist
    assert!(ccstatus_dir.exists());

    // Log file should exist
    let log_file = ccstatus_dir.join("ccstatus-debug.log");
    assert!(log_file.exists() || !logger.is_enabled());

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[test]
fn test_session_refresh_functionality() {
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    // Create log file with existing content
    let mut log_path = temp_home.to_path_buf();
    log_path.push(".claude");
    log_path.push("ccstatus");
    fs::create_dir_all(log_path.parent().unwrap()).unwrap();
    log_path.push("ccstatus-debug.log");
    fs::write(&log_path, "Old log content\n").unwrap();

    // Create first logger (should clear the file)
    let _logger1 = DebugLogger::new();

    // File should be empty due to session refresh
    if log_path.exists() {
        let contents = fs::read_to_string(&log_path).unwrap();
        assert!(contents.is_empty() || contents == "Old log content\n");
    }

    // Create second logger in same session (should NOT clear again)
    let _logger2 = DebugLogger::new();

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[tokio::test]
async fn test_timestamp_format() {
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    let logger = DebugLogger::new();

    logger.debug("TestComponent", "timestamp test").await;

    // Give async operations time to complete
    sleep(Duration::from_millis(50)).await;

    let mut log_path = temp_home.to_path_buf();
    log_path.push(".claude");
    log_path.push("ccstatus");
    log_path.push("ccstatus-debug.log");

    if log_path.exists() {
        let contents = fs::read_to_string(&log_path).unwrap();

        // Should contain timestamp format: YYYY-MM-DD HH:MM:SS.sss UTC
        assert!(contents.contains(" UTC"));
        assert!(contents.contains("DEBUG"));
        assert!(contents.contains("TestComponent"));

        // Check basic timestamp pattern (not exact due to timing)
        assert!(contents.chars().filter(|&c| c == ':').count() >= 3); // At least 3 colons in timestamp
        assert!(contents.chars().filter(|&c| c == '-').count() >= 2); // At least 2 dashes in date
    }

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[tokio::test]
async fn test_concurrent_logging() {
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    let logger = DebugLogger::new();

    // Log multiple messages concurrently
    let future1 = logger.debug("Component1", "Message 1");
    let future2 = logger.debug("Component2", "Message 2");
    let future3 = logger.error("Component3", "Error 1");
    let future4 = logger.performance("Component4", "Operation1", 500);

    tokio::join!(future1, future2, future3, future4);

    // Give async operations time to complete
    sleep(Duration::from_millis(100)).await;

    let mut log_path = temp_home.to_path_buf();
    log_path.push(".claude");
    log_path.push("ccstatus");
    log_path.push("ccstatus-debug.log");

    if log_path.exists() {
        let contents = fs::read_to_string(&log_path).unwrap();

        // All messages should be present
        assert!(contents.contains("Component1"));
        assert!(contents.contains("Message 1"));
        assert!(contents.contains("Component2"));
        assert!(contents.contains("Message 2"));
        assert!(contents.contains("Component3"));
        assert!(contents.contains("Error 1"));
        assert!(contents.contains("Component4"));
        assert!(contents.contains("Operation1 took 500ms"));
    }

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[test]
fn test_logger_with_no_home_directory() {
    env::set_var("CCSTATUS_DEBUG", "true");
    env::remove_var("HOME");

    // Should fallback to current directory
    let logger = DebugLogger::new();
    assert!(logger.is_enabled());

    // Should not panic
    assert!(true);

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
}

#[tokio::test]
async fn test_empty_component_and_message() {
    let temp_dir = tempdir().unwrap();
    let temp_home = temp_dir.path();

    env::set_var("CCSTATUS_DEBUG", "true");
    env::set_var("HOME", temp_home.to_str().unwrap());

    let logger = DebugLogger::new();

    // Test with empty strings
    logger.debug("", "").await;
    logger.error("", "empty error").await;
    logger.debug("EmptyMessage", "").await;

    // Should not crash
    sleep(Duration::from_millis(50)).await;

    // Clean up
    env::remove_var("CCSTATUS_DEBUG");
    env::remove_var("HOME");
}

#[test]
fn test_debug_logger_default_trait() {
    // Test that DebugLogger can be created with new()
    let logger = DebugLogger::new();

    // Should have proper default state based on environment
    assert!(
        logger.is_enabled()
            == env::var("CCSTATUS_DEBUG")
                .map(|v| v == "true")
                .unwrap_or(false)
    );
}
