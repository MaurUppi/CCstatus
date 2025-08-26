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
use isahc::config::Configurable;
#[cfg(feature = "network-monitoring")]
use isahc::{AsyncReadResponseExt, HttpClient, Request};

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

        let mut response = self
            .client
            .send_async(request)
            .await
            .map_err(|e| format!("Request failed: {}", e))?;
        let ttfb_duration = start.elapsed();

        let status = response.status().as_u16();

        // Consume response body to complete the request
        let _ = response.text().await.unwrap_or_default();

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
    /// Clock for timing operations
    clock: Box<dyn ClockTrait>,
    /// Optional timeout override for testing
    timeout_override_ms: Option<u32>,
    /// Current session ID for COLD probe deduplication
    current_session_id: Option<String>,
}

impl HttpMonitor {
    /// Create new HttpMonitor with default configuration
    ///
    /// Uses default state path: `~/.claude/ccstatus/ccstatus-monitoring.json`
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

        Ok(Self {
            state_path,
            http_client,
            clock: Box::new(SystemClock),
            timeout_override_ms: None,
            current_session_id: None,
        })
    }

    /// Configure HttpMonitor with custom HTTP client (for testing)
    pub fn with_http_client(mut self, client: Box<dyn HttpClientTrait>) -> Self {
        self.http_client = client;
        self
    }

    /// Configure HttpMonitor with custom clock (for testing)
    pub fn with_clock(mut self, clock: Box<dyn ClockTrait>) -> Self {
        self.clock = clock;
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
    async fn execute_http_probe(
        &self,
        creds: &ApiCredentials,
        timeout_ms: u32,
        _probe_start: Instant,
    ) -> Result<(u16, Duration, String), NetworkError> {
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

        // Connection reuse heuristic (heuristic-only, does not affect status/rolling stats)
        let p95 = state.network.p95_latency_ms;
        let base_thresh = if p95 > 0 && state.network.rolling_totals.len() >= 4 { p95 / 3 } else { 500 };
        let mut reuse_thresh = base_thresh.max(350);
        reuse_thresh = reuse_thresh.clamp(250, 1500);

        let connection_reused = metrics.latency_ms <= reuse_thresh;

        // Keep breakdown numeric; log reuse heuristics
        let breakdown_numeric = format!(
            "DNS:0ms|TCP:0ms|TLS:0ms|TTFB:{}ms|Total:{}ms",
            metrics.latency_ms, metrics.latency_ms
        );

        let debug_logger = get_debug_logger();
        debug_logger.debug(
            "HttpMonitor",
            &format!("reuse_heuristic: reused={} thresh={}ms total={}ms p95={}ms",
                     connection_reused, reuse_thresh, metrics.latency_ms, p95),
        ).await;

        // Update immediate metrics with numeric breakdown
        state.network.latency_ms = metrics.latency_ms;
        state.network.breakdown = breakdown_numeric; // Use computed numeric breakdown
        state.network.last_http_status = metrics.last_http_status;
        state.network.error_type = metrics.error_type.clone();
        state.network.connection_reused = Some(connection_reused);
        state.network.breakdown_source = Some("heuristic".to_string());
        state.timestamp = self.clock.local_timestamp();

        // Update API config
        state.api_config = Some(ApiConfig {
            endpoint: format!("{}/v1/messages", creds.base_url),
            source: creds.source.to_string(),
        });
        state.monitoring_enabled = true;

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
}

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
