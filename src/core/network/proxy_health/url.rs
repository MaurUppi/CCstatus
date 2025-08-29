//! URL Construction Utilities for Proxy Health Checks
//!
//! Provides flexible URL construction strategies:
//! - Root-based: Extract scheme+host from base_url → scheme://host/health
//! - Path-based: Append /health to normalized base_url (current behavior)
//! - Fallback logic for improved success rates

use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum UrlError {
    #[error("Invalid URL format: {0}")]
    ParseError(#[from] url::ParseError),
    #[error("Missing host in URL")]
    MissingHost,
}

/// Build root-based health URL: scheme://host[:port]/health  
/// 
/// This is the recommended approach as proxy health endpoints are typically
/// exposed at the root level regardless of API path configuration.
///
/// # Examples
/// - `https://proxy.com/api/v1` → `https://proxy.com/health`
/// - `https://api.example.com:8080/claude` → `https://api.example.com:8080/health`
/// - `http://localhost:3000/proxy` → `http://localhost:3000/health`
///
/// # Arguments
/// * `base_url` - The base URL to extract root from
///
/// # Returns
/// * `Ok(String)` - Root-based health URL
/// * `Err(UrlError)` - Invalid URL or missing host
pub fn build_root_health_url(base_url: &str) -> Result<String, UrlError> {
    let url = Url::parse(base_url)?;
    
    let host = url.host_str().ok_or(UrlError::MissingHost)?;
    
    let mut root_url = format!("{}://{}", url.scheme(), host);
    
    // Include port if present and not default for scheme
    if let Some(port) = url.port() {
        let default_port = match url.scheme() {
            "http" => 80,
            "https" => 443,
            _ => 0, // Include port for non-standard schemes
        };
        if port != default_port {
            root_url.push_str(&format!(":{}", port));
        }
    }
    
    root_url.push_str("/health");
    Ok(root_url)
}

/// Build path-based health URL: normalize(base_url) + "/health"
/// 
/// This is the current behavior, maintained for backward compatibility.
/// Less reliable than root-based approach as it depends on API path structure.
///
/// # Examples  
/// - `https://proxy.com/api/v1/` → `https://proxy.com/api/v1/health`
/// - `https://api.example.com/claude` → `https://api.example.com/claude/health`
///
/// # Arguments
/// * `base_url` - The base URL to append /health to
///
/// # Returns
/// * Complete health check URL with path preserved
pub fn build_path_health_url(base_url: &str) -> String {
    let normalized = normalize_base_url(base_url);
    format!("{}/health", normalized)
}

/// Normalize base URL by trimming trailing slashes
///
/// # Arguments
/// * `base_url` - The base URL to normalize
///
/// # Returns  
/// * Normalized URL string without trailing slash
pub fn normalize_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}

/// Check if a base URL is the official Anthropic API endpoint
///
/// Performs case-insensitive comparison, ignoring trailing slashes.
/// Used to determine whether proxy health checks should be performed.
///
/// # Arguments
/// * `base_url` - The base URL to check
///
/// # Returns
/// * `true` if this is the official endpoint (skip proxy health check)
/// * `false` if this is a proxy endpoint (perform health check)
pub fn is_official_base_url(base_url: &str) -> bool {
    let normalized = normalize_base_url(base_url);
    normalized.eq_ignore_ascii_case("https://api.anthropic.com")
}

/// Extract base host from URL for redirect validation
/// 
/// Used to ensure redirect locations point to the same host for security.
///
/// # Arguments
/// * `url_str` - URL string to extract host from
///
/// # Returns
/// * `Ok(String)` - Host component (e.g., "api.example.com")
/// * `Err(UrlError)` - Invalid URL or missing host  
pub fn extract_host(url_str: &str) -> Result<String, UrlError> {
    let url = Url::parse(url_str)?;
    url.host_str()
        .map(|h| h.to_string())
        .ok_or(UrlError::MissingHost)
}

/// Build messages API endpoint with URL normalization support
/// 
/// Automatically handles URL normalization and appends the appropriate messages path.
/// Prevents duplication when base_url already ends with /v1 or /api/v1.
///
/// # Examples
/// - `https://api.anthropic.com` → `https://api.anthropic.com/v1/messages`
/// - `https://proxy.com/api` → `https://proxy.com/api/v1/messages`  
/// - `https://proxy.com/` → `https://proxy.com/v1/messages`
/// - `https://custom.com/api/v1/` → `https://custom.com/api/v1/messages` (avoids duplication)
/// - `https://proxy.com/v1` → `https://proxy.com/v1/messages` (avoids duplication)
///
/// # Arguments
/// * `base_url` - The base URL to build messages endpoint from
///
/// # Returns
/// * Complete messages API endpoint URL
pub fn build_messages_endpoint(base_url: &str) -> String {
    let normalized = normalize_base_url(base_url);
    
    // Check if path already ends with /v1 or /api/v1 to avoid duplication
    if normalized.ends_with("/v1") || normalized.ends_with("/api/v1") {
        format!("{}/messages", normalized)
    } else {
        format!("{}/v1/messages", normalized)
    }
}

