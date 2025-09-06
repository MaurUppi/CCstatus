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
use crate::core::network::oauth_masquerade::{
    run_probe as oauth_run_probe, OauthMasqueradeOptions,
};
use crate::core::network::proxy_health::{
    assess_proxy_health, build_messages_endpoint, HealthCheckClient, ProxyHealthOptions,
};
use serde_json;

#[cfg(all(feature = "network-monitoring", feature = "timings-curl"))]
use crate::core::network::proxy_health::CurlHealthCheckClient;

#[cfg(all(feature = "network-monitoring", not(feature = "timings-curl")))]
use crate::core::network::proxy_health::IsahcHealthCheckClient;

#[cfg(not(feature = "network-monitoring"))]
use crate::core::network::proxy_health::MockHealthCheckClient;
use crate::core::network::types::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[cfg(feature = "network-monitoring")]
use isahc::config::Configurable;
#[cfg(feature = "network-monitoring")]
use isahc::{HttpClient, Request};

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
    pub ttfb_ms: u32,       // ServerTTFB (isolated server processing time)
    pub total_ttfb_ms: u32, // TotalTTFB (end-to-end first byte time)
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
    /// Returns (status_code, duration, breakdown_string, response_headers, http_version)
    async fn execute_request(
        &self,
        url: String,
        headers: std::collections::HashMap<String, String>,
        body: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<
        (
            u16,
            Duration,
            String,
            std::collections::HashMap<String, String>,
            Option<String>,
        ),
        String,
    >;
}

// HealthCheckClient and HealthResponse are now imported from proxy_health module

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
    ) -> Result<
        (
            u16,
            Duration,
            String,
            std::collections::HashMap<String, String>,
            Option<String>,
        ),
        String,
    > {
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

        // Capture HTTP version for diagnostics
        let http_version = match response.version() {
            isahc::http::Version::HTTP_09 => Some("HTTP/0.9".to_string()),
            isahc::http::Version::HTTP_10 => Some("HTTP/1.0".to_string()),
            isahc::http::Version::HTTP_11 => Some("HTTP/1.1".to_string()),
            isahc::http::Version::HTTP_2 => Some("HTTP/2.0".to_string()),
            isahc::http::Version::HTTP_3 => Some("HTTP/3.0".to_string()),
            _ => None, // Unknown version
        };

        // Capture response headers for Cloudflare bot challenge detection
        let mut response_headers = std::collections::HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                response_headers.insert(name.to_string(), value_str.to_string());
            }
        }

        // Drain response body without allocating (zero-copy to sink)
        let mut body = response.into_body();
        let _ = copy(&mut body, &mut sink())
            .await
            .map_err(|e| format!("Failed to drain response body: {}", e))?;

        // Generate timing breakdown with stable numeric format
        let total_ms = ttfb_duration.as_millis() as u32;

        // Simplified breakdown format for isahc - just Total time
        let breakdown = format!("Total:{}ms", total_ms);

        Ok((
            status,
            ttfb_duration,
            breakdown,
            response_headers,
            http_version,
        ))
    }
}

#[cfg(feature = "network-monitoring")]
impl IsahcHttpClient {
    pub fn new() -> Result<Self, NetworkError> {
        let client = HttpClient::builder()
            .cookies() // Enable in-memory cookie store for session continuity
            .build()
            .map_err(|e| NetworkError::HttpError(format!("Failed to create HTTP client: {}", e)))?;
        Ok(Self { client })
    }
}

// IsahcHealthCheckClient is now provided by the proxy_health module

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
        let headers = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<Vec<_>>();
        let body = body.to_vec();

        let result = tokio::task::spawn_blocking(move || -> Result<PhaseTimings, String> {
            let mut handle = Easy::new();

            // Configure request
            handle
                .url(&url)
                .map_err(|e| format!("URL set failed: {}", e))?;
            handle
                .post(true)
                .map_err(|e| format!("POST set failed: {}", e))?;
            handle
                .post_fields_copy(&body)
                .map_err(|e| format!("POST fields failed: {}", e))?;
            handle
                .timeout(std::time::Duration::from_millis(timeout_ms as u64))
                .map_err(|e| format!("Timeout set failed: {}", e))?;

            // Bot-fight protocol enhancements
            handle
                .http_version(curl::easy::HttpVersion::V2TLS)
                .map_err(|e| format!("HTTP/2 version failed: {}", e))?;
            handle
                .accept_encoding("gzip, deflate, br")
                .map_err(|e| format!("Accept-Encoding failed: {}", e))?;
            handle
                .useragent("claude-cli/1.0.93 (external, cli)")
                .map_err(|e| format!("User-Agent failed: {}", e))?;

            // Enable cookie engine for session continuity
            handle
                .cookie_file("")
                .map_err(|e| format!("Cookie engine failed: {}", e))?;

            // Set headers
            let mut header_list = curl::easy::List::new();
            for (key, value) in headers {
                header_list
                    .append(&format!("{}: {}", key, value))
                    .map_err(|e| format!("Header append failed: {}", e))?;
            }
            handle
                .http_headers(header_list)
                .map_err(|e| format!("Headers set failed: {}", e))?;

            // Capture response body but don't store it
            handle
                .write_function(|data| {
                    // Drain response body without storing
                    Ok(data.len())
                })
                .map_err(|e| format!("Write function failed: {}", e))?;

            // Execute request and capture timings
            handle
                .perform()
                .map_err(|e| format!("Request perform failed: {}", e))?;

            // Extract phase timings from libcurl (in seconds, convert to ms)
            let dns_time = handle
                .namelookup_time()
                .map_err(|e| format!("DNS time failed: {}", e))?
                .as_secs_f64();
            let connect_time = handle
                .connect_time()
                .map_err(|e| format!("Connect time failed: {}", e))?
                .as_secs_f64();
            let appconnect_time = handle
                .appconnect_time()
                .map_err(|e| format!("App connect time failed: {}", e))?
                .as_secs_f64();
            let starttransfer_time = handle
                .starttransfer_time()
                .map_err(|e| format!("Start transfer time failed: {}", e))?
                .as_secs_f64();
            let total_time = handle
                .total_time()
                .map_err(|e| format!("Total time failed: {}", e))?
                .as_secs_f64();

            // Calculate phase durations and convert to milliseconds
            let dns_ms = (dns_time * 1000.0).max(0.0) as u32;
            let tcp_ms = ((connect_time - dns_time).max(0.0) * 1000.0) as u32;
            let tls_ms = ((appconnect_time - connect_time).max(0.0) * 1000.0) as u32;
            let ttfb_ms = ((starttransfer_time - appconnect_time).max(0.0) * 1000.0) as u32;
            let total_ttfb_ms = (starttransfer_time * 1000.0).max(0.0) as u32; // End-to-end TTFB
            let total_ms = (total_time * 1000.0).max(0.0) as u32;

            // Get response status
            let status = handle
                .response_code()
                .map_err(|e| format!("Response code failed: {}", e))?
                as u16;

            Ok(PhaseTimings {
                status,
                dns_ms,
                tcp_ms,
                tls_ms,
                ttfb_ms,
                total_ttfb_ms,
                total_ms,
            })
        })
        .await
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

        // Health check client selection: prefer curl for enhanced timing when available
        #[cfg(all(feature = "network-monitoring", feature = "timings-curl"))]
        let health_client: Box<dyn HealthCheckClient> = Box::new(CurlHealthCheckClient::new()?);
        #[cfg(all(feature = "network-monitoring", not(feature = "timings-curl")))]
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

    /// Disable curl runner for testing (forces use of HTTP client mock)
    #[cfg(feature = "timings-curl")]
    pub fn without_curl_runner(mut self) -> Self {
        self.curl_runner = None;
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
    async fn set_last_window_id(
        &self,
        color: WindowColor,
        window_id: u64,
    ) -> Result<(), NetworkError> {
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

        let (status_code, latency_ms, breakdown, error_type, http_version) = match probe_result {
            Ok((status, duration, breakdown, response_headers, http_version)) => {
                let error_type = self.classify_http_error(status, &response_headers);
                (
                    status,
                    duration.as_millis() as u32,
                    breakdown,
                    error_type,
                    http_version,
                )
            }
            Err(NetworkError::SkipProbe(skip_reason)) => {
                // OAuth token expired - silently skip probe and return previous state unchanged
                debug_logger
                    .debug("HttpMonitor", &format!("Probe skipped: {}", skip_reason))
                    .await;

                let state = self.load_state().await?;
                let outcome = ProbeOutcome {
                    status: state.status,
                    metrics: ProbeMetrics {
                        latency_ms: state.network.latency_ms,
                        breakdown: state.network.breakdown,
                        last_http_status: state.network.last_http_status,
                        error_type: state.network.error_type,
                        http_version: state.network.http_version,
                    },
                    p95_latency_ms: state.network.p95_latency_ms,
                    rolling_len: state.network.rolling_totals.len(),
                    api_config: state.api_config.unwrap_or_default(),
                    mode,
                    state_written: false, // No state was written since we skipped
                    timestamp_local: state.timestamp,
                };

                debug_logger.network_probe_end(
                    &format!("{:?}", mode),
                    None, // No status code for skipped probe
                    0,    // No latency for skipped probe
                    probe_id,
                );

                return Ok(outcome);
            }
            Err(err) => {
                debug_logger
                    .error("HttpMonitor", &format!("Probe failed: {}", err))
                    .await;

                let elapsed_ms = probe_start.elapsed().as_millis();

                // Connection error breakdown - format based on feature
                #[cfg(feature = "timings-curl")]
                let breakdown = format!(
                    "DNS:0ms|TCP:0ms|TLS:0ms|ServerTTFB:0ms/TotalTTFB:0ms|Total:{}ms",
                    elapsed_ms
                );

                #[cfg(not(feature = "timings-curl"))]
                let breakdown = format!("Total:{}ms", elapsed_ms);

                (
                    0,
                    elapsed_ms as u32,
                    breakdown,
                    Some("connection_error".to_string()),
                    None, // No HTTP version available for connection errors
                )
            }
        };

        // Build probe metrics
        let metrics = ProbeMetrics {
            latency_ms,
            breakdown: breakdown.clone(),
            last_http_status: status_code,
            error_type: error_type.clone(),
            http_version: http_version.clone(),
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
        use chrono::{DateTime, Local, Utc};

        let utc_dt: DateTime<Utc> = utc_timestamp
            .parse()
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
    /// Uses OAuth masquerade for OAuth credentials when unexpired, otherwise uses x-api-key flow.
    /// For x-api-key: Uses curl-based probe for detailed phase timings when timings-curl feature
    /// is enabled (auto-wired by default, can be overridden). Falls back to isahc-based probe
    /// on curl failures or when no runner is available.
    async fn execute_http_probe(
        &self,
        creds: &ApiCredentials,
        timeout_ms: u32,
        _probe_start: Instant,
    ) -> Result<
        (
            u16,
            Duration,
            String,
            std::collections::HashMap<String, String>,
            Option<String>,
        ),
        NetworkError,
    > {
        // Path selection: OAuth masquerade vs x-api-key flow
        if creds.source == CredentialSource::OAuth {
            // Check if token is expired (hard gate)
            if let Some(expires_at) = creds.expires_at {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;

                if expires_at <= now_ms {
                    // Token expired - return error immediately without probe
                    let debug_logger = get_debug_logger();
                    let _ = debug_logger
                        .debug(
                            "HttpMonitor",
                            &format!(
                                "OAuth token expired: now={} expires_at={}",
                                now_ms, expires_at
                            ),
                        )
                        .await;

                    return Err(NetworkError::SkipProbe("OAuth token expired".to_string()));
                }
            }

            // OAuth masquerade path
            let oauth_opts = OauthMasqueradeOptions {
                base_url: creds.base_url.clone(),
                access_token: creds.auth_token.clone(),
                expires_at: creds.expires_at,
                stream: false, // No streaming for probe requests
            };

            #[cfg(feature = "timings-curl")]
            let result = oauth_run_probe(
                &oauth_opts,
                self.http_client.as_ref(),
                self.curl_runner.as_ref().map(|r| r.as_ref()),
            )
            .await;

            #[cfg(not(feature = "timings-curl"))]
            let result = oauth_run_probe(&oauth_opts, self.http_client.as_ref()).await;

            return match result {
                Ok(result) => {
                    let duration = Duration::from_millis(result.duration_ms as u64);
                    Ok((
                        result.status,
                        duration,
                        result.breakdown,
                        result.response_headers,
                        result.http_version,
                    ))
                }
                Err(e) => {
                    let debug_logger = get_debug_logger();
                    let _ = debug_logger
                        .error(
                            "HttpMonitor",
                            &format!("OAuth masquerade probe failed: {}", e),
                        )
                        .await;

                    // For now, return the error rather than falling back to x-api-key
                    // (OAuth-only environments should not have x-api-key as fallback)
                    Err(e)
                }
            };
        }

        // x-api-key flow (existing implementation)
        // Check if curl runner is available for detailed timing measurements
        #[cfg(feature = "timings-curl")]
        if let Some(ref curl_runner) = self.curl_runner {
            let endpoint = build_messages_endpoint(&creds.base_url);

            // Minimal Claude API payload for probing
            let payload = serde_json::json!({
                "model": "claude-3-5-haiku-20241022",
                "max_tokens": 1,
                "messages": [
                    {"role": "user", "content": "Hi"}
                ]
            });

            let body = serde_json::to_vec(&payload).map_err(|e| {
                NetworkError::HttpError(format!("Payload serialization failed: {}", e))
            })?;

            let headers = vec![
                ("Content-Type", "application/json".to_string()),
                ("x-api-key", creds.auth_token.clone()),
                (
                    "User-Agent",
                    "claude-cli/1.0.93 (external, cli)".to_string(),
                ),
                ("anthropic-version", "2023-06-01".to_string()),
                // Bot-fight mitigation headers
                ("Accept", "application/json".to_string()),
                ("Accept-Encoding", "gzip, deflate, br".to_string()),
            ];

            // Try curl first, fallback to isahc on failure for resiliency
            match curl_runner
                .run(&endpoint, &headers, &body, timeout_ms)
                .await
            {
                Ok(phase_timings) => {
                    let duration = Duration::from_millis(phase_timings.ttfb_ms as u64);

                    // Load current state to get P80 threshold for network performance check
                    let temp_state = self.load_state_internal().await.unwrap_or_default();
                    let p80 = self.calculate_p80(&temp_state.network.rolling_totals);

                    // Check both HTTP errors AND network performance degradation
                    let is_degraded_or_error = phase_timings.status >= 400
                        || phase_timings.status == 0
                        || phase_timings.ttfb_ms > p80;

                    let breakdown = if is_degraded_or_error {
                        format!(
                            "DNS:{}ms|TCP:{}ms|TLS:{}ms|ServerTTFB:{}ms/TotalTTFB:{}ms|Total:{}ms",
                            phase_timings.dns_ms,
                            phase_timings.tcp_ms,
                            phase_timings.tls_ms,
                            phase_timings.ttfb_ms,
                            phase_timings.total_ttfb_ms,
                            phase_timings.total_ms
                        )
                    } else {
                        format!(
                            "DNS:{}ms|TCP:{}ms|TLS:{}ms|TTFB:{}ms|Total:{}ms",
                            phase_timings.dns_ms,
                            phase_timings.tcp_ms,
                            phase_timings.tls_ms,
                            phase_timings.ttfb_ms,
                            phase_timings.total_ms
                        )
                    };

                    // Note: curl branch doesn't capture response headers or HTTP version in current implementation
                    // Setting http_version=None to avoid misleading diagnostics about version negotiation
                    let empty_headers = std::collections::HashMap::new();
                    let http_version = None; // Unknown version - curl implementation doesn't capture this
                    return Ok((
                        phase_timings.status,
                        duration,
                        breakdown,
                        empty_headers,
                        http_version,
                    ));
                }
                Err(curl_error) => {
                    // Log curl failure and fallback to isahc for resiliency
                    let debug_logger = get_debug_logger();
                    let _ = debug_logger
                        .error(
                            "HttpMonitor",
                            &format!("Curl probe failed, falling back to isahc: {}", curl_error),
                        )
                        .await;
                    // Fall through to isahc path below
                }
            }
        }

        // Fallback to isahc-based probe with heuristic timing breakdown
        let endpoint = build_messages_endpoint(&creds.base_url);

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
        headers.insert(
            "User-Agent".to_string(),
            "claude-cli/1.0.93 (external, cli)".to_string(),
        );
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());
        // Bot-fight mitigation headers
        headers.insert("Accept".to_string(), "application/json".to_string());
        headers.insert(
            "Accept-Encoding".to_string(),
            "gzip, deflate, br".to_string(),
        );

        let (status_code, duration, breakdown, response_headers, http_version) = self
            .http_client
            .execute_request(endpoint, headers, body, timeout_ms)
            .await
            .map_err(NetworkError::HttpError)?;

        Ok((
            status_code,
            duration,
            breakdown,
            response_headers,
            http_version,
        ))
    }

    /// Classify HTTP status codes into standard error types
    /// Enhanced Phase 2 bot challenge detection with header analysis for 429 responses
    ///
    /// Uses detect_cloudflare_challenge for comprehensive header-based CF detection on 429.
    /// GET detection uses comprehensive header/body analysis via detect_cloudflare_challenge.
    fn classify_http_error(
        &self,
        status_code: u16,
        response_headers: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        match status_code {
            200..=299 => None, // Success
            0 => Some("connection_error".to_string()),
            400 => Some("invalid_request_error".to_string()),
            401 => Some("authentication_error".to_string()),
            403 => {
                // 403 is highly likely to be a Cloudflare bot challenge for API endpoints
                // Phase 2: Enhanced heuristic - 403 on /v1/messages is almost always CF
                Some("bot_challenge".to_string())
            }
            404 => Some("not_found_error".to_string()),
            413 => Some("request_too_large".to_string()),
            429 => {
                // 429 can be rate limiting OR bot challenge depending on context
                // Phase 2 enhancement: Use header analysis to detect Cloudflare challenges
                use crate::core::network::proxy_health::parsing::detect_cloudflare_challenge;

                if detect_cloudflare_challenge(429, response_headers, &[]) {
                    Some("bot_challenge".to_string())
                } else {
                    // No CF indicators - treat as legitimate rate limit
                    Some("rate_limit_error".to_string())
                }
            }
            500 => Some("api_error".to_string()),
            503 => {
                // 503 Service Unavailable commonly used by CF for bot challenges
                // Phase 2: Enhanced heuristic - 503 on API endpoints often indicates CF challenge
                Some("bot_challenge".to_string())
            }
            504 => Some("socket_hang_up".to_string()),
            529 => Some("overloaded_error".to_string()),
            402 | 405..=412 | 414..=428 | 430..=499 => Some("client_error".to_string()),
            501..=502 | 505..=528 | 530..=599 => Some("server_error".to_string()),
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
            // Isahc path: use simplified Total format (already set in isahc execute_request)
            let debug_logger = get_debug_logger();
            debug_logger
                .debug(
                    "HttpMonitor",
                    &format!(
                        "heuristic_breakdown: total={}ms p95={}ms",
                        metrics.latency_ms, _p95
                    ),
                )
                .await;

            // Update immediate metrics with isahc breakdown (already in Total:Xms format)
            state.network.latency_ms = metrics.latency_ms;
            state.network.breakdown = metrics.breakdown.clone();
            state.network.last_http_status = metrics.last_http_status;
            state.network.error_type = metrics.error_type.clone();
            state.network.breakdown_source = Some(breakdown_source.to_string());
            state.network.http_version = metrics.http_version.clone();
        }

        #[cfg(feature = "timings-curl")]
        {
            // Use measured timing breakdown directly from curl
            state.network.latency_ms = metrics.latency_ms;
            state.network.breakdown = metrics.breakdown.clone();
            state.network.last_http_status = metrics.last_http_status;
            state.network.error_type = metrics.error_type.clone();
            state.network.http_version = metrics.http_version.clone();

            // Parse DNS timing from breakdown to determine connection reuse
            let dns_reused = if let Some(dns_part) = metrics.breakdown.split('|').next() {
                if let Some(dns_ms_str) = dns_part
                    .strip_prefix("DNS:")
                    .and_then(|s| s.strip_suffix("ms"))
                {
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
            endpoint: build_messages_endpoint(&creds.base_url),
            source: creds.source.to_string(),
        });
        state.monitoring_enabled = true;

        // Proxy health check using new proxy_health module
        // Skip proxy health check in OAuth mode per development plan
        if creds.source == CredentialSource::OAuth {
            // OAuth mode: skip proxy health check and set fields to None
            state.network.set_proxy_health(None, None);
        } else {
            // Non-OAuth mode: perform proxy health check as usual
            let proxy_health_options = ProxyHealthOptions {
                use_root_urls: true, // Enhanced mode: try root-based URLs first
                try_fallback: true,
                follow_redirect_once: true, // Enable safe same-host redirect following
                timeout_ms: 1500,
            };

            let proxy_health_outcome =
                assess_proxy_health(&creds.base_url, &proxy_health_options, &*self.health_client)
                    .await;

            // Use centralized mapping function to set both legacy and new fields
            match proxy_health_outcome {
                Ok(outcome) => {
                    state
                        .network
                        .set_proxy_health(outcome.level, outcome.detail);
                }
                Err(_) => {
                    // Health check errors: no proxy detected or internal error
                    state.network.set_proxy_health(None, None);
                }
            }
        }

        // Mode-specific processing
        let (final_status, p95_updated, rolling_len) = match mode {
            ProbeMode::Red => {
                // RED mode: Set status=error, update error event, don't touch rolling stats
                if let Some(mut error_event) = last_jsonl_error_event {
                    // Convert UTC timestamp to local time for consistent persistence
                    error_event.timestamp =
                        Self::convert_utc_to_local_timestamp(&error_event.timestamp)
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
                // GREEN/COLD: Update rolling stats ONLY if HTTP 200 AND no bot challenge
                let is_bot_blocked =
                    metrics.error_type.as_ref() == Some(&"bot_challenge".to_string());

                let (status, p95, rolling_len) =
                    if metrics.last_http_status == 200 && !is_bot_blocked {
                        // Safe to add to rolling statistics - HTTP 200 with no bot challenge
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
                    } else if metrics.last_http_status == 429 && !is_bot_blocked {
                        // Rate limited but not bot blocked - degraded status
                        (
                            NetworkStatus::Degraded,
                            state.network.p95_latency_ms,
                            state.network.rolling_totals.len(),
                        )
                    } else {
                        // Bot blocked or error - don't contaminate stats
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
                        state.monitoring_state.last_cold_probe_at =
                            Some(self.clock.local_timestamp());
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

// MockHealthCheckClient is now provided by the proxy_health module
