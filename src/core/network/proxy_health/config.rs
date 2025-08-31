//! Proxy Health Configuration and Data Types

use serde::{Deserialize, Serialize};

/// Tri-state proxy health levels for enhanced status granularity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyHealthLevel {
    /// Proxy is fully operational (🟢)
    /// - JSON status="healthy" (case-insensitive) OR healthy=true
    Healthy,

    /// Proxy is operational but degraded (🟡)  
    /// - JSON status="unhealthy" OR healthy=false
    Degraded,

    /// Proxy is in error state (🔴)
    /// - JSON status="error"|"down"|"fail" OR invalid JSON schema
    Bad,

    /// Proxy status cannot be determined (⚪)
    /// - Cloudflare challenges (cf-mitigated, "Just a moment...")
    /// - Network errors preventing detection
    /// - Authentication required responses (401/403 without CF indicators)
    Unknown,
}

/// Proxy health check configuration options
#[derive(Debug, Clone)]
pub struct ProxyHealthOptions {
    /// Use root-based URL construction (scheme://host/health) instead of path-based
    /// Default: false (maintain compatibility with existing behavior)
    pub use_root_urls: bool,

    /// Try path-based URL as fallback when root-based returns 404
    /// Default: true (improve success rate)
    pub try_fallback: bool,

    /// Follow single redirect if Location header points to same host
    /// Default: false (security consideration - avoid redirect loops)
    pub follow_redirect_once: bool,

    /// Timeout in milliseconds for health check requests
    /// Default: 1500ms (current behavior)
    pub timeout_ms: u32,
}

impl Default for ProxyHealthOptions {
    fn default() -> Self {
        Self {
            use_root_urls: false,        // Maintain compatibility
            try_fallback: true,          // Improve success rate
            follow_redirect_once: false, // Security first
            timeout_ms: 1500,            // Current timeout
        }
    }
}

impl ProxyHealthOptions {
    /// Create default configuration for backward compatibility
    pub fn compatible() -> Self {
        Self::default()
    }

    /// Create enhanced configuration with new features enabled
    pub fn enhanced() -> Self {
        Self {
            use_root_urls: true,
            try_fallback: true,
            follow_redirect_once: true,
            timeout_ms: 1500,
        }
    }

    /// Create security-focused configuration
    pub fn secure() -> Self {
        Self {
            use_root_urls: true,
            try_fallback: false,         // Single attempt only
            follow_redirect_once: false, // No redirects
            timeout_ms: 1000,            // Shorter timeout
        }
    }
}
