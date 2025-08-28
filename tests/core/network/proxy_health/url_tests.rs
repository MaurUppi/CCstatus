/*!
Tests for proxy health URL construction module.

Extracted from src/core/network/proxy_health/url.rs #[cfg(test)] module.
Tests the URL construction utilities including root-based health URLs,
path-based health URLs, normalization, and official endpoint detection.
*/

use ccstatus::core::network::proxy_health::url::{
    build_root_health_url, build_path_health_url, normalize_base_url,
    is_official_base_url, extract_host
};

#[test]
fn test_build_root_health_url() {
    // Basic cases
    assert_eq!(
        build_root_health_url("https://proxy.com/api/v1").unwrap(),
        "https://proxy.com/health"
    );
    
    assert_eq!(
        build_root_health_url("http://localhost:3000/claude").unwrap(),
        "http://localhost:3000/health"
    );
    
    // Custom ports
    assert_eq!(
        build_root_health_url("https://api.example.com:8443/path").unwrap(),
        "https://api.example.com:8443/health"
    );
    
    // Default ports (should be omitted)
    assert_eq!(
        build_root_health_url("https://proxy.com:443/api").unwrap(),
        "https://proxy.com/health"
    );
    
    assert_eq!(
        build_root_health_url("http://proxy.com:80/api").unwrap(),
        "http://proxy.com/health"
    );
}

#[test]
fn test_build_path_health_url() {
    assert_eq!(
        build_path_health_url("https://proxy.com/api/v1/"),
        "https://proxy.com/api/v1/health"
    );
    
    assert_eq!(
        build_path_health_url("https://api.example.com/claude"),
        "https://api.example.com/claude/health"
    );
}

#[test]
fn test_normalize_base_url() {
    assert_eq!(normalize_base_url("https://api.com/"), "https://api.com");
    assert_eq!(normalize_base_url("https://api.com"), "https://api.com");
    assert_eq!(normalize_base_url("https://api.com///"), "https://api.com");
}

#[test]
fn test_is_official_base_url() {
    assert!(is_official_base_url("https://api.anthropic.com"));
    assert!(is_official_base_url("https://api.anthropic.com/"));
    assert!(is_official_base_url("HTTPS://API.ANTHROPIC.COM"));
    
    assert!(!is_official_base_url("https://proxy.com/api"));
    assert!(!is_official_base_url("https://api.anthropic.com.evil.com"));
}

#[test]  
fn test_extract_host() {
    assert_eq!(extract_host("https://proxy.com/path").unwrap(), "proxy.com");
    assert_eq!(extract_host("http://api.example.com:8080").unwrap(), "api.example.com");
    
    assert!(extract_host("not-a-url").is_err());
}