/*! 
Tests for proxy health JSON parsing module.

Extracted from src/core/network/proxy_health/parsing.rs #[cfg(test)] module.
Tests the health response parsing logic including tri-state level parsing,
legacy validation, and Cloudflare challenge detection.
*/

use ccstatus::core::network::proxy_health::config::ProxyHealthLevel;
use ccstatus::core::network::proxy_health::parsing::{parse_health_response, validate_health_json};

#[test]
fn test_parse_status_field() {
    // Healthy variants
    assert_eq!(
        parse_health_response(br#"{"status": "healthy"}"#),
        Some(ProxyHealthLevel::Healthy)
    );
    
    assert_eq!(
        parse_health_response(br#"{"STATUS": "OK"}"#),
        Some(ProxyHealthLevel::Healthy)
    );
    
    // Degraded variants
    assert_eq!(
        parse_health_response(br#"{"status": "unhealthy"}"#),
        Some(ProxyHealthLevel::Degraded)
    );
    
    assert_eq!(
        parse_health_response(br#"{"status": "warning"}"#),
        Some(ProxyHealthLevel::Degraded)
    );
    
    // Bad variants
    assert_eq!(
        parse_health_response(br#"{"status": "error"}"#),
        Some(ProxyHealthLevel::Bad)
    );
    
    assert_eq!(
        parse_health_response(br#"{"status": "down"}"#),
        Some(ProxyHealthLevel::Bad)
    );
}

#[test]
fn test_parse_healthy_field() {
    assert_eq!(
        parse_health_response(br#"{"healthy": true}"#),
        Some(ProxyHealthLevel::Healthy)
    );
    
    assert_eq!(
        parse_health_response(br#"{"HEALTHY": false}"#),
        Some(ProxyHealthLevel::Degraded)
    );
}

#[test]
fn test_parse_mixed_schema() {
    // Component-based health
    let components_json = br#"{
        "status": "healthy",
        "components": {
            "redis": {"status": "healthy"},
            "database": {"status": "healthy"}
        }
    }"#;
    assert_eq!(
        parse_health_response(components_json),
        Some(ProxyHealthLevel::Healthy)
    );
    
    // Error indicators
    assert_eq!(
        parse_health_response(br#"{"error": "connection failed"}"#),
        Some(ProxyHealthLevel::Bad)
    );
}

#[test]
fn test_invalid_cases() {
    // Invalid JSON
    assert_eq!(
        parse_health_response(b"not json"),
        Some(ProxyHealthLevel::Bad)
    );
    
    // Empty response  
    assert_eq!(parse_health_response(b""), None);
    
    // Whitespace only
    assert_eq!(parse_health_response(b"   \n\t  "), None);
    
    // Not an object
    assert_eq!(
        parse_health_response(br#""healthy""#),
        Some(ProxyHealthLevel::Bad)
    );
    
    // Unknown schema
    assert_eq!(
        parse_health_response(br#"{"foo": "bar"}"#),
        Some(ProxyHealthLevel::Bad)
    );
}

#[test]
fn test_validate_health_json_legacy() {
    // Legacy function should only accept status="healthy"
    assert!(validate_health_json(br#"{"status": "healthy"}"#));
    assert!(validate_health_json(br#"{"STATUS": "HEALTHY"}"#));
    
    assert!(!validate_health_json(br#"{"status": "ok"}"#));
    assert!(!validate_health_json(br#"{"healthy": true}"#));
    assert!(!validate_health_json(br#"{"status": "unhealthy"}"#));
    assert!(!validate_health_json(b"invalid json"));
}