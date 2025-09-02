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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Unknown, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
    let result = renderer.render_status(&NetworkStatus::Unknown, &metrics, None);

    assert_eq!(result, "⚪ Env vars NOT Found");
}

#[test]
fn test_all_emoji_combinations() {
    let renderer = StatusRenderer::new();
    let metrics = NetworkMetrics::default();

    // Test all status emoji outputs
    let healthy = renderer.render_status(&NetworkStatus::Healthy, &metrics, None);
    let degraded = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);
    let error = renderer.render_status(&NetworkStatus::Error, &metrics, None);
    let unknown = renderer.render_status(&NetworkStatus::Unknown, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

    // Should wrap to next line due to length (80+ chars)
    assert!(result.starts_with("🟡"));
    assert!(result.contains("P95:3100ms"));
    assert!(result.contains(long_breakdown));
    // Should contain newline due to line wrapping
    assert!(result.contains("\n"));

    // Test error status line wrapping too
    let error_result = renderer.render_status(&NetworkStatus::Error, &metrics, None);
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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Unknown, &metrics, None);

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Degraded, &metrics, None);

    // Should show red proxy prefix and wrap due to length
    assert!(result.starts_with("🔴 | 🟡"));
    assert!(result.contains("P95:3100ms"));
    assert!(result.contains(long_breakdown));
    assert!(result.contains("\n")); // Should wrap to next line
}

// Shield rendering edge case tests

#[test]
fn test_bot_challenge_both_blocked() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 1500,
        breakdown: "DNS:25ms|TCP:30ms|TLS:35ms|TTFB:1410ms|Total:1500ms".to_string(),
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![1200, 1400, 1500],
        p95_latency_ms: 1450,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    // Test the render_bot_challenge method directly through reflection or by triggering the right conditions
    // Since render_bot_challenge is private, we need to test it through the main render_status method
    // This would require error_type = "bot_challenge" and specific conditions to trigger shield rendering
    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Should contain bot challenge indicator (🛡️) and timing info
    assert!(result.contains("🛡️"));
}

#[test]
fn test_bot_challenge_get_only_with_zero_p95() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 800,
        breakdown: "DNS:25ms|TCP:30ms|TLS:35ms|TTFB:710ms|Total:800ms".to_string(),
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![800], // Only one sample, so P95 should be 0
        p95_latency_ms: 0,         // Zero P95 due to insufficient samples
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Should show bot challenge with P95:N/A when P95 is zero
    assert!(result.contains("🛡️"));
    // For direct testing, we'd need access to render_bot_challenge method
    // This test verifies the shield appears in error status with bot_challenge
}

#[test]
fn test_bot_challenge_post_only_high_latency() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 9999, // Very high latency
        breakdown: "DNS:100ms|TCP:200ms|TLS:300ms|TTFB:9399ms|Total:9999ms".to_string(),
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![8000, 9000, 9999],
        p95_latency_ms: 9500,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Should handle very high latencies in bot challenge scenarios
    assert!(result.contains("🛡️"));
    assert!(result.contains("9999ms")); // Should show the high latency
}

#[test]
fn test_bot_challenge_edge_case_empty_breakdown() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 1200,
        breakdown: "".to_string(), // Empty breakdown
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![1000, 1100, 1200],
        p95_latency_ms: 1150,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Should handle empty breakdown gracefully in bot challenge
    assert!(result.contains("🛡️"));
}

#[test]
fn test_bot_challenge_neither_blocked_fallback() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 500,
        breakdown: "Total:500ms".to_string(),
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![400, 450, 500],
        p95_latency_ms: 475,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Should show fallback bot challenge message
    assert!(result.contains("🛡️"));
}

#[test]
fn test_shield_rendering_with_proxy_health_combination() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 600,
        breakdown: "DNS:10ms|TCP:15ms|TLS:20ms|TTFB:555ms|Total:600ms".to_string(),
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![500, 550, 600],
        p95_latency_ms: 580,
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: Some(false), // Unhealthy proxy + bot challenge
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Should show bot challenge shield with POST-only pattern (Total: XXms)
    assert!(result.contains("🛡️")); // Bot challenge shield
    assert!(result.contains("Total: 600ms")); // POST bot challenge shows total time
    assert_eq!(result, "🛡️ Total: 600ms"); // Exact expected format for POST-only bot challenge
}

#[test]
fn test_shield_minimal_latency_values() {
    let renderer = StatusRenderer::new();

    let metrics = NetworkMetrics {
        latency_ms: 1, // Minimal latency
        breakdown: "Total:1ms".to_string(),
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![1],
        p95_latency_ms: 0, // Zero due to single sample
        connection_reused: None,
        breakdown_source: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Should handle minimal timing values
    assert!(result.contains("🛡️"));
    assert!(result.contains("1ms")); // Should show the minimal latency
}

#[test]
fn test_post_bot_challenge_breakdown_suppression_with_timings_curl() {
    let renderer = StatusRenderer::new();

    // Simulate detailed breakdown that would be generated by timings-curl feature
    let metrics = NetworkMetrics {
        latency_ms: 2500,
        // This detailed breakdown would normally be shown in error status
        breakdown: "DNS:50ms|TCP:75ms|TLS:100ms|TTFB:2275ms|Total:2500ms".to_string(),
        last_http_status: 429,
        error_type: Some("bot_challenge".to_string()),
        rolling_totals: vec![2000, 2200, 2500],
        p95_latency_ms: 2400,
        connection_reused: Some(false), // Not reused - would show full timing details
        breakdown_source: Some("measured".to_string()), // From timings-curl
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: Some("HTTP/2.0".to_string()),
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, None);

    // Verify breakdown suppression: should show shield with total time only
    assert!(result.contains("🛡️"));
    assert!(result.contains("Total: 2500ms"));

    // Critical: Verify detailed breakdown components are NOT shown
    assert!(
        !result.contains("DNS:50ms"),
        "DNS timing should be suppressed"
    );
    assert!(
        !result.contains("TCP:75ms"),
        "TCP timing should be suppressed"
    );
    assert!(
        !result.contains("TLS:100ms"),
        "TLS timing should be suppressed"
    );
    assert!(
        !result.contains("TTFB:2275ms"),
        "TTFB timing should be suppressed"
    );

    // Verify no individual timing components leak through
    assert!(!result.contains("DNS:"), "No DNS timing should appear");
    assert!(!result.contains("TCP:"), "No TCP timing should appear");
    assert!(!result.contains("TLS:"), "No TLS timing should appear");
    assert!(!result.contains("TTFB:"), "No TTFB timing should appear");

    // Should be exactly "🛡️ Total: 2500ms" format for POST-only bot challenge
    assert_eq!(
        result, "🛡️ Total: 2500ms",
        "POST bot challenge should show exactly '🛡️ Total: 2500ms', got: '{}'",
        result
    );
}

#[test]
fn test_oauth_mode_hides_status_lights_and_proxy_health() {
    use ccstatus::core::network::types::ApiConfig;
    let renderer = StatusRenderer::new();

    // Create OAuth API config
    let oauth_config = ApiConfig {
        endpoint: "https://api.anthropic.com/v1/messages".to_string(),
        source: "oauth".to_string(),
    };

    let metrics = NetworkMetrics {
        latency_ms: 150,
        breakdown: "DNS:5ms|TCP:10ms|TLS:15ms|TTFB:120ms|Total:150ms".to_string(),
        last_http_status: 401, // Expected for OAuth dummy key
        error_type: Some("authentication_error".to_string()),
        rolling_totals: vec![100, 120, 150],
        p95_latency_ms: 145,
        breakdown_source: Some("measured".to_string()),
        connection_reused: Some(false),
        proxy_healthy: None, // OAuth mode should have no proxy health
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: Some("HTTP/2.0".to_string()),
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, Some(&oauth_config));

    // OAuth mode should not show status lights (🟢/🟡/🔴) or proxy health prefix
    assert!(
        !result.contains("🟢"),
        "OAuth mode should not show green status light"
    );
    assert!(
        !result.contains("🟡"),
        "OAuth mode should not show yellow status light"
    );
    assert!(
        !result.contains("🔴"),
        "OAuth mode should not show red status light"
    );
    assert!(
        !result.contains("⚪"),
        "OAuth mode should not show unknown status light"
    );
    assert!(
        !result.contains(" | "),
        "OAuth mode should not show proxy health prefix"
    );

    // Should show timing metrics instead
    assert!(
        result.contains("P95:145ms"),
        "OAuth mode should show P95 metrics"
    );
    assert!(
        result.contains("DNS:5ms|TCP:10ms|TLS:15ms|TTFB:120ms|Total:150ms"),
        "OAuth mode should show timing breakdown"
    );
    assert!(
        result.contains("HTTP/2.0"),
        "OAuth mode should show HTTP version"
    );
}

#[test]
fn test_oauth_mode_with_minimal_metrics() {
    use ccstatus::core::network::types::ApiConfig;
    let renderer = StatusRenderer::new();

    // Create OAuth API config
    let oauth_config = ApiConfig {
        endpoint: "https://api.anthropic.com/v1/messages".to_string(),
        source: "oauth".to_string(),
    };

    // Minimal metrics (no P95, no breakdown, no HTTP version)
    let metrics = NetworkMetrics {
        latency_ms: 100,
        breakdown: "".to_string(),
        last_http_status: 401,
        error_type: Some("authentication_error".to_string()),
        rolling_totals: vec![],
        p95_latency_ms: 0,
        breakdown_source: None,
        connection_reused: None,
        proxy_healthy: None,
        proxy_health_level: None,
        proxy_health_detail: None,
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Error, &metrics, Some(&oauth_config));

    // Should fallback to "OAuth mode" when no metrics available
    assert_eq!(
        result, "OAuth mode",
        "Should show minimal OAuth mode text when no metrics available"
    );
}

#[test]
fn test_non_oauth_mode_unchanged() {
    use ccstatus::core::network::types::ApiConfig;
    let renderer = StatusRenderer::new();

    // Create non-OAuth API config
    let env_config = ApiConfig {
        endpoint: "https://api.anthropic.com/v1/messages".to_string(),
        source: "environment".to_string(),
    };

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
        http_version: None,
    };

    let result = renderer.render_status(&NetworkStatus::Healthy, &metrics, Some(&env_config));

    // Non-OAuth mode should show normal status lights and behavior
    assert!(
        result.contains("🟢"),
        "Non-OAuth mode should show green status light"
    );
    assert!(
        result.contains("P95:145ms"),
        "Non-OAuth mode should show P95 metrics"
    );

    // Compare with None config to ensure same behavior
    let result_none = renderer.render_status(&NetworkStatus::Healthy, &metrics, None);
    assert_eq!(
        result, result_none,
        "Non-OAuth config should behave same as None config"
    );
}
