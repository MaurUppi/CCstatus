/*!
Network HTTP monitoring with probe execution and state persistence.

This module implements the HttpMonitor component - the single authoritative writer
for network monitoring state. It executes lightweight HTTP probes to Claude API
endpoints and maintains atomic state persistence with comprehensive timing metrics.

## Core Responsibilities

- **Single Writer**: Only HttpMonitor writes to ccstatus-monitoring.json
- **Probe Execution**: Lightweight POST probes with different timeout strategies
- **State Management**: Atomic file operations with temp+rename pattern
- **Rolling Statistics**: P95 calculation from GREEN probe samples only
- **Error Classification**: Standardized HTTP status code mapping
- **Observability**: Integration with DebugLogger for probe lifecycle tracking

## Probe Modes

- **COLD**: One-time session startup probe with session deduplication
- **GREEN**: Regular health monitoring (300s intervals, adaptive timeout)
- **RED**: Error-driven rapid diagnosis (10s intervals, fixed 2000ms timeout)

## Dependencies

- `isahc`: HTTP client with timing metrics and configurability
- `tokio`: Async runtime for non-blocking probe execution
- `serde_json`: State serialization/deserialization
- `chrono`: Local timezone timestamp generation
*/

use crate::core::network::debug_logger::get_debug_logger;
use crate::core::network::types::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[cfg(feature = "network-monitoring")]
use isahc::config::{Configurable, RedirectPolicy};
#[cfg(feature = "network-monitoring")]
use isahc::{HttpClient, Request, AsyncReadResponseExt};

#[cfg(feature = "network-monitoring")]
use futures::io::{copy, sink};

#[cfg(feature = "timings-curl")]
use curl::easy::Easy;

#[cfg(feature = "timings-curl")]
/// Phase timings extracted from curl probe
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseTimings {
    pub status: u16,
    pub dns_ms: u32,
    pub tcp_ms: u32,
    pub tls_ms: u32,
    pub ttfb_ms: u32,
    pub total_ms: u32,
}

#[cfg(feature = "timings-curl")]
/// Curl probe runner abstraction for dependency injection
#[async_trait::async_trait]
pub trait CurlProbeRunner: Send + Sync {
    async fn run(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
        timeout_ms: u32,
    ) -> Result<PhaseTimings, NetworkError>;
}

/// HTTP client abstraction for dependency injection and testing
#[async_trait::async_trait]
pub trait HttpClientTrait: Send + Sync {
    /// Execute HTTP request with timing measurement
    /// Returns (status_code, duration, breakdown_string)
    async fn execute_request(
        &self,
        url: String,
        headers: std::collections::HashMap<String, String>,
        body: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<(u16, Duration, String), String>;
}

/// Health check response containing full response data for validation
#[derive(Debug, Clone)]
pub struct HealthResponse {
    /// HTTP status code from health endpoint
    pub status_code: u16,
    /// Response body content for JSON validation
    pub body: Vec<u8>,
    /// Request duration for metrics
    pub duration: Duration,
}

/// Dedicated HTTP client for health check operations
/// 
/// This trait is specialized for health check requirements that differ from 
/// regular API calls: GET method, response body access, redirect control.
#[async_trait::async_trait]
pub trait HealthCheckClient: Send + Sync {
    /// Execute GET request to health endpoint with full response
    ///
    /// # Arguments
    /// * `url` - Complete health check URL (e.g., "https://proxy.com/health")
    /// * `timeout_ms` - Request timeout in milliseconds
    ///
    /// # Returns
    /// * `Ok(HealthResponse)` - Successful response with status, body, and timing
    /// * `Err(String)` - Network error or request failure
    ///
    /// # Implementation Requirements
    /// * Must use GET method (not POST)
    /// * Must not follow redirects (treat 3xx as error)
    /// * Must return complete response body for JSON validation
    async fn get_health(
        &self,
        url: String,
        timeout_ms: u32,
    ) -> Result<HealthResponse, String>;
}

/// Clock abstraction for dependency injection and testing  
pub trait ClockTrait: Send + Sync {
    /// Get current system time
    fn now(&self) -> Instant;
    /// Get local timezone timestamp
    fn local_timestamp(&self) -> String;
}

/// Production HTTP client implementation using isahc
#[cfg(feature = "network-monitoring")]
pub struct IsahcHttpClient {
    client: HttpClient,
}

#[cfg(feature = "network-monitoring")]
#[async_trait::async_trait]
impl HttpClientTrait for IsahcHttpClient {
    async fn execute_request(
        &self,
        url: String,
        headers: std::collections::HashMap<String, String>,
        body: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<(u16, Duration, String), String> {
        let start = Instant::now();

        let mut request = Request::post(&url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .body(body)
            .map_err(|e| format!("Request creation failed: {}", e))?;

        // Add headers
        for (key, value) in headers {
            let header_name = key
                .parse::<isahc::http::header::HeaderName>()
                .map_err(|e| format!("Invalid header name: {}", e))?;
            let header_value = value
                .parse::<isahc::http::header::HeaderValue>()
                .map_err(|e| format!("Invalid header value: {}", e))?;
            request.headers_mut().insert(header_name, header_value);
        }

        let response = self
            .client
            .send_async(request)
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        let ttfb_duration = start.elapsed();

        let status = response.status().as_u16();

        // Drain response body without allocating (zero-copy to sink)
        let mut body = response.into_body();
        let _ = copy(&mut body, &mut sink()).await.map_err(|e| format!("Failed to drain response body: {}", e))?;

        // Generate timing breakdown with stable numeric format
        let total_ms = ttfb_duration.as_millis() as u32;
        
        // Keep breakdown numeric and parseable - always DNS|TCP|TLS|TTFB|Total format
        // For now, we don't have individual timing phases from isahc, so estimate
        let dns_ms = 0u32;  // Connection reuse = 0ms for DNS
        let tcp_ms = 0u32;  // Connection reuse = 0ms for TCP  
        let tls_ms = 0u32;  // Connection reuse = 0ms for TLS
        let breakdown = format!(
            "DNS:{}ms|TCP:{}ms|TLS:{}ms|TTFB:{}ms|Total:{}ms",
            dns_ms, tcp_ms, tls_ms, total_ms, total_ms
        );

        Ok((status, ttfb_duration, breakdown))
    }
}

#[cfg(feature = "network-monitoring")]
impl IsahcHttpClient {
    pub fn new() -> Result<Self, NetworkError> {
        let client = HttpClient::new()
            .map_err(|e| NetworkError::HttpError(format!("Failed to create HTTP client: {}", e)))?;
        Ok(Self { client })
    }
}

/// Production health check client implementation using isahc with GET method
#[cfg(feature = "network-monitoring")]
pub struct IsahcHealthCheckClient {
    client: HttpClient,
}

#[cfg(feature = "network-monitoring")]
#[async_trait::async_trait]
impl HealthCheckClient for IsahcHealthCheckClient {
    async fn get_health(
        &self,
        url: String,
        timeout_ms: u32,
    ) -> Result<HealthResponse, String> {
        let start = Instant::now();

        let request = Request::get(&url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .redirect_policy(RedirectPolicy::None)  // Critical: Don't follow redirects
            .header("User-Agent", "claude-cli/1.0.80 (external, cli)")
            .body(Vec::new())  // Empty body for GET request
            .map_err(|e| format!("Health check request creation failed: {}", e))?;

        let mut response = self
            .client
            .send_async(request)
            .await
            .map_err(|e| format!("Health check request failed: {}", e))?;

        let status_code = response.status().as_u16();
        let duration = start.elapsed();

        // Read response body for JSON validation (unlike regular API client)
        let body = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read health check response body: {}", e))?
            .to_vec();

        Ok(HealthResponse {
            status_code,
            body,
            duration,
        })
    }
}

#[cfg(feature = "network-monitoring")]
impl IsahcHealthCheckClient {
    pub fn new() -> Result<Self, NetworkError> {
        let client = HttpClient::builder()
            .redirect_policy(RedirectPolicy::None)  // Global redirect policy
            .build()
            .map_err(|e| NetworkError::HttpError(format!("Failed to create health check client: {}", e)))?;
        Ok(Self { client })
    }
}

/// Production clock implementation using system time
#[derive(Default)]
pub struct SystemClock;

impl ClockTrait for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn local_timestamp(&self) -> String {
        get_local_timestamp()
    }
}

/// Production curl runner implementation
#[cfg(feature = "timings-curl")]
pub struct RealCurlRunner;

#[cfg(feature = "timings-curl")]
#[async_trait::async_trait]
impl CurlProbeRunner for RealCurlRunner {
    async fn run(
        &self,
        url: &str,
        headers: &[(&str, String)],
        body: &[u8],
        timeout_ms: u32,
    ) -> Result<PhaseTimings, NetworkError> {
        let url = url.to_string();
        let headers = headers.iter().map(|(k, v)| (k.to_string(), v.clone())).collect::<Vec<_>>();
        let body = body.to_vec();
        
        let result = tokio::task::spawn_blocking(move || -> Result<PhaseTimings, String> {
            let mut handle = Easy::new();
            
            // Configure request
            handle.url(&url).map_err(|e| format!("URL set failed: {}", e))?;
            handle.post(true).map_err(|e| format!("POST set failed: {}", e))?;
            handle.post_fields_copy(&body).map_err(|e| format!("POST fields failed: {}", e))?;
            handle.timeout(std::time::Duration::from_millis(timeout_ms as u64))
                .map_err(|e| format!("Timeout set failed: {}", e))?;
            
            // Set headers
            let mut header_list = curl::easy::List::new();
            for (key, value) in headers {
                header_list.append(&format!("{}: {}", key, value))
                    .map_err(|e| format!("Header append failed: {}", e))?;
            }
            handle.http_headers(header_list).map_err(|e| format!("Headers set failed: {}", e))?;
            
            // Capture response body but don't store it
            handle.write_function(|data| {
                // Drain response body without storing
                Ok(data.len())
            }).map_err(|e| format!("Write function failed: {}", e))?;
            
            // Execute request and capture timings
            handle.perform().map_err(|e| format!("Request perform failed: {}", e))?;
            
            // Extract phase timings from libcurl (in seconds, convert to ms)
            let dns_time = handle.namelookup_time()
                .map_err(|e| format!("DNS time failed: {}", e))?.as_secs_f64();
            let connect_time = handle.connect_time()
                .map_err(|e| format!("Connect time failed: {}", e))?.as_secs_f64();
            let appconnect_time = handle.appconnect_time()
                .map_err(|e| format!("App connect time failed: {}", e))?.as_secs_f64();
            let starttransfer_time = handle.starttransfer_time()
                .map_err(|e| format!("Start transfer time failed: {}", e))?.as_secs_f64();
            let total_time = handle.total_time()
                .map_err(|e| format!("Total time failed: {}", e))?.as_secs_f64();
            
            // Calculate phase durations and convert to milliseconds
            let dns_ms = (dns_time * 1000.0).max(0.0) as u32;
            let tcp_ms = ((connect_time - dns_time).max(0.0) * 1000.0) as u32;
            let tls_ms = ((appconnect_time - connect_time).max(0.0) * 1000.0) as u32;
            let ttfb_ms = ((starttransfer_time - appconnect_time).max(0.0) * 1000.0) as u32;
            let total_ms = (total_time * 1000.0).max(0.0) as u32;
            
            // Get response status
            let status = handle.response_code()
                .map_err(|e| format!("Response code failed: {}", e))? as u16;
            
            Ok(PhaseTimings {
                status,
                dns_ms,
                tcp_ms,
                tls_ms,
                ttfb_ms,
                total_ms,
            })
        }).await
        .map_err(|e| NetworkError::HttpError(format!("Curl task join failed: {}", e)))?
        .map_err(|e| NetworkError::HttpError(e))?;

        Ok(result)
    }
}

/// HTTP monitoring component - single writer for network state
///
/// HttpMonitor executes lightweight HTTP probes and maintains authoritative
/// network monitoring state. It's the only component that writes to the
/// ccstatus-monitoring.json file, ensuring data consistency and avoiding
/// concurrent write conflicts.
pub struct HttpMonitor {
    /// Path to monitoring state file
    state_path: PathBuf,
    /// HTTP client for probe execution
    http_client: Box<dyn HttpClientTrait>,
    /// Health check client for proxy endpoint validation
    health_client: Box<dyn HealthCheckClient>,
    /// Clock for timing operations
    clock: Box<dyn ClockTrait>,
    /// Optional timeout override for testing
    timeout_override_ms: Option<u32>,
    /// Current session ID for COLD probe deduplication
    current_session_id: Option<String>,
    /// Optional curl probe runner for phase timing measurement
    #[cfg(feature = "timings-curl")]
    curl_runner: Option<Box<dyn CurlProbeRunner>>,
}

impl HttpMonitor {
    /// Create new HttpMonitor with default configuration
    ///
    /// Uses default state path: `~/.claude/ccstatus/ccstatus-monitoring.json`
    /// 
    /// When `timings-curl` feature is enabled, automatically wires `RealCurlRunner`
    /// for detailed phase timings (disabled in test builds for safety).
    /// Use `with_curl_runner()` to override with custom implementations.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::HomeDirNotFound` if home directory cannot be determined.
    /// Returns `NetworkError::HttpError` if HTTP client creation fails.
    pub fn new(state_path: Option<PathBuf>) -> Result<Self, NetworkError> {
        let state_path = match state_path {
            Some(path) => path,
            None => {
                let home = dirs::home_dir().ok_or(NetworkError::HomeDirNotFound)?;
                home.join(".claude")
                    .join("ccstatus")
                    .join("ccstatus-monitoring.json")
            }
        };

        #[cfg(feature = "network-monitoring")]
        let http_client: Box<dyn HttpClientTrait> = Box::new(IsahcHttpClient::new()?);
        #[cfg(not(feature = "network-monitoring"))]
        let http_client: Box<dyn HttpClientTrait> = Box::new(MockHttpClient::default());

        #[cfg(feature = "network-monitoring")]
        let health_client: Box<dyn HealthCheckClient> = Box::new(IsahcHealthCheckClient::new()?);
        #[cfg(not(feature = "network-monitoring"))]
        let health_client: Box<dyn HealthCheckClient> = Box::new(MockHealthCheckClient::default());

        Ok(Self {
            state_path,
            http_client,
            health_client,
            clock: Box::new(SystemClock),
            timeout_override_ms: None,
            current_session_id: None,
            #[cfg(feature = "timings-curl")]
            curl_runner: Some(Box::new(RealCurlRunner)),
        })
    }

    /// Configure HttpMonitor with custom HTTP client (for testing)
    pub fn with_http_client(mut self, client: Box<dyn HttpClientTrait>) -> Self {
        self.http_client = client;
        self
    }

    /// Configure HttpMonitor with custom health check client (for testing)
    pub fn with_health_client(mut self, client: Box<dyn HealthCheckClient>) -> Self {
        self.health_client = client;
        self
    }

    /// Configure HttpMonitor with custom clock (for testing)
    pub fn with_clock(mut self, clock: Box<dyn ClockTrait>) -> Self {
        self.clock = clock;
        self
    }
    
    /// Configure HttpMonitor with custom curl runner (for testing with timings-curl feature)
    #[cfg(feature = "timings-curl")]
    pub fn with_curl_runner(mut self, runner: Box<dyn CurlProbeRunner>) -> Self {
        self.curl_runner = Some(runner);
        self
    }

    /// Override timeout for all probe modes (for testing)
    ///
    /// When set, both GREEN and RED probes will use min(override_ms, 6000).
    /// This matches the behavior of the CCSTATUS_TIMEOUT_MS environment variable.
    pub fn with_timeout_override_ms(mut self, timeout_ms: u32) -> Self {
        self.timeout_override_ms = Some(timeout_ms);
        self
    }

    /// Set session ID for COLD probe deduplication
    ///
    /// This method allows NetworkSegment to provide the actual session_id for proper
    /// COLD probe deduplication. Should be called before executing COLD probes to
    /// ensure accurate session tracking.
    ///
    /// # Arguments
    ///
    /// - `session_id`: Unique session identifier from the orchestrating component
    ///
    /// # Usage
    ///
    /// NetworkSegment should call `monitor.set_session_id("session_abc123")` before
    /// executing COLD probes to enable proper session deduplication.
    pub fn set_session_id(&mut self, session_id: String) {
        self.current_session_id = Some(session_id);
    }

    /// Set last GREEN window ID for per-window deduplication
    ///
    /// Updates the persisted state with the GREEN window ID to prevent redundant
    /// probes within the same 300-second window. Only persists if the ID is newer
    /// than the currently stored ID (monotonic updates).
    ///
    /// # Arguments
    ///
    /// * `window_id` - The GREEN window ID to persist (calculated as total_duration_ms / 300_000)
    ///
    /// # Returns
    ///
    /// * `Ok(())` if successfully persisted or skipped due to monotonic check
    /// * `Err(NetworkError)` if persistence fails
    pub async fn set_green_window_id(&self, window_id: u64) -> Result<(), NetworkError> {
        self.set_last_window_id(WindowColor::Green, window_id).await
    }

    /// Set last RED window ID for per-window deduplication
    ///
    /// Updates the persisted state with the RED window ID to prevent redundant
    /// probes within the same 10-second window. Only persists if the ID is newer
    /// than the currently stored ID (monotonic updates).
    ///
    /// # Arguments
    ///
    /// * `window_id` - The RED window ID to persist (calculated as total_duration_ms / 10_000)
    ///
    /// # Returns
    ///
    /// * `Ok(())` if successfully persisted or skipped due to monotonic check
    /// * `Err(NetworkError)` if persistence fails
    pub async fn set_red_window_id(&self, window_id: u64) -> Result<(), NetworkError> {
        self.set_last_window_id(WindowColor::Red, window_id).await
    }

    /// Internal helper for monotonic window ID persistence
    ///
    /// Updates the specified window ID field atomically with monotonic enforcement.
    /// Only persists if the new ID is strictly greater than the current stored ID.
    ///
    /// # Arguments
    ///
    /// * `color` - Which window type to update (Green or Red)
    /// * `window_id` - The window ID to persist
    ///
    /// # Returns
    ///
    /// * `Ok(())` if successfully persisted or skipped due to monotonic check
    /// * `Err(NetworkError)` if loading state or persistence fails
    async fn set_last_window_id(&self, color: WindowColor, window_id: u64) -> Result<(), NetworkError> {
        // Load current state to get existing window IDs
        let mut state = self.load_state().await?;
        
        // Get reference to the appropriate window ID field
        let current_id = match color {
            WindowColor::Green => state.monitoring_state.last_green_window_id,
            WindowColor::Red => state.monitoring_state.last_red_window_id,
        };
        
        // Monotonic check: only persist if new ID is greater than current
        if let Some(current) = current_id {
            if current >= window_id {
                // Skip update - current ID is equal or newer
                return Ok(());
            }
        }
        
        // Update the appropriate field
        match color {
            WindowColor::Green => {
                state.monitoring_state.last_green_window_id = Some(window_id);
            }
            WindowColor::Red => {
                state.monitoring_state.last_red_window_id = Some(window_id);
            }
        }
        
        // Update timestamp
        state.timestamp = get_local_timestamp();
        
        // Persist atomically
        self.write_state_atomic(&state).await
    }

    /// Execute HTTP probe and update monitoring state
    ///
    /// This is the primary method for network monitoring. It executes a lightweight
    /// HTTP probe to the Claude API endpoint and updates the persistent state based
    /// on the probe mode and results.
    ///
    /// # Arguments
    ///
    /// - `mode`: Probe execution mode (COLD/GREEN/RED) with different timeout strategies
    /// - `creds`: API credentials including endpoint URL and authentication token  
    /// - `last_jsonl_error_event`: Optional error event from transcript (for RED mode context)
    ///
    /// # Probe Mode Behavior
    ///
    /// - **COLD**: Uses GREEN timeout strategy, includes session deduplication fields
    /// - **GREEN**: Adaptive timeout based on P95+500ms, updates rolling statistics if HTTP 200
    /// - **RED**: Fixed 2000ms timeout, never updates rolling statistics, sets status=error
    ///
    /// # Returns
    ///
    /// `ProbeOutcome` with comprehensive results including:
    /// - Network status determination (healthy/degraded/error)
    /// - Timing metrics with DNS|TCP|TLS|TTFB|Total breakdown
    /// - Updated P95 latency and rolling statistics
    /// - State persistence confirmation
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::HttpError` for probe execution failures.
    /// Returns `NetworkError::StateFileError` for state persistence failures.
    pub async fn probe(
        &mut self,
        mode: ProbeMode,
        creds: ApiCredentials,
        last_jsonl_error_event: Option<JsonlError>,
    ) -> Result<ProbeOutcome, NetworkError> {
        let debug_logger = get_debug_logger();
        let probe_start = self.clock.now();

        // Calculate timeout based on mode and existing state
        let timeout_ms = self.calculate_timeout(mode).await?;

        // Generate consistent probe ID for logging correlation
        let probe_id = format!("probe_{}", uuid::Uuid::new_v4());
        debug_logger.network_probe_start(
            &format!("{:?}", mode),
            timeout_ms as u64,
            probe_id.clone(),
        );

        // Execute HTTP probe
        let probe_result = self
            .execute_http_probe(&creds, timeout_ms, probe_start)
            .await;

        let (status_code, latency_ms, breakdown, error_type) = match probe_result {
            Ok((status, duration, breakdown)) => {
                let error_type = self.classify_http_error(status);
                (status, duration.as_millis() as u32, breakdown, error_type)
            }
            Err(err) => {
                debug_logger
                    .error("HttpMonitor", &format!("Probe failed: {}", err))
                    .await;
                (
                    0,
                    probe_start.elapsed().as_millis() as u32,
                    format!(
                        "DNS:0ms|TCP:0ms|TLS:0ms|TTFB:0ms|Total:{}ms",
                        probe_start.elapsed().as_millis()
                    ),
                    Some("connection_error".to_string()),
                )
            }
        };

        // Build probe metrics
        let metrics = ProbeMetrics {
            latency_ms,
            breakdown: breakdown.clone(),
            last_http_status: status_code,
            error_type: error_type.clone(),
        };

        // Process probe results and update state
        let outcome = self
            .process_probe_results(mode, creds, metrics, last_jsonl_error_event)
            .await?;

        debug_logger.network_probe_end(
            &format!("{:?}", mode),
            if status_code == 0 {
                None
            } else {
                Some(status_code)
            },
            latency_ms as u64,
            probe_id,
        );

        debug_logger.state_write_summary(
            &format!("{:?}", outcome.status),
            outcome.p95_latency_ms as u64,
            outcome.rolling_len as u32,
        );

        Ok(outcome)
    }

    /// Write unknown status when credentials are unavailable
    ///
    /// This method handles the case where network monitoring cannot proceed due to
    /// missing or invalid credentials. It writes a minimal state indicating monitoring
    /// is disabled while preserving existing P95 statistics.
    ///
    /// # Arguments
    ///
    /// - `monitoring_enabled`: Whether to mark monitoring as enabled in the state file
    ///
    /// # State Updates
    ///
    /// - Sets `status = unknown`
    /// - Sets `monitoring_enabled = false`
    /// - Clears `api_config` or sets `source = null`
    /// - Preserves existing `rolling_totals` and `p95_latency_ms`
    /// - Updates timestamp to current local time
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::StateFileError` if state file cannot be written.
    pub async fn write_unknown(&mut self, monitoring_enabled: bool) -> Result<(), NetworkError> {
        let debug_logger = get_debug_logger();

        debug_logger
            .debug(
                "HttpMonitor",
                "Writing unknown status - no credentials available",
            )
            .await;

        // Load existing state to preserve rolling statistics
        let mut state = self.load_state_internal().await.unwrap_or_default();

        // Update state for unknown status
        state.status = NetworkStatus::Unknown;
        state.monitoring_enabled = monitoring_enabled;
        state.api_config = None; // Clear API config when no credentials
        state.timestamp = self.clock.local_timestamp();

        // Write state atomically
        self.write_state_atomic(&state).await?;

        debug_logger.state_write_summary(
            "Unknown",
            state.network.p95_latency_ms as u64,
            state.network.rolling_totals.len() as u32,
        );

        Ok(())
    }

    /// Load current monitoring state for read-only access
    ///
    /// This method provides read-only access to the current monitoring state
    /// without modifying any data. It's useful for status rendering and debugging.
    ///
    /// # Returns
    ///
    /// `MonitoringSnapshot` containing complete current state including:
    /// - Current network status and metrics
    /// - Rolling statistics and P95 latency
    /// - Monitoring configuration and last error events
    /// - Window tracking state for deduplication
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::StateFileError` if state file cannot be read or parsed.
    /// If the state file doesn't exist, returns a default state rather than an error.
    pub async fn load_state(&self) -> Result<MonitoringSnapshot, NetworkError> {
        self.load_state_internal().await
    }

    // Private helper methods

    /// Get timeout from environment variables (supports both naming conventions)
    fn get_timeout_env_var() -> Option<u32> {
        // Try both uppercase (current) and lowercase (spec) variants for compatibility
        let env_names = ["CCSTATUS_TIMEOUT_MS", "ccstatus_TIMEOUT_MS"];
        for name in &env_names {
            if let Ok(env_timeout) = std::env::var(name) {
                if let Ok(env_val) = env_timeout.parse::<u32>() {
                    return Some(env_val);
                }
            }
        }
        None
    }

    /// Convert UTC timestamp to local timezone ISO-8601 format
    /// 
    /// Converts timestamps from JSONL transcript (typically UTC with 'Z' suffix) 
    /// to local timezone format for consistent persistence.
    fn convert_utc_to_local_timestamp(utc_timestamp: &str) -> Result<String, NetworkError> {
        use chrono::{DateTime, Utc, Local};
        
        let utc_dt: DateTime<Utc> = utc_timestamp.parse()
            .map_err(|e| NetworkError::ConfigParseError(format!("Invalid UTC timestamp: {}", e)))?;
        
        let local_dt = utc_dt.with_timezone(&Local);
        Ok(local_dt.to_rfc3339())
    }

    /// Calculate appropriate timeout for probe mode
    async fn calculate_timeout(&self, mode: ProbeMode) -> Result<u32, NetworkError> {
        // Check for environment override first (supports both naming conventions)
        if let Some(env_val) = Self::get_timeout_env_var() {
            return Ok(std::cmp::min(env_val, 6000));
        }

        // Check for test override
        if let Some(override_ms) = self.timeout_override_ms {
            return Ok(std::cmp::min(override_ms, 6000));
        }

        match mode {
            ProbeMode::Red => Ok(2000), // Fixed 2000ms for RED mode
            ProbeMode::Green | ProbeMode::Cold => {
                // GREEN/COLD use adaptive timeout based on P95
                let state = self.load_state_internal().await.unwrap_or_default();

                if state.network.rolling_totals.len() < 4 {
                    Ok(3500) // Default when insufficient samples
                } else {
                    let p95 = state.network.p95_latency_ms;
                    let adaptive_timeout = p95 + 500;
                    Ok(adaptive_timeout.clamp(2500, 4000))
                }
            }
        }
    }



    /// Execute HTTP probe with timing measurement
    /// 
    /// Uses curl-based probe for detailed phase timings when timings-curl feature is enabled
    /// (auto-wired by default, can be overridden). Falls back to isahc-based probe on curl 
    /// failures or when no runner is available.
    async fn execute_http_probe(
        &self,
        creds: &ApiCredentials,
        timeout_ms: u32,
        _probe_start: Instant,
    ) -> Result<(u16, Duration, String), NetworkError> {
        // Check if curl runner is available for detailed timing measurements
        #[cfg(feature = "timings-curl")]
        if let Some(ref curl_runner) = self.curl_runner {
            let endpoint = format!("{}/v1/messages", creds.base_url);

            // Minimal Claude API payload for probing
            let payload = serde_json::json!({
                "model": "claude-3-5-haiku-20241022",
                "max_tokens": 1,
                "messages": [
                    {"role": "user", "content": "Hi"}
                ]
            });

            let body = serde_json::to_vec(&payload)
                .map_err(|e| NetworkError::HttpError(format!("Payload serialization failed: {}", e)))?;

            let headers = vec![
                ("Content-Type", "application/json".to_string()),
                ("x-api-key", creds.auth_token.clone()),
                ("User-Agent", "claude-cli/1.0.80 (external, cli)".to_string()),
                ("anthropic-version", "2023-06-01".to_string()),
            ];

            // Try curl first, fallback to isahc on failure for resiliency
            match curl_runner.run(&endpoint, &headers, &body, timeout_ms).await {
                Ok(phase_timings) => {
                    let duration = Duration::from_millis(phase_timings.ttfb_ms as u64);
                    let breakdown = format!(
                        "DNS:{}ms|TCP:{}ms|TLS:{}ms|TTFB:{}ms|Total:{}ms",
                        phase_timings.dns_ms,
                        phase_timings.tcp_ms,
                        phase_timings.tls_ms,
                        phase_timings.ttfb_ms,
                        phase_timings.total_ms
                    );
                    return Ok((phase_timings.status, duration, breakdown));
                }
                Err(curl_error) => {
                    // Log curl failure and fallback to isahc for resiliency
                    let debug_logger = get_debug_logger();
                    let _ = debug_logger.error("HttpMonitor", 
                        &format!("Curl probe failed, falling back to isahc: {}", curl_error)).await;
                    // Fall through to isahc path below
                }
            }
        }

        // Fallback to isahc-based probe with heuristic timing breakdown
        let endpoint = format!("{}/v1/messages", creds.base_url);

        // Minimal Claude API payload for probing
        let payload = serde_json::json!({
            "model": "claude-3-5-haiku-20241022",
            "max_tokens": 1,
            "messages": [
                {"role": "user", "content": "Hi"}
            ]
        });

        let body = serde_json::to_vec(&payload)
            .map_err(|e| NetworkError::HttpError(format!("Payload serialization failed: {}", e)))?;

        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("x-api-key".to_string(), creds.auth_token.clone());
        headers.insert("User-Agent".to_string(), "claude-cli/1.0.80 (external, cli)".to_string());
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());

        let (status_code, duration, breakdown) = self
            .http_client
            .execute_request(endpoint, headers, body, timeout_ms)
            .await
            .map_err(NetworkError::HttpError)?;

        Ok((status_code, duration, breakdown))
    }

    /// Classify HTTP status codes into standard error types
    fn classify_http_error(&self, status_code: u16) -> Option<String> {
        match status_code {
            200..=299 => None, // Success
            0 => Some("connection_error".to_string()),
            400 => Some("invalid_request_error".to_string()),
            401 => Some("authentication_error".to_string()),
            403 => Some("permission_error".to_string()),
            404 => Some("not_found_error".to_string()),
            413 => Some("request_too_large".to_string()),
            429 => Some("rate_limit_error".to_string()),
            500 => Some("api_error".to_string()),
            504 => Some("socket_hang_up".to_string()),
            529 => Some("overloaded_error".to_string()),
            402 | 405..=412 | 414..=428 | 430..=499 => Some("client_error".to_string()),
            501..=503 | 505..=528 | 530..=599 => Some("server_error".to_string()),
            _ => Some("unknown_error".to_string()),
        }
    }

    /// Process probe results and update persistent state
    async fn process_probe_results(
        &mut self,
        mode: ProbeMode,
        creds: ApiCredentials,
        metrics: ProbeMetrics,
        last_jsonl_error_event: Option<JsonlError>,
    ) -> Result<ProbeOutcome, NetworkError> {
        let mut state = self.load_state_internal().await.unwrap_or_default();

        // Connection reuse calculation (only for heuristic path)
        let _p95 = state.network.p95_latency_ms;

        // Determine breakdown source and connection reuse based on feature configuration
        #[cfg(feature = "timings-curl")]
        let breakdown_source = "measured";
        #[cfg(not(feature = "timings-curl"))]
        let breakdown_source = "heuristic";
        
        #[cfg(not(feature = "timings-curl"))]
        {
            // Keep breakdown numeric; log reuse heuristics (only for heuristic path)
            let breakdown_numeric = format!(
                "DNS:0ms|TCP:0ms|TLS:0ms|TTFB:{}ms|Total:{}ms",
                metrics.latency_ms, metrics.latency_ms
            );

            let debug_logger = get_debug_logger();
            debug_logger.debug(
                "HttpMonitor",
                &format!("heuristic_breakdown: total={}ms p95={}ms", metrics.latency_ms, _p95),
            ).await;

            // Update immediate metrics with computed heuristic breakdown
            state.network.latency_ms = metrics.latency_ms;
            state.network.breakdown = breakdown_numeric;
            state.network.last_http_status = metrics.last_http_status;
            state.network.error_type = metrics.error_type.clone();
            state.network.breakdown_source = Some(breakdown_source.to_string());
        }

        #[cfg(feature = "timings-curl")]
        {
            // Use measured timing breakdown directly from curl
            state.network.latency_ms = metrics.latency_ms;
            state.network.breakdown = metrics.breakdown.clone();
            state.network.last_http_status = metrics.last_http_status;
            state.network.error_type = metrics.error_type.clone();
            
            // Parse DNS timing from breakdown to determine connection reuse  
            let dns_reused = if let Some(dns_part) = metrics.breakdown.split('|').next() {
                if let Some(dns_ms_str) = dns_part.strip_prefix("DNS:").and_then(|s| s.strip_suffix("ms")) {
                    // Treat DNS <= 2ms as reused connection to account for timing precision
                    dns_ms_str.parse::<u32>().unwrap_or(0) <= 2
                } else {
                    false
                }
            } else {
                false
            };
            
            state.network.connection_reused = Some(dns_reused);
            state.network.breakdown_source = Some(breakdown_source.to_string());
        }
        state.timestamp = self.clock.local_timestamp();

        // Update API config
        state.api_config = Some(ApiConfig {
            endpoint: format!("{}/v1/messages", creds.base_url),
            source: creds.source.to_string(),
        });
        state.monitoring_enabled = true;

        // Proxy health check implementation
        if Self::is_official_base_url(&creds.base_url) {
            // Official Anthropic API - skip proxy health check
            state.network.proxy_healthy = None;
        } else {
            // Proxy endpoint - perform health check using GET method with JSON validation
            match self.check_proxy_health_internal(&creds.base_url).await {
                Ok(health_status) => state.network.proxy_healthy = health_status,
                Err(_) => state.network.proxy_healthy = Some(false), // Health check errors treated as unhealthy
            }
        }

        // Mode-specific processing
        let (final_status, p95_updated, rolling_len) = match mode {
            ProbeMode::Red => {
                // RED mode: Set status=error, update error event, don't touch rolling stats
                if let Some(mut error_event) = last_jsonl_error_event {
                    // Convert UTC timestamp to local time for consistent persistence
                    error_event.timestamp = Self::convert_utc_to_local_timestamp(&error_event.timestamp)
                        .unwrap_or_else(|_| self.clock.local_timestamp());
                    state.last_jsonl_error_event = Some(error_event);
                }
                state.status = NetworkStatus::Error;
                state.monitoring_state.state = NetworkStatus::Error;
                let rolling_len = state.network.rolling_totals.len();
                (
                    NetworkStatus::Error,
                    state.network.p95_latency_ms,
                    rolling_len,
                )
            }
            ProbeMode::Green | ProbeMode::Cold => {
                // GREEN/COLD: Update rolling stats if HTTP 200, determine status by thresholds
                let (status, p95, rolling_len) = if metrics.last_http_status == 200 {
                    // Add to rolling totals and recalculate P95
                    state.network.rolling_totals.push(metrics.latency_ms);
                    if state.network.rolling_totals.len() > 12 {
                        state.network.rolling_totals.remove(0);
                    }

                    let new_p95 = self.calculate_p95(&state.network.rolling_totals);
                    state.network.p95_latency_ms = new_p95;

                    // Determine status based on P80/P95 thresholds
                    let p80 = self.calculate_p80(&state.network.rolling_totals);
                    let status = if metrics.latency_ms <= p80 {
                        NetworkStatus::Healthy
                    } else if metrics.latency_ms <= new_p95 {
                        NetworkStatus::Degraded
                    } else {
                        NetworkStatus::Error
                    };

                    (status, new_p95, state.network.rolling_totals.len())
                } else if metrics.last_http_status == 429 {
                    (
                        NetworkStatus::Degraded,
                        state.network.p95_latency_ms,
                        state.network.rolling_totals.len(),
                    )
                } else {
                    (
                        NetworkStatus::Error,
                        state.network.p95_latency_ms,
                        state.network.rolling_totals.len(),
                    )
                };

                state.status = status.clone();
                state.monitoring_state.state = status.clone();

                // COLD mode: Update session deduplication fields
                if mode == ProbeMode::Cold {
                    if let Some(ref session_id) = self.current_session_id {
                        state.monitoring_state.last_cold_session_id = Some(session_id.clone());
                        state.monitoring_state.last_cold_probe_at = Some(self.clock.local_timestamp());
                    }
                }

                (status, p95, rolling_len)
            }
        };

        // Write state atomically
        self.write_state_atomic(&state).await?;

        // Build outcome
        let outcome = ProbeOutcome {
            status: final_status,
            metrics,
            p95_latency_ms: p95_updated,
            rolling_len,
            api_config: state.api_config.unwrap_or_default(),
            mode,
            state_written: true,
            timestamp_local: state.timestamp,
        };

        Ok(outcome)
    }

    /// Calculate 95th percentile from rolling samples using nearest-rank method
    fn calculate_p95(&self, samples: &[u32]) -> u32 {
        if samples.is_empty() {
            return 0;
        }

        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();

        // Use nearest-rank (inclusive) method: ceil(p * n) - 1
        let rank = ((0.95 * n as f64).ceil() as usize).max(1); // 1-based rank
        sorted[rank - 1] // Convert to 0-based index
    }

    /// Calculate 80th percentile from rolling samples using nearest-rank method
    fn calculate_p80(&self, samples: &[u32]) -> u32 {
        if samples.is_empty() {
            return 0;
        }

        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();

        // Use nearest-rank (inclusive) method: ceil(p * n) - 1
        let rank = ((0.80 * n as f64).ceil() as usize).max(1); // 1-based rank
        sorted[rank - 1] // Convert to 0-based index
    }

    /// Load monitoring state from file (internal)
    async fn load_state_internal(&self) -> Result<MonitoringSnapshot, NetworkError> {
        if !self.state_path.exists() {
            return Ok(MonitoringSnapshot {
                status: NetworkStatus::Unknown,
                monitoring_enabled: false,
                api_config: None,
                network: NetworkMetrics::default(),
                monitoring_state: MonitoringState::default(),
                last_jsonl_error_event: None,
                timestamp: self.clock.local_timestamp(),
            });
        }

        let content = tokio::fs::read_to_string(&self.state_path)
            .await
            .map_err(|e| {
                NetworkError::StateFileError(format!("Failed to read state file: {}", e))
            })?;

        let state: MonitoringSnapshot = serde_json::from_str(&content).map_err(|e| {
            NetworkError::StateFileError(format!("Failed to parse state file: {}", e))
        })?;

        Ok(state)
    }

    /// Write state atomically using temp file + rename
    async fn write_state_atomic(&self, state: &MonitoringSnapshot) -> Result<(), NetworkError> {
        // Ensure directory exists
        if let Some(parent) = self.state_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                NetworkError::StateFileError(format!("Failed to create directory: {}", e))
            })?;
        }

        // Write to temporary file
        let temp_path = self.state_path.with_extension("tmp");
        let content = serde_json::to_string_pretty(state).map_err(|e| {
            NetworkError::StateFileError(format!("Failed to serialize state: {}", e))
        })?;

        tokio::fs::write(&temp_path, content).await.map_err(|e| {
            NetworkError::StateFileError(format!("Failed to write temp file: {}", e))
        })?;

        // Atomic rename
        tokio::fs::rename(&temp_path, &self.state_path)
            .await
            .map_err(|e| {
                NetworkError::StateFileError(format!("Failed to rename temp file: {}", e))
            })?;

        Ok(())
    }

    // URL utility functions for proxy health checking

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
        let normalized = Self::normalize_base_url(base_url);
        normalized.eq_ignore_ascii_case("https://api.anthropic.com")
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

    /// Build health check URL from base URL
    ///
    /// # Arguments
    /// * `base_url` - The base URL to append /health to
    ///
    /// # Returns
    /// * Complete health check URL
    pub fn build_health_url(base_url: &str) -> String {
        let normalized = Self::normalize_base_url(base_url);
        format!("{}/health", normalized)
    }

    /// Validate health check JSON response body
    ///
    /// Validates that the response contains `{"status": "healthy"}` (case-insensitive).
    /// Tolerates extra fields and flexible JSON formatting.
    ///
    /// # Arguments
    /// * `body` - Response body bytes to validate
    ///
    /// # Returns
    /// * `true` - Valid JSON with status=healthy (case-insensitive)
    /// * `false` - Invalid JSON, missing status field, or status!=healthy
    pub(crate) fn validate_health_json(&self, body: &[u8]) -> bool {
        // Parse JSON response
        let json_value: serde_json::Value = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return false, // Invalid JSON
        };

        // Check if it's an object with a status field
        let obj = match json_value.as_object() {
            Some(obj) => obj,
            None => return false, // Not a JSON object
        };

        // Get status field value (case-insensitive field name)
        let status = obj.iter()
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

    /// Check proxy health endpoint using internal health check client
    ///
    /// MEDIUM Priority Fix: Cleaner API design using stored health client
    /// This method uses the HttpMonitor's internal health_client, providing
    /// a simpler interface for callers and reducing parameter passing complexity.
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the proxy to check
    ///
    /// # Returns
    /// * `Ok(Some(true))` - Proxy is healthy (200 + valid JSON)
    /// * `Ok(Some(false))` - Proxy is unhealthy (non-200, invalid JSON, network error)
    /// * `Ok(None)` - No health endpoint (404 response)
    /// * `Err(NetworkError)` - Internal error (should be mapped to Some(false) by caller)
    #[cfg(feature = "network-monitoring")]
    pub async fn check_proxy_health_internal(&self, base_url: &str) -> Result<Option<bool>, NetworkError> {
        self.check_proxy_health_with_client(base_url, &*self.health_client).await
    }

    /// Check proxy health endpoint using dedicated health check client
    ///
    /// Performs a GET request to `<base_url>/health` with 1500ms timeout.
    /// Uses dedicated HealthCheckClient for proper GET method and redirect handling.
    /// Validates JSON response with `{"status": "healthy"}` (case-insensitive).
    ///
    /// # Arguments
    /// * `base_url` - Base URL of the proxy to check
    /// * `health_client` - Dedicated health check client for GET requests
    ///
    /// # Returns
    /// * `Ok(Some(true))` - Proxy is healthy (200 + valid JSON)
    /// * `Ok(Some(false))` - Proxy is unhealthy (non-200, invalid JSON, network error)
    /// * `Ok(None)` - No health endpoint (404 response)
    /// * `Err(NetworkError)` - Internal error (should be mapped to Some(false) by caller)
    #[cfg(feature = "network-monitoring")]
    pub async fn check_proxy_health_with_client(
        &self,
        base_url: &str,
        health_client: &dyn HealthCheckClient,
    ) -> Result<Option<bool>, NetworkError> {
        let health_url = Self::build_health_url(base_url);
        let timeout_ms = 1500u32;

        match health_client.get_health(health_url, timeout_ms).await {
            Ok(response) => {
                match response.status_code {
                    404 => Ok(None), // No health endpoint available
                    200 => {
                        // Validate JSON body content
                        let is_healthy = self.validate_health_json(&response.body);
                        Ok(Some(is_healthy))
                    }
                    _ => Ok(Some(false)), // Any other HTTP status is unhealthy
                }
            }
            Err(_) => Ok(Some(false)), // Network errors are treated as unhealthy
        }
    }

    /// **DEPRECATED** Legacy proxy health check method
    ///
    /// **Use `check_proxy_health_internal()` instead** for new code.
    ///
    /// This method uses the legacy HttpClientTrait interface which:
    /// - Uses generic POST-based requests (not dedicated GET)
    /// - Lacks response body access for JSON validation
    /// - Does not enforce redirect policy explicitly
    /// 
    /// This method is kept only for backward compatibility and will be removed
    /// in a future version. All new code should use `check_proxy_health_internal()`
    /// which provides proper GET method, JSON validation, and redirect control.
    /// 
    /// # Arguments
    /// * `base_url` - Base URL of the proxy to check
    ///
    /// # Returns
    /// * `Ok(Some(true))` - Proxy responds with 200 (limited validation)
    /// * `Ok(Some(false))` - Proxy is unhealthy (non-200, network error)
    /// * `Ok(None)` - No health endpoint (404 response)
    #[cfg(feature = "network-monitoring")]
    #[deprecated(since = "1.0.81", note = "Use `check_proxy_health_internal()` instead")]
    pub async fn check_proxy_health(&self, base_url: &str) -> Result<Option<bool>, NetworkError> {
        let health_url = Self::build_health_url(base_url);
        let timeout_ms = 1500u32;

        // WARNING: Uses generic HTTP client with POST-like semantics (legacy behavior)
        // This does not guarantee GET method or provide response body access
        let mut headers = std::collections::HashMap::new();
        headers.insert("User-Agent".to_string(), "claude-cli/1.0.80 (external, cli)".to_string());
        
        let empty_body = Vec::new();

        match self.http_client.execute_request(health_url, headers, empty_body, timeout_ms).await {
            Ok((status, _duration, _breakdown)) => {
                match status {
                    404 => Ok(None), // No health endpoint
                    200 => {
                        // LIMITATION: Cannot validate JSON response body due to HttpClientTrait design
                        // Only status code validation is possible with this legacy method
                        Ok(Some(true))
                    }
                    _ => Ok(Some(false)), // Any other status is unhealthy
                }
            }
            Err(_) => Ok(Some(false)), // Network errors are treated as unhealthy
        }
    }

    /// Check proxy health endpoint using internal health check client (mock version)
    #[cfg(not(feature = "network-monitoring"))]
    pub async fn check_proxy_health_internal(&self, _base_url: &str) -> Result<Option<bool>, NetworkError> {
        Ok(None) // Always return None when monitoring is disabled
    }

    /// Mock proxy health check for when network-monitoring feature is disabled
    #[cfg(not(feature = "network-monitoring"))]
    async fn check_proxy_health(&self, _base_url: &str) -> Result<Option<bool>, NetworkError> {
        Ok(None) // Always return None when monitoring is disabled
    }
}

// Proxy health check implementation complete

/// Default mock implementation when network-monitoring feature is disabled
#[cfg(not(feature = "network-monitoring"))]
#[derive(Default)]
pub struct MockHttpClient;

#[cfg(not(feature = "network-monitoring"))]
#[async_trait::async_trait]
impl HttpClientTrait for MockHttpClient {
    async fn execute_request(
        &self,
        _url: String,
        _headers: std::collections::HashMap<String, String>,
        _body: Vec<u8>,
        _timeout_ms: u32,
    ) -> Result<(u16, Duration, String), String> {
        // Return successful mock response
        let duration = Duration::from_millis(1000);
        let breakdown = format!("DNS:10ms|TCP:20ms|TLS:30ms|TTFB:940ms|Total:1000ms");
        Ok((200, duration, breakdown))
    }
}

/// Mock health check client implementation when network-monitoring feature is disabled
#[cfg(not(feature = "network-monitoring"))]
#[derive(Default)]
pub struct MockHealthCheckClient;

#[cfg(not(feature = "network-monitoring"))]
#[async_trait::async_trait]
impl HealthCheckClient for MockHealthCheckClient {
    async fn get_health(
        &self,
        _url: String,
        _timeout_ms: u32,
    ) -> Result<HealthResponse, String> {
        // Return mock healthy response
        let duration = Duration::from_millis(200);
        let body = r#"{"status": "healthy"}"#.as_bytes().to_vec();
        Ok(HealthResponse {
            status_code: 200,
            body,
            duration,
        })
    }
}
