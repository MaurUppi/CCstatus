// Integration tests for NetworkSegment - stdin orchestration
use ccstatus::core::network::{NetworkSegment, ProbeMode, StatuslineInput, CostInfo};
use serde_json;
use std::fs;
use tempfile::TempDir;

/// Create test StatuslineInput with minimal required fields
fn create_test_input(session_id: &str, total_duration_ms: u64, transcript_path: &str) -> StatuslineInput {
    StatuslineInput {
        session_id: session_id.to_string(),
        transcript_path: transcript_path.to_string(),
        cwd: "/tmp".to_string(),
        model: serde_json::json!({"id": "claude-3-haiku", "display_name": "Haiku"}),
        workspace: serde_json::json!({"current_dir": "/tmp", "project_dir": "/tmp"}),
        version: "1.0.0".to_string(),
        output_style: serde_json::json!({"name": "default"}),
        cost: CostInfo {
            total_cost_usd: 0.001,
            total_duration_ms,
            total_api_duration_ms: 1000,
            total_lines_added: 0,
            total_lines_removed: 0,
        },
        exceeds_200k_tokens: false,
    }
}

#[tokio::test]
async fn test_window_calculation_cold() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test COLD window (duration < 5000ms)
    let input = create_test_input("session1", 3000, "/tmp/transcript.jsonl");
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(decision.is_cold_window);
    assert!(!decision.is_red_window);
    assert!(!decision.is_green_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Cold));
}

#[tokio::test]
async fn test_window_calculation_green() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test GREEN window (300s cycle, first 3s)
    let input = create_test_input("session1", 301000, "/tmp/transcript.jsonl"); // 301s = 1ms into green window
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(!decision.is_cold_window);
    assert!(!decision.is_red_window);
    assert!(decision.is_green_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Green));
}

#[tokio::test]
async fn test_window_calculation_red_timing_only() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Create empty transcript to ensure no error is detected
    let temp_transcript = temp_dir.path().join("transcript.jsonl");
    fs::write(&temp_transcript, "").unwrap();
    
    // Test RED timing window without error - should not trigger RED
    let input = create_test_input("session1", 10500, temp_transcript.to_str().unwrap()); // 10.5s = within RED timing but outside COLD
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(!decision.is_cold_window);
    assert!(!decision.is_red_window); // No error detected, so RED shouldn't trigger
    assert!(!decision.is_green_window);
    assert_eq!(decision.probe_mode, None);
}

#[tokio::test]
async fn test_window_calculation_red_with_error() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Create transcript with API error
    let temp_transcript = temp_dir.path().join("transcript.jsonl");
    let error_line = r#"{"timestamp": "2025-01-25T10:00:00Z", "message": {"content": [{"text": "API Error: 529 Overloaded"}]}, "isApiErrorMessage": true}"#;
    fs::write(&temp_transcript, error_line).unwrap();
    
    // Test RED window with error detected - should trigger RED  
    let input = create_test_input("session1", 10500, temp_transcript.to_str().unwrap()); // 10.5s = within RED window
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(!decision.is_cold_window);
    assert!(decision.is_red_window);
    assert!(!decision.is_green_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Red));
}

#[tokio::test]
async fn test_window_calculation_no_active() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test no active window (outside all windows)
    let input = create_test_input("session1", 305000, "/tmp/transcript.jsonl"); // 305s = outside green window
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(!decision.is_cold_window);
    assert!(!decision.is_red_window); 
    assert!(!decision.is_green_window);
    assert_eq!(decision.probe_mode, None);
}

#[tokio::test]
async fn test_cold_probe_deduplication() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    
    // Create initial state with session1 already having done COLD probe
    let initial_state = r#"{
        "status": "Healthy",
        "monitoring_enabled": true,
        "monitoring_state": {
            "last_cold_session_id": "session1",
            "last_cold_probe_at": "2025-01-25T10:00:00-08:00",
            "last_green_window_id": 0,
            "last_red_window_id": 0,
            "state": "Healthy"
        },
        "network": {
            "latency_ms": 1000,
            "breakdown": "Total:1000ms",
            "last_http_status": 200,
            "error_type": null,
            "rolling_totals": [1000],
            "p95_latency_ms": 1000
        },
        "timestamp": "2025-01-25T10:00:00-08:00"
    }"#;
    
    fs::write(&state_path, initial_state).unwrap();
    
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test COLD window with same session_id - should be skipped
    let input = create_test_input("session1", 3000, "/tmp/transcript.jsonl");
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(decision.is_cold_window);
    assert_eq!(decision.probe_mode, None); // Skipped due to deduplication
}

#[tokio::test]
async fn test_cold_probe_different_session() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    
    // Create initial state with session1 having done COLD probe
    let initial_state = r#"{
        "status": "Healthy",
        "monitoring_enabled": true,
        "monitoring_state": {
            "last_cold_session_id": "session1",
            "last_cold_probe_at": "2025-01-25T10:00:00-08:00",
            "last_green_window_id": 0,
            "last_red_window_id": 0,
            "state": "Healthy"
        },
        "network": {
            "latency_ms": 1000,
            "breakdown": "Total:1000ms",
            "last_http_status": 200,
            "error_type": null,
            "rolling_totals": [1000],
            "p95_latency_ms": 1000
        },
        "timestamp": "2025-01-25T10:00:00-08:00"
    }"#;
    
    fs::write(&state_path, initial_state).unwrap();
    
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test COLD window with different session_id - should execute
    let input = create_test_input("session2", 3000, "/tmp/transcript.jsonl");
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(decision.is_cold_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Cold)); // Should execute for new session
}

#[tokio::test]
async fn test_window_priority_cold_over_green() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test timing that would match both COLD and GREEN, COLD should win
    let input = create_test_input("session1", 2000, "/tmp/transcript.jsonl"); // 2s = COLD + GREEN window
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(decision.is_cold_window);
    assert!(!decision.is_green_window); // GREEN not evaluated when COLD active
    assert_eq!(decision.probe_mode, Some(ProbeMode::Cold));
}

#[tokio::test]
async fn test_custom_cold_window_env_var() {
    // Set custom COLD window to 10 seconds
    std::env::set_var("ccstatus_COLD_WINDOW_MS", "10000");
    
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("monitoring.json");
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test duration that would be outside default (5s) but inside custom (10s)
    let input = create_test_input("session1", 7000, "/tmp/transcript.jsonl");
    let decision = segment.calculate_window_decision(&input).await.unwrap();
    
    assert!(decision.is_cold_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Cold));
    
    // Clean up
    std::env::remove_var("ccstatus_COLD_WINDOW_MS");
}

#[test]
fn test_stdin_input_validation() {
    // Test valid input
    let valid_json = r#"{
        "session_id": "test123",
        "transcript_path": "/path/to/transcript.jsonl", 
        "cwd": "/tmp",
        "model": {"id": "claude-3-haiku", "display_name": "Haiku"},
        "workspace": {"current_dir": "/tmp", "project_dir": "/tmp"},
        "version": "1.0.0",
        "output_style": {"name": "default"},
        "cost": {
            "total_cost_usd": 0.001,
            "total_duration_ms": 5000,
            "total_api_duration_ms": 1000,
            "total_lines_added": 0,
            "total_lines_removed": 0
        },
        "exceeds_200k_tokens": false
    }"#;
    
    let input: StatuslineInput = serde_json::from_str(valid_json).unwrap();
    assert_eq!(input.session_id, "test123");
    assert_eq!(input.cost.total_duration_ms, 5000);
    
    // Test invalid input - missing session_id
    let invalid_json = r#"{"transcript_path": "/path", "cost": {"total_duration_ms": 1000}}"#;
    assert!(serde_json::from_str::<StatuslineInput>(invalid_json).is_err());
}

#[test]
fn test_cost_info_parsing() {
    let cost_json = r#"{
        "total_cost_usd": 0.0015,
        "total_duration_ms": 45000,
        "total_api_duration_ms": 2500,
        "total_lines_added": 15,
        "total_lines_removed": 3
    }"#;
    
    let cost: CostInfo = serde_json::from_str(cost_json).unwrap();
    assert_eq!(cost.total_duration_ms, 45000);
    assert_eq!(cost.total_api_duration_ms, 2500);
    assert_eq!(cost.total_lines_added, 15);
    assert_eq!(cost.total_lines_removed, 3);
}