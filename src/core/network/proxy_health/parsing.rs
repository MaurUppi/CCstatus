//! Health Check JSON Response Parsing
//!
//! Provides tri-state health level parsing with backward compatibility.
//! Supports various JSON health schemas commonly used by proxy services.

use crate::core::network::proxy_health::config::ProxyHealthLevel;
use serde_json::Value;
use std::collections::HashMap;

/// Parse health check response body to determine proxy health level
///
/// Supports multiple JSON schema patterns:
/// - `{"status": "healthy"}` → Healthy
/// - `{"status": "unhealthy"}` → Degraded  
/// - `{"status": "error"}` → Bad
/// - `{"healthy": true}` → Healthy
/// - `{"healthy": false}` → Degraded
/// - Invalid JSON or unknown schema → Bad
///
/// All field names and string values are case-insensitive.
///
/// # Arguments
/// * `body` - Response body bytes to parse
///
/// # Returns
/// * `Some(ProxyHealthLevel)` - Successfully parsed health level
/// * `None` - Body is empty or whitespace-only (treat as no endpoint)
pub fn parse_health_response(body: &[u8]) -> Option<ProxyHealthLevel> {
    // Handle empty/whitespace-only responses
    if body.is_empty() || body.iter().all(|&b| b.is_ascii_whitespace()) {
        return None;
    }

    // Parse JSON response
    let json_value: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return Some(ProxyHealthLevel::Bad), // Invalid JSON
    };

    // Must be a JSON object
    let obj = match json_value.as_object() {
        Some(obj) => obj,
        None => return Some(ProxyHealthLevel::Bad), // Not an object
    };

    // Try parsing different schema patterns

    // Pattern 1: status field (string)
    if let Some(status_level) = parse_status_field(obj) {
        return Some(status_level);
    }

    // Pattern 2: healthy field (boolean)
    if let Some(healthy_level) = parse_healthy_field(obj) {
        return Some(healthy_level);
    }

    // Pattern 3: Mixed patterns or complex schemas
    if let Some(mixed_level) = parse_mixed_schema(obj) {
        return Some(mixed_level);
    }

    // Unknown schema - default to Bad for safety
    Some(ProxyHealthLevel::Bad)
}

/// Parse status field: {"status": "healthy|unhealthy|error|down|fail|..."}
fn parse_status_field(obj: &serde_json::Map<String, Value>) -> Option<ProxyHealthLevel> {
    // Find status field (case-insensitive)
    let status_value = obj
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("status"))
        .map(|(_, value)| value)?;

    let status_str = status_value.as_str()?;

    match status_str.to_ascii_lowercase().as_str() {
        "healthy" | "ok" | "up" | "running" => Some(ProxyHealthLevel::Healthy),
        "unhealthy" | "degraded" | "warning" => Some(ProxyHealthLevel::Degraded),
        "error" | "down" | "fail" | "failed" | "critical" | "offline" => {
            Some(ProxyHealthLevel::Bad)
        }
        _ => None, // Unknown status value, try other patterns
    }
}

/// Parse healthy field: {"healthy": true|false}
fn parse_healthy_field(obj: &serde_json::Map<String, Value>) -> Option<ProxyHealthLevel> {
    // Find healthy field (case-insensitive)
    let healthy_value = obj
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("healthy"))
        .map(|(_, value)| value)?;

    match healthy_value.as_bool()? {
        true => Some(ProxyHealthLevel::Healthy),
        false => Some(ProxyHealthLevel::Degraded), // Unhealthy but responding
    }
}

/// Parse mixed/complex schemas with multiple fields
fn parse_mixed_schema(obj: &serde_json::Map<String, Value>) -> Option<ProxyHealthLevel> {
    // Check for component-based health (common in microservices)
    if let Some(components) = obj.get("components").and_then(|c| c.as_object()) {
        let all_healthy = components.values().all(|component| {
            component
                .as_object()
                .and_then(|c| c.get("status"))
                .and_then(|s| s.as_str())
                .map(|status| status.eq_ignore_ascii_case("healthy"))
                .unwrap_or(false)
        });

        if all_healthy {
            return Some(ProxyHealthLevel::Healthy);
        } else {
            return Some(ProxyHealthLevel::Degraded);
        }
    }

    // Check for error conditions in various fields
    let error_indicators = ["error", "errors", "failure", "failures"];
    for indicator in &error_indicators {
        if obj.contains_key(*indicator) {
            return Some(ProxyHealthLevel::Bad);
        }
    }

    None // No recognizable pattern
}

/// Legacy validation function for backward compatibility
///
/// Only checks for `status="healthy"` (case-insensitive), maintaining
/// exact behavior of existing validate_health_json function.
///
/// # Arguments
/// * `body` - Response body bytes to validate
///
/// # Returns  
/// * `true` - Valid JSON with status=healthy (case-insensitive)
/// * `false` - Invalid JSON, missing status field, or status!=healthy
pub fn validate_health_json(body: &[u8]) -> bool {
    // Parse JSON response
    let json_value: Value = match serde_json::from_slice(body) {
        Ok(value) => value,
        Err(_) => return false, // Invalid JSON
    };

    // Check if it's an object with a status field
    let obj = match json_value.as_object() {
        Some(obj) => obj,
        None => return false, // Not a JSON object
    };

    // Get status field value (case-insensitive field name)
    let status = obj
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("status"))
        .map(|(_, value)| value);

    let status = match status {
        Some(status) => status,
        None => return false, // Missing status field
    };

    // Check if status is "healthy" (case-insensitive)
    match status.as_str() {
        Some(status_str) => status_str.eq_ignore_ascii_case("healthy"),
        None => false, // Status is not a string
    }
}

/// Detect Cloudflare challenge responses that indicate Unknown proxy health status
///
/// Enhanced Phase 2 detection for Cloudflare challenges:
/// 1. HTTP 403/503/429 status codes with Cloudflare-specific headers
/// 2. Headers: cf-mitigated: challenge (high-confidence), cf-ray, server: cloudflare, CF set-cookies
/// 3. Body content: "Just a moment", "Enable JavaScript and cookies to continue", /cdn-cgi/challenge-platform
///
/// # Arguments
/// * `status_code` - HTTP response status code
/// * `headers` - Case-insensitive HTTP response headers map
/// * `body` - Response body content to check for challenge markers
///
/// # Returns
/// * `true` - Cloudflare challenge detected (proxy status Unknown)
/// * `false` - No Cloudflare challenge indicators found
pub fn detect_cloudflare_challenge(
    status_code: u16,
    headers: &HashMap<String, String>,
    body: &[u8],
) -> bool {
    // First check: Status codes commonly used by CF challenges
    if !matches!(status_code, 403 | 503 | 429) {
        return false;
    }

    // Second check: Cloudflare-specific headers (case-insensitive)
    let has_cf_headers = headers.iter().any(|(key, value)| {
        let key_lower = key.to_lowercase();
        let value_lower = value.to_lowercase();
        match key_lower.as_str() {
            "cf-ray" => true,                                    // Cloudflare request ID
            "cf-mitigated" => value_lower.contains("challenge"), // Active challenge (high-confidence)
            "server" => value_lower.contains("cloudflare"),      // CF server header
            "cf-cache-status" => true,                           // Any CF cache status
            "set-cookie" => {
                // Cloudflare-specific cookies indicating bot mitigation
                value_lower.contains("cf_clearance")
                    || value_lower.contains("__cf_bm")
                    || value_lower.contains("cf_chl_jschl_tk")
            }
            _ => false,
        }
    });

    if has_cf_headers {
        return true;
    }

    // Third check: HTML body content with challenge markers
    if let Ok(body_str) = std::str::from_utf8(body) {
        let body_lower = body_str.to_lowercase();
        let challenge_markers = [
            "just a moment",
            "enable javascript and cookies to continue", // Phase 2 marker
            "checking your browser",
            "cloudflare security",
            "cf-browser-verification",
            "challenge-running",
            "cf-challenge-form",
            "please wait while we verify",
            "ddos protection by cloudflare",
            "/cdn-cgi/challenge-platform", // Phase 2 script marker
            "browser security check",
            "ray id:", // Common in CF error pages
        ];

        for marker in &challenge_markers {
            if body_lower.contains(marker) {
                return true;
            }
        }
    }

    false
}
