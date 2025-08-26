// Core types for network monitoring
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

/// State tracking for monitoring windows and probe deduplication
///
/// This structure maintains window-based deduplication to prevent redundant probes
/// within the same timing windows, plus session-based COLD probe deduplication.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MonitoringState {
    /// Last GREEN window ID that was processed (300s intervals)
    pub last_green_window_id: u64,
    /// Last RED window ID that was processed (10s intervals)  
    pub last_red_window_id: u64,
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
}

/// Credential source types (aligned with credential.md)
#[derive(Debug, Clone)]
pub enum CredentialSource {
    Environment,
    ShellConfig(PathBuf),
    ClaudeConfig(PathBuf),
}

impl std::fmt::Display for CredentialSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialSource::Environment => write!(f, "environment"),
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
        }
    }
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
            last_green_window_id: 0,
            last_red_window_id: 0,
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
        }
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
