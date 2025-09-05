// Core types for network monitoring
use crate::core::network::proxy_health::config::ProxyHealthLevel;
use std::path::PathBuf;

// Re-export credential types from existing module (don't move them)
// pub use super::credential::{CredentialManager, ShellType};

/// Network monitoring status levels
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum NetworkStatus {
    /// API is responding normally within P80 threshold
    Healthy,
    /// API responding but with elevated latency (P80-P95) or rate limited (429)
    Degraded,
    /// API errors, timeouts, or latency above P95
    Error,
    /// No credentials configured or monitoring disabled
    #[default]
    Unknown,
}

/// Detailed information about proxy health check attempt
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProxyHealthDetail {
    /// Primary URL attempted
    pub primary_url: String,
    /// Fallback URL attempted (if any)
    pub fallback_url: Option<String>,
    /// Redirect URL followed (if any)
    pub redirect_url: Option<String>,
    /// Which attempt succeeded: "primary" | "fallback" | "redirect"
    pub success_method: Option<String>,
    /// Timestamp when check was performed
    pub checked_at: String,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Reason for health determination
    /// Values: "cloudflare_challenge", "redirect_followed", "no_endpoint_404",
    /// "non_200_no_cf", "invalid_json_200", "unknown_schema_200", "timeout"
    pub reason: Option<String>,
}

/// State tracking for monitoring windows and probe deduplication
///
/// This structure maintains window-based deduplication to prevent redundant probes
/// within the same timing windows, plus session-based COLD probe deduplication.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitoringState {
    /// Last GREEN window ID that was processed (300s intervals)
    pub last_green_window_id: Option<u64>,
    /// Last RED window ID that was processed (10s intervals)  
    pub last_red_window_id: Option<u64>,
    /// Session ID of the last COLD probe to prevent duplicate session probes
    /// Used for deduplication: same session_id won't trigger multiple COLD probes
    pub last_cold_session_id: Option<String>,
    /// Timestamp of last COLD probe in local timezone ISO-8601 format
    /// Format example: "2025-01-25T10:30:45-08:00"
    pub last_cold_probe_at: Option<String>,
    /// Current network monitoring status
    pub state: NetworkStatus,
}

/// Network metrics and measurements
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkMetrics {
    pub latency_ms: u32,
    pub breakdown: String, // Format: "DNS:20ms|TCP:30ms|TLS:40ms|TTFB:1324ms|Total:2650ms"
    pub last_http_status: u16,
    pub error_type: Option<String>,
    pub rolling_totals: Vec<u32>, // Capacity: 12 samples (~60 min at 300s cadence)
    pub p95_latency_ms: u32,
    #[serde(default)]
    pub connection_reused: Option<bool>, // Connection reuse detection for display purposes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breakdown_source: Option<String>, // "heuristic" | "measured"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_healthy: Option<bool>, // Proxy health status: Some(true)=healthy, Some(false)=unhealthy, None=no proxy or no endpoint
    // New proxy health fields for enhanced tri-state support
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_health_level: Option<ProxyHealthLevel>, // Enhanced tri-state health level
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_health_detail: Option<ProxyHealthDetail>, // Detailed health check information
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_version: Option<String>, // HTTP version used for request (e.g., "HTTP/1.1", "HTTP/2.0")
}

/// Credential source types (aligned with credential.md)
#[derive(Debug, Clone, PartialEq)]
pub enum CredentialSource {
    Environment,
    OAuth,
    ShellConfig(PathBuf),
    ClaudeConfig(PathBuf),
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialSource::Environment => write!(f, "environment"),
            CredentialSource::OAuth => write!(f, "oauth"),
            CredentialSource::ShellConfig(_) => write!(f, "shell"),
            CredentialSource::ClaudeConfig(_) => write!(f, "claude_config"),
        }
    }
}

/// API credentials with source tracking
#[derive(Debug, Clone)]
pub struct ApiCredentials {
    pub base_url: String,
    pub auth_token: String,
    pub source: CredentialSource,
    /// Token expiry timestamp in milliseconds since epoch (OAuth only)
    pub expires_at: Option<i64>,
}

/// Error metadata from JSONL transcript
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JsonlError {
    pub timestamp: String,
    pub code: u16,
    pub message: String,
}

/// HTTP probe execution modes with different timeout and behavior strategies
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeMode {
    /// Cold startup probe - executed once per session when no valid state exists
    /// Uses GREEN timeout strategy but includes session deduplication
    Cold,
    /// Regular monitoring probe every 300 seconds (first 3 seconds of window)
    /// Uses adaptive timeout based on P95 + buffer
    Green,
    /// Error-driven probe every 10 seconds (first 1 second of window) when errors detected
    /// Uses fixed 2000ms timeout for rapid error diagnosis
    Red,
}

/// Complete result of an HTTP probe operation
#[derive(Debug, Clone)]
pub struct ProbeOutcome {
    /// Final network status determination
    pub status: NetworkStatus,
    /// Timing and response metrics
    pub metrics: ProbeMetrics,
    /// Updated P95 latency after this probe
    pub p95_latency_ms: u32,
    /// Number of samples in rolling window
    pub rolling_len: usize,
    /// API configuration that was used
    pub api_config: ApiConfig,
    /// The probe mode that was executed
    pub mode: ProbeMode,
    /// Whether state was successfully written to disk
    pub state_written: bool,
    /// Local timezone timestamp of probe completion
    pub timestamp_local: String,
}

/// Metrics from a single HTTP probe
#[derive(Debug, Clone, Default)]
pub struct ProbeMetrics {
    /// Total request latency in milliseconds
    pub latency_ms: u32,
    /// Timing breakdown string (DNS|TCP|TLS|TTFB|Total format)
    pub breakdown: String,
    /// HTTP status code received
    pub last_http_status: u16,
    /// Standardized error type classification
    pub error_type: Option<String>,
    /// HTTP version used for request (e.g., "HTTP/1.1", "HTTP/2.0")
    pub http_version: Option<String>,
}

/// API configuration metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ApiConfig {
    /// Full endpoint URL that was probed
    pub endpoint: String,
    /// Source of credentials (environment, shell, config)
    pub source: String,
}

/// Complete monitoring state snapshot for read-only access
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct MonitoringSnapshot {
    /// Current network status
    pub status: NetworkStatus,
    /// Whether monitoring is currently enabled
    pub monitoring_enabled: bool,
    /// API configuration details
    pub api_config: Option<ApiConfig>,
    /// Current network metrics and timing data
    pub network: NetworkMetrics,
    /// Monitoring state for window tracking
    pub monitoring_state: MonitoringState,
    /// Last JSONL error event if any
    pub last_jsonl_error_event: Option<JsonlError>,
    /// Timestamp of last state update
    pub timestamp: String,
}

/// Gate types for timing-driven probe execution priority
///
/// Implements COLD > RED > GREEN priority logic where only one gate type
/// executes per collect() call, ensuring optimal resource usage and avoiding
/// redundant network probes.
#[derive(Debug, Clone, PartialEq)]
pub enum GateType {
    /// Cold startup probe for new sessions
    /// Contains session_id for deduplication tracking
    Cold(String),
    /// Error-driven probe during RED window (10s intervals)
    Red,
    /// Regular health check during GREEN window (300s intervals)  
    Green,
    /// Skip probe execution (no conditions met or already deduplicated)
    Skip,
}

/// Network monitoring errors
#[derive(Debug)]
pub enum NetworkError {
    HomeDirNotFound,
    ConfigReadError(String),
    ConfigParseError(String),
    InputParseError(String),
    RegexError(String),
    HttpError(String),
    StateFileError(String),
    CredentialError(String),
    /// Indicates probe should be silently skipped (e.g., expired OAuth token)
    SkipProbe(String),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::HomeDirNotFound => write!(f, "Home directory not found"),
            NetworkError::ConfigReadError(msg) => write!(f, "Config read error: {}", msg),
            NetworkError::ConfigParseError(msg) => write!(f, "Config parse error: {}", msg),
            NetworkError::InputParseError(msg) => write!(f, "Input parse error: {}", msg),
            NetworkError::RegexError(msg) => write!(f, "Regex error: {}", msg),
            NetworkError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            NetworkError::StateFileError(msg) => write!(f, "State file error: {}", msg),
            NetworkError::CredentialError(msg) => write!(f, "Credential error: {}", msg),
            NetworkError::SkipProbe(msg) => write!(f, "Skip probe: {}", msg),
        }
    }
}

/// Window color types for deduplication persistence
#[derive(Debug, Clone, Copy)]
pub enum WindowColor {
    Green,
    Red,
}

impl std::error::Error for NetworkError {}

impl From<std::io::Error> for NetworkError {
    fn from(error: std::io::Error) -> Self {
        NetworkError::ConfigReadError(error.to_string())
    }
}

impl From<serde_json::Error> for NetworkError {
    fn from(error: serde_json::Error) -> Self {
        NetworkError::ConfigParseError(error.to_string())
    }
}

// Default implementations

impl Default for MonitoringState {
    fn default() -> Self {
        Self {
            last_green_window_id: None,
            last_red_window_id: None,
            last_cold_session_id: None,
            last_cold_probe_at: None,
            state: NetworkStatus::Unknown,
        }
    }
}

impl Default for NetworkMetrics {
    fn default() -> Self {
        Self {
            latency_ms: 0,
            breakdown: String::new(),
            last_http_status: 0,
            error_type: None,
            rolling_totals: Vec::with_capacity(12), // Max 60 minutes at 300s intervals
            p95_latency_ms: 0,
            connection_reused: None,
            breakdown_source: None,
            proxy_healthy: None,
            proxy_health_level: None,
            proxy_health_detail: None,
            http_version: None,
        }
    }
}

/// Centralized proxy health field management
impl NetworkMetrics {
    /// Set proxy health with automatic field consistency
    ///
    /// Updates both legacy proxy_healthy and new proxy_health_level fields
    /// to maintain backward compatibility while supporting enhanced tri-state levels.
    ///
    /// # Arguments
    /// * `level` - Enhanced proxy health level (None = no proxy/no endpoint)
    /// * `detail` - Detailed information about health check attempt
    ///
    /// # Field Mapping
    /// - Healthy → proxy_healthy=Some(true), proxy_health_level=Some(Healthy)
    /// - Degraded → proxy_healthy=Some(false), proxy_health_level=Some(Degraded)  
    /// - Bad → proxy_healthy=Some(false), proxy_health_level=Some(Bad)
    /// - None → proxy_healthy=None, proxy_health_level=None
    pub fn set_proxy_health(
        &mut self,
        level: Option<ProxyHealthLevel>,
        detail: Option<ProxyHealthDetail>,
    ) {
        self.proxy_health_level = level.clone();
        self.proxy_health_detail = detail;

        // Maintain backward compatibility with legacy proxy_healthy field
        self.proxy_healthy = match level {
            Some(ProxyHealthLevel::Healthy) => Some(true),
            Some(ProxyHealthLevel::Degraded)
            | Some(ProxyHealthLevel::Bad)
            | Some(ProxyHealthLevel::Unknown) => Some(false),
            None => None,
        };
    }

    /// Get proxy health level with fallback to legacy field
    ///
    /// Provides seamless access to proxy health status with automatic fallback
    /// for backward compatibility with existing monitoring files.
    ///
    /// # Returns
    /// - Enhanced level if available (proxy_health_level)
    /// - Mapped from legacy field if enhanced unavailable (proxy_healthy)
    /// - None if no proxy health information available
    pub fn get_proxy_health_level(&self) -> Option<ProxyHealthLevel> {
        // Priority: new field > legacy field mapping
        self.proxy_health_level.clone().or_else(|| {
            self.proxy_healthy.map(|healthy| {
                if healthy {
                    ProxyHealthLevel::Healthy
                } else {
                    ProxyHealthLevel::Bad // Default mapping for false
                }
            })
        })
    }
}

// Environment variable utilities
/// Parse boolean environment variables (strict true/false only)
///
/// Only accepts "true" or "false" (case insensitive). All other values default to false.
///
/// # Examples
///
/// ```rust
/// use ccstatus::core::network::types::parse_env_bool;
///
/// // These return true
/// std::env::set_var("TEST_VAR", "true");
/// assert_eq!(parse_env_bool("TEST_VAR"), true);
/// std::env::set_var("TEST_VAR", "TRUE");
/// assert_eq!(parse_env_bool("TEST_VAR"), true);
///
/// // These all return false  
/// std::env::set_var("TEST_VAR", "false");
/// assert_eq!(parse_env_bool("TEST_VAR"), false);
/// std::env::set_var("TEST_VAR", "1");      // Not accepted
/// std::env::set_var("TEST_VAR", "yes");    // Not accepted  
/// std::env::remove_var("TEST_VAR");        // Unset
/// assert_eq!(parse_env_bool("TEST_VAR"), false);
/// ```
pub fn parse_env_bool(env_var: &str) -> bool {
    std::env::var(env_var)
        .map(|v| match v.trim().to_lowercase().as_str() {
            "true" => true,
            "false" => false,
            _ => false,
        })
        .unwrap_or(false)
}

// Timestamp standardization utilities
/// Generate standardized local timezone ISO-8601 timestamp
///
/// This function provides consistent timestamp formatting across all network monitoring
/// components. All persistent timestamps should use this function to ensure uniformity.
///
/// # Returns
///
/// A string in RFC3339/ISO-8601 format with local timezone offset.
///
/// # Example Format
///
/// ```text
/// "2025-01-25T10:30:45-08:00"  // Pacific Time (PST)
/// "2025-01-25T18:30:45+00:00"  // UTC
/// "2025-01-25T19:30:45+01:00"  // Central European Time (CET)
/// ```
///
/// # Usage
///
/// Used for:
/// - `MonitoringState.last_cold_probe_at` field
/// - Error tracking timestamps  
/// - State persistence timestamps
/// - Debug logging with consistent time format
pub fn get_local_timestamp() -> String {
    use std::time::SystemTime;

    // Get current local time and format as ISO-8601 with timezone offset
    let now = SystemTime::now();
    let datetime: chrono::DateTime<chrono::Local> = now.into();
    datetime.to_rfc3339()
}
