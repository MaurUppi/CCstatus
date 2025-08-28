use ccstatus::core::network::types::{NetworkMetrics, NetworkStatus};
use ccstatus::core::network::StatusRenderer;

#[test]
fn test_status_renderer_creation() {
    let renderer = StatusRenderer::new();
    assert!(true); // Just verify creation works
}

#[test]
fn test_healthy_status_rendering() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 150,
        breakdown: "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:120ms|Total:150ms".to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![100, 120, 150],
        p95_latency_ms: 145,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics);

    // Should show green circle and P95 latency
    assert!(result.starts_with("🟢"));
    assert!(result.contains("P95:145ms"));
}

#[test]
fn test_degraded_status_rendering() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 800,
        breakdown: "DNS:10ms|TCP:20ms|TLS:30ms|TTFB:740ms|Total:800ms".to_string(),
        last_http_status: 200,
        error_type: Some("HighLatency".to_string()),
        rolling_totals: vec![600, 700, 800],
        p95_latency_ms: 750,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should show yellow circle, P95, and breakdown (no error_type)
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:750ms"));
    assert!(result.contains("DNS:10ms|TCP:20ms|TLS:30ms|TTFB:740ms|Total:800ms"));
    // Should NOT contain error_type display
    assert!(!result.contains("err:"));
}

#[test]
fn test_degraded_rate_limit_rendering() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 200,
        breakdown: "Total:200ms".to_string(),
        last_http_status: 429,
        error_type: Some("RateLimit".to_string()),
        rolling_totals: vec![150, 180, 200],
        p95_latency_ms: 190,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should show yellow circle, P95, and breakdown (no error_type)
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:190ms"));
    assert!(result.contains("Total:200ms"));
    // Should NOT contain error_type display
    assert!(!result.contains("err:"));
}

#[test]
fn test_error_status_rendering() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 1500,
        breakdown: "Total:1500ms|Error:Timeout".to_string(),
        last_http_status: 500,
        error_type: Some("ServerError".to_string()),
        rolling_totals: vec![1200, 1300, 1500],
        p95_latency_ms: 1400,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics);

    // Should show red circle and breakdown (no P95, no error_type)
    assert!(result.starts_with("🔴"));
    assert!(result.contains("Total:1500ms|Error:Timeout"));
    // Should NOT contain error_type display or P95
    assert!(!result.contains("err:"));
    assert!(!result.contains("P95:"));
}

#[test]
fn test_error_timeout_rendering() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 3000,
        breakdown: "Total:3000ms".to_string(),
        last_http_status: 0, // Timeout
        error_type: None,
        rolling_totals: vec![2000, 2500, 3000],
        p95_latency_ms: 2800,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics);

    // Should show red circle and breakdown (no timeout indication)
    assert!(result.starts_with("🔴"));
    assert!(result.contains("Total:3000ms"));
    // Should NOT contain timeout indication
    assert!(!result.contains("timeout"));
}

#[test]
fn test_error_http_status_rendering() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 500,
        breakdown: "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:470ms|Total:500ms".to_string(),
        last_http_status: 404,
        error_type: Some("ClientError".to_string()),
        rolling_totals: vec![400, 450, 500],
        p95_latency_ms: 475,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics);

    // Should show red circle and breakdown (no error_type)
    assert!(result.starts_with("🔴"));
    assert!(result.contains("DNS:5ms|TCP:10ms|TLS:15ms|TTFB:470ms|Total:500ms"));
    // Should NOT contain error_type display
    assert!(!result.contains("err:"));
}

#[test]
fn test_unknown_status_rendering() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 0,
        breakdown: "".to_string(),
        last_http_status: 0,
        error_type: None,
        rolling_totals: vec![],
        p95_latency_ms: 0,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Unknown, &metrics);

    // Should show white circle and "Env vars NOT Found"
    assert_eq!(result, "⚪ Env vars NOT Found");
}

#[test]
fn test_empty_breakdown_handling() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 200,
        breakdown: "".to_string(), // Empty breakdown
        last_http_status: 200,
        error_type: Some("TestError".to_string()),
        rolling_totals: vec![180, 190, 200],
        p95_latency_ms: 195,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should handle empty breakdown gracefully (no error_type display)
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:195ms"));
    // Should NOT contain error_type display
    assert!(!result.contains("err:"));
    // Should not have breakdown in result since it's empty
}

#[test]
fn test_no_error_type_handling() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 300,
        breakdown: "Total:300ms".to_string(),
        last_http_status: 200,
        error_type: None, // No error type
        rolling_totals: vec![250, 275, 300],
        p95_latency_ms: 285,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should handle missing error type gracefully
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:285ms"));
    assert!(result.contains("Total:300ms"));
    // Should not contain "err:" since error_type is None
    assert!(!result.contains("err:"));
}

#[test]
fn test_edge_case_zero_p95() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 100,
        breakdown: "Total:100ms".to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![100],
        p95_latency_ms: 0, // Zero P95 (not enough samples)
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics);

    // Should show P95:N/A for zero P95 (insufficient samples)
    assert!(result.starts_with("🟢"));
    assert!(result.contains("P95:N/A"));
}

#[test]
fn test_very_high_latencies() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 9999,
        breakdown: "Total:9999ms".to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![8000, 9000, 9999],
        p95_latency_ms: 9500,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics);

    // Should handle high latencies correctly
    assert!(result.starts_with("🟢"));
    assert!(result.contains("P95:9500ms"));
}

#[test]
fn test_special_characters_in_error_type() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 500,
        breakdown: "Total:500ms".to_string(),
        last_http_status: 500,
        error_type: Some("Server-Error_With.Special&Chars".to_string()),
        rolling_totals: vec![400, 450, 500],
        p95_latency_ms: 475,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics);

    // Should handle special characters gracefully (no error_type display)
    assert!(result.starts_with("🔴"));
    // Should NOT contain error_type display
    assert!(!result.contains("err:"));
}

#[test]
fn test_long_breakdown_strings() {
    let renderer = StatusRenderer::new();

    let long_breakdown =
        "DNS:50ms|TCP:100ms|TLS:150ms|TTFB:1200ms|Processing:800ms|Transfer:200ms|Total:2500ms";

    let metrics = NetworkMetrics {
        latency_ms: 2500,
        breakdown: long_breakdown.to_string(),
        last_http_status: 200,
        error_type: Some("HighLatency".to_string()),
        rolling_totals: vec![2000, 2250, 2500],
        p95_latency_ms: 2400,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should handle long breakdown strings (may wrap to next line)
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:2400ms"));
    assert!(result.contains(long_breakdown));
    // Should NOT contain error_type display
    assert!(!result.contains("err:"));
}

#[test]
fn test_status_renderer_default() {
    // Test Default trait implementation
    let renderer = StatusRenderer::default();

    let metrics = NetworkMetrics::default();
    let result = renderer.render_status(&NetworkStatus::Unknown, &metrics);

    assert_eq!(result, "⚪ Env vars NOT Found");
}

#[test]
fn test_all_emoji_combinations() {
    let renderer = StatusRenderer::new();
    let metrics = NetworkMetrics::default();

    // Test all status emoji outputs
    let healthy = renderer.render_status(&NetworkStatus::Healthy, &metrics);
    let degraded = renderer.render_status(&NetworkStatus::Degraded, &metrics);
    let error = renderer.render_status(&NetworkStatus::Error, &metrics);
    let unknown = renderer.render_status(&NetworkStatus::Unknown, &metrics);

    assert!(healthy.starts_with("🟢"));
    assert!(degraded.starts_with("🟡"));
    assert!(error.starts_with("🔴"));
    assert!(unknown.starts_with("⚪"));

    // Each should be different
    assert_ne!(healthy, degraded);
    assert_ne!(degraded, error);
    assert_ne!(error, unknown);
    assert_ne!(unknown, healthy);
}

#[test]
fn test_line_wrapping_behavior() {
    let renderer = StatusRenderer::new();

    // Create a very long breakdown that should trigger line wrapping
    let long_breakdown = "DNS:50ms|TCP:100ms|TLS:150ms|TTFB:1200ms|Processing:800ms|Transfer:200ms|Authentication:300ms|Validation:400ms|Total:3200ms";

    let metrics = NetworkMetrics {
        latency_ms: 3200,
        breakdown: long_breakdown.to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![3000, 3100, 3200],
        p95_latency_ms: 3100,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should wrap to next line due to length (80+ chars)
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:3100ms"));
    assert!(result.contains(long_breakdown));
    // Should contain newline due to line wrapping
    assert!(result.contains("\n"));

    // Test error status line wrapping too
    let error_result = renderer.render_status(&NetworkStatus::Error, &metrics);
    assert!(error_result.starts_with("🔴"));
    assert!(error_result.contains(long_breakdown));
    assert!(error_result.contains("\n"));
}

#[test]
fn test_no_line_wrapping_for_short_content() {
    let renderer = StatusRenderer::new();

    let short_breakdown = "Total:150ms";

    let metrics = NetworkMetrics {
        latency_ms: 150,
        breakdown: short_breakdown.to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![120, 135, 150],
        p95_latency_ms: 145,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should NOT wrap - content should fit on one line
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:145ms"));
    assert!(result.contains(short_breakdown));
    // Should NOT contain newline
    assert!(!result.contains("\n"));
    // Should be on single line with space separator
    assert!(result.contains("🟡 P95:145ms Total:150ms"));
}

#[test]
fn test_zero_p95_in_degraded_status() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 200,
        breakdown: "Total:200ms".to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![200],
        p95_latency_ms: 0, // Zero P95 (insufficient samples)
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should show P95:N/A for degraded status with zero P95
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:N/A"));
    assert!(result.contains("Total:200ms"));
    assert!(!result.contains("err:"));
}

#[test]
fn test_empty_breakdown_in_error_status() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 1000,
        breakdown: "".to_string(), // Empty breakdown
        last_http_status: 500,
        error_type: None,
        rolling_totals: vec![900, 950, 1000],
        p95_latency_ms: 980,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics);

    // Should handle empty breakdown in error status
    assert!(result.starts_with("🔴"));
    // Should not contain any additional text beyond the emoji
    assert_eq!(result, "🔴");
}

// Proxy health check tests

#[test]
fn test_proxy_healthy_prefix_none() {
    let renderer = StatusRenderer::new();

    let mut metrics = NetworkMetrics {
        latency_ms: 150,
        breakdown: "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:120ms|Total:150ms".to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![100, 120, 150],
        p95_latency_ms: 145,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: None, // Official endpoint, no proxy health check
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics);

    // Should show normal status without proxy prefix
    assert_eq!(result, "🟢 P95:145ms");
    assert!(!result.contains(" | "));
}

#[test]
fn test_proxy_healthy_prefix_true() {
    let renderer = StatusRenderer::new();

    let mut metrics = NetworkMetrics {
        latency_ms: 150,
        breakdown: "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:120ms|Total:150ms".to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![100, 120, 150],
        p95_latency_ms: 145,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: Some(true), // Healthy proxy
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics);

    // Should show green proxy prefix + normal status
    assert_eq!(result, "🟢 | 🟢 P95:145ms");
    assert!(result.starts_with("🟢 | "));
}

#[test]
fn test_proxy_unhealthy_prefix_false() {
    let renderer = StatusRenderer::new();

    let mut metrics = NetworkMetrics {
        latency_ms: 800,
        breakdown: "DNS:10ms|TCP:20ms|TLS:30ms|TTFB:740ms|Total:800ms".to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![600, 700, 800],
        p95_latency_ms: 750,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: Some(false), // Unhealthy proxy
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should show red proxy prefix + degraded status
    assert!(result.starts_with("🔴 | 🟡"));
    assert!(result.contains("P95:750ms"));
    assert!(result.contains("DNS:10ms|TCP:20ms|TLS:30ms|TTFB:740ms|Total:800ms"));
}

#[test]
fn test_proxy_healthy_with_error_status() {
    let renderer = StatusRenderer::new();

    let mut metrics = NetworkMetrics {
        latency_ms: 1500,
        breakdown: "Total:1500ms|Error:Timeout".to_string(),
        last_http_status: 500,
        error_type: Some("ServerError".to_string()),
        rolling_totals: vec![1200, 1300, 1500],
        p95_latency_ms: 1400,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: Some(true), // Healthy proxy but main API erroring
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics);

    // Should show green proxy prefix + red error status
    assert_eq!(result, "🟢 | 🔴 Total:1500ms|Error:Timeout");
}

#[test]
fn test_proxy_unhealthy_with_unknown_status() {
    let renderer = StatusRenderer::new();

    let mut metrics = NetworkMetrics {
        latency_ms: 0,
        breakdown: "".to_string(),
        last_http_status: 0,
        error_type: None,
        rolling_totals: vec![],
        p95_latency_ms: 0,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: Some(false), // Unhealthy proxy
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Unknown, &metrics);

    // Should show red proxy prefix + unknown status
    assert_eq!(result, "🔴 | ⚪ Env vars NOT Found");
}

#[test]
fn test_proxy_health_with_line_wrapping() {
    let renderer = StatusRenderer::new();

    let long_breakdown = "DNS:50ms|TCP:100ms|TLS:150ms|TTFB:1200ms|Processing:800ms|Transfer:200ms|Authentication:300ms|Validation:400ms|Total:3200ms";

    let mut metrics = NetworkMetrics {
        latency_ms: 3200,
        breakdown: long_breakdown.to_string(),
        last_http_status: 200,
        error_type: None,
        rolling_totals: vec![3000, 3100, 3200],
        p95_latency_ms: 3100,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: Some(false), // Unhealthy proxy with long breakdown
        proxy_health_level: None,
        proxy_health_detail: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics);

    // Should show red proxy prefix and wrap due to length
    assert!(result.starts_with("🔴 | 🟡"));
    assert!(result.contains("P95:3100ms"));
    assert!(result.contains(long_breakdown));
    assert!(result.contains("\n")); // Should wrap to next line
}
