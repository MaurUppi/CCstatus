// Integration tests for NetworkSegment - stdin orchestration
use ccstatus::core::network::{NetworkSegment, ProbeMode, StatuslineInput, CostInfo};
use serde_json::{self, json};
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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
    let decision = segment.calculate_window_decision(&input, None).await.unwrap();
    
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

// ================================
// NetworkSegment Enhancement Tests
// ================================

/// Create test StatuslineInput for enhancement tests with specified duration
fn create_enhancement_test_input(total_duration_ms: u64, session_id: &str) -> StatuslineInput {
    StatuslineInput {
        session_id: session_id.to_string(),
        transcript_path: "/tmp/test_transcript.jsonl".to_string(),
        cwd: "/tmp".to_string(),
        model: json!({"name": "claude-3"}),
        workspace: json!({"type": "test"}),
        version: "1.0.0".to_string(),
        output_style: json!({}),
        cost: CostInfo {
            total_cost_usd: 0.01,
            total_duration_ms,
            total_api_duration_ms: total_duration_ms / 2,
            total_lines_added: 10,
            total_lines_removed: 5,
        },
        exceeds_200k_tokens: false,
    }
}

/// Test Phase 1: Single JSONL Scan Elimination
#[tokio::test]
async fn test_phase1_single_jsonl_scan() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test_state.json");
    
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Create input in non-COLD path (> 5000ms)
    let input = create_enhancement_test_input(10_000, "test_session_1");
    
    // Test that error_detected parameter is properly passed
    let window_decision = segment.calculate_window_decision(&input, Some(true)).await.unwrap();
    
    // In RED window with error detected, should return RED mode
    assert!(window_decision.is_red_window);
    assert_eq!(window_decision.probe_mode, Some(ProbeMode::Red));
    
    // Test fallback to scanning when error_detected is None
    let window_decision_fallback = segment.calculate_window_decision(&input, None).await;
    
    // Should not fail even if transcript doesn't exist (JsonlMonitor handles this)
    assert!(window_decision_fallback.is_ok());
}

/// Test Phase 2: Per-Window Deduplication
#[tokio::test] 
async fn test_phase2_window_deduplication() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test_state.json");
    
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test GREEN window deduplication  
    // Window 1: duration 302_000ms = green_window_id 1, in GREEN window (2000ms < 3000ms)
    let input_green_1 = create_enhancement_test_input(302_000, "session_1");
    let decision_1 = segment.calculate_window_decision(&input_green_1, Some(false)).await.unwrap();
    
    assert!(decision_1.is_green_window);
    assert_eq!(decision_1.probe_mode, Some(ProbeMode::Green));
    assert_eq!(decision_1.green_window_id, Some(1)); // 302_000 / 300_000 = 1
    
    // Simulate state persistence (in real implementation, HttpMonitor would do this)
    // For testing, we verify window ID calculation is correct
    
    // Test RED window deduplication  
    // Window 1: duration 10_500ms = red_window_id 1, in RED window (500ms into 10s window)
    let input_red_1 = create_enhancement_test_input(10_500, "session_2");
    let decision_2 = segment.calculate_window_decision(&input_red_1, Some(true)).await.unwrap();
    
    assert!(decision_2.is_red_window);
    assert_eq!(decision_2.probe_mode, Some(ProbeMode::Red));
    assert_eq!(decision_2.red_window_id, Some(1)); // 10_500 / 10_000 = 1
    
    // Test window boundary calculations
    let input_green_2 = create_enhancement_test_input(300_000, "session_3"); // Exactly on boundary
    let decision_3 = segment.calculate_window_decision(&input_green_2, Some(false)).await.unwrap();
    
    // Should be GREEN window ID 1 (300_000 / 300_000 = 1) and IS in timing window
    assert!(decision_3.is_green_window); // 300_000 % 300_000 = 0, and 0 < 3000
    assert_eq!(decision_3.probe_mode, Some(ProbeMode::Green));
    assert_eq!(decision_3.green_window_id, Some(1)); // 300_000 / 300_000 = 1
}

/// Test Phase 3: COLD State Validation
#[tokio::test]
async fn test_phase3_cold_state_validation() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test_state.json");
    
    let segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test COLD window with no existing state (should NOT skip)
    let should_skip_1 = segment.should_skip_cold_probe("new_session").await.unwrap();
    assert!(!should_skip_1); // Don't skip - no valid state exists
    
    // Test COLD window timing condition
    let input_cold = create_enhancement_test_input(3_000, "session_cold"); // < 5000ms default
    let mut segment_mut = NetworkSegment::with_state_path(temp_dir.path().join("test_state2.json")).unwrap();
    let decision = segment_mut.calculate_window_decision(&input_cold, None).await.unwrap();
    
    assert!(decision.is_cold_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Cold));
    
    // Test non-COLD window
    let input_non_cold = create_enhancement_test_input(10_000, "session_non_cold"); // >= 5000ms
    let decision_non_cold = segment_mut.calculate_window_decision(&input_non_cold, Some(false)).await.unwrap();
    
    assert!(!decision_non_cold.is_cold_window);
}

/// Test window priority: COLD > RED > GREEN
#[tokio::test]
async fn test_enhancement_window_priority() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test_state.json");
    
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // COLD takes priority (duration < 5000ms)
    let input_cold = create_enhancement_test_input(2_000, "priority_session");
    let decision = segment.calculate_window_decision(&input_cold, Some(true)).await.unwrap();
    
    assert!(decision.is_cold_window);
    assert!(!decision.is_red_window);
    assert!(!decision.is_green_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Cold));
    
    // RED takes priority over GREEN when both conditions met
    // Choose timing that satisfies both RED and GREEN windows
    let input_both = create_enhancement_test_input(300_500, "priority_session_2"); // Both conditions
    // 300_500 % 10_000 = 500 < 1_000 (RED condition)
    // 300_500 % 300_000 = 500 < 3_000 (GREEN condition)
    let decision_both = segment.calculate_window_decision(&input_both, Some(true)).await.unwrap();
    
    assert!(!decision_both.is_cold_window);
    assert!(decision_both.is_red_window);
    assert!(!decision_both.is_green_window); // RED takes priority
    assert_eq!(decision_both.probe_mode, Some(ProbeMode::Red));
}

/// Test WindowDecision struct enhancements
#[tokio::test]
async fn test_window_decision_struct() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test_state.json");
    
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test that WindowDecision includes window IDs
    let input = create_enhancement_test_input(610_500, "test_session"); // Complex timing
    // GREEN: 610_500 / 300_000 = 2, 610_500 % 300_000 = 10_500 < 3_000 ✓
    // RED: 610_500 / 10_000 = 61, 610_500 % 10_000 = 500 < 1_000 ✓
    
    let decision = segment.calculate_window_decision(&input, Some(true)).await.unwrap();
    
    // RED should win priority
    assert!(decision.is_red_window);
    assert_eq!(decision.probe_mode, Some(ProbeMode::Red));
    assert_eq!(decision.red_window_id, Some(61));
    assert_eq!(decision.green_window_id, None); // Not set when RED active
    
    // Test GREEN window alone
    let input_green_only = create_enhancement_test_input(601_500, "test_session_2");
    // GREEN: 601_500 % 300_000 = 1_500 < 3_000 ✓  
    // RED: 601_500 % 10_000 = 1_500 NOT < 1_000 ✗
    
    let decision_green = segment.calculate_window_decision(&input_green_only, Some(false)).await.unwrap();
    
    assert!(!decision_green.is_red_window);
    assert!(decision_green.is_green_window);
    assert_eq!(decision_green.probe_mode, Some(ProbeMode::Green));
    assert_eq!(decision_green.green_window_id, Some(2)); // 601_500 / 300_000 = 2
    assert_eq!(decision_green.red_window_id, None);
}

/// Test error handling and edge cases
#[tokio::test]
async fn test_enhancement_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let state_path = temp_dir.path().join("test_state.json");
    
    let mut segment = NetworkSegment::with_state_path(state_path).unwrap();
    
    // Test with zero duration (edge case)
    let input_zero = create_enhancement_test_input(0, "zero_session");
    let decision = segment.calculate_window_decision(&input_zero, None).await.unwrap();
    
    assert!(decision.is_cold_window); // 0 < 5000
    assert_eq!(decision.probe_mode, Some(ProbeMode::Cold));
    
    // Test with very large duration
    let input_large = create_enhancement_test_input(u64::MAX, "large_session");
    let decision_large = segment.calculate_window_decision(&input_large, Some(false)).await.unwrap();
    
    // Should not be in any active window due to modulo calculations
    assert!(!decision_large.is_cold_window);
    assert!(!decision_large.is_red_window);  
    assert!(!decision_large.is_green_window);
    assert_eq!(decision_large.probe_mode, None);
}