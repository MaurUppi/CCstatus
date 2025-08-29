//! Health Check Client Implementations
//! 
//! Provides HTTP client abstraction specialized for proxy health check operations
//! with GET method, response body access, and redirect control.

use crate::core::network::types::NetworkError;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[cfg(feature = "network-monitoring")]
use isahc::config::{Configurable, RedirectPolicy};
#[cfg(feature = "network-monitoring")]
use isahc::{AsyncReadResponseExt, HttpClient, Request};

#[cfg(feature = "timings-curl")]
use curl::easy::Easy;

/// Health check response containing full response data for validation
#[derive(Debug, Clone)]
pub struct HealthResponse {
    /// HTTP status code from health endpoint
    pub status_code: u16,
    /// Response body content for JSON validation
    pub body: Vec<u8>,
    /// Request duration for metrics
    pub duration: Duration,
    /// HTTP response headers for Cloudflare detection and redirect handling
    pub headers: HashMap<String, String>,
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
    async fn get_health(&self, url: String, timeout_ms: u32) -> Result<HealthResponse, String>;
}

/// Production health check client implementation using isahc with GET method
#[cfg(feature = "network-monitoring")]
pub struct IsahcHealthCheckClient {
    client: HttpClient,
}

#[cfg(feature = "network-monitoring")]
#[async_trait::async_trait]
impl HealthCheckClient for IsahcHealthCheckClient {
    async fn get_health(&self, url: String, timeout_ms: u32) -> Result<HealthResponse, String> {
        let start = Instant::now();

        let request = Request::get(&url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .redirect_policy(RedirectPolicy::None) // Critical: Don't follow redirects
            .header("User-Agent", "claude-cli/1.0.93 (external, cli)")
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip, deflate, br") // Bot-fight mitigation
            .header("Accept-Language", "en-US,en;q=0.9") // Bot-fight mitigation
            .body(Vec::new()) // Empty body for GET request
            .map_err(|e| format!("Health check request creation failed: {}", e))?;

        let mut response = self
            .client
            .send_async(request)
            .await
            .map_err(|e| format!("Health check request failed: {}", e))?;

        let status_code = response.status().as_u16();
        let duration = start.elapsed();

        // Collect headers for Cloudflare detection and redirect handling
        let mut headers = HashMap::new();
        for (key, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(key.to_string().to_lowercase(), value_str.to_string());
            }
        }

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
            headers,
        })
    }
}

#[cfg(feature = "timings-curl")]
use crate::core::network::http_monitor::PhaseTimings;

/// Curl-based GET runner for enhanced proxy health timing
/// 
/// Provides detailed phase timings (DNS, TCP, TLS, TTFB, Total) using curl
/// with bot-fight mitigation headers and HTTP/2 support.
#[cfg(feature = "timings-curl")]
pub struct CurlGetRunner;

#[cfg(feature = "timings-curl")]
impl CurlGetRunner {
    /// Execute GET request with detailed phase timing extraction
    /// 
    /// # Arguments
    /// * `url` - Complete health check URL
    /// * `timeout_ms` - Request timeout in milliseconds
    /// 
    /// # Returns
    /// * `Ok((HealthResponse, PhaseTimings))` - Response with detailed timings
    /// * `Err(String)` - Network error or curl failure
    /// 
    /// # Bot-Fight Enhancements
    /// * HTTP/2 negotiation with TLS fallback
    /// * Compression support (gzip, deflate, br)
    /// * Claude CLI user agent
    /// * In-memory cookie engine for session continuity
    pub async fn get_health_with_timings(
        &self,
        url: &str,
        timeout_ms: u32,
    ) -> Result<(HealthResponse, PhaseTimings), String> {
        
        let url = url.to_string();
        let result = tokio::task::spawn_blocking(move || -> Result<(HealthResponse, PhaseTimings), String> {
            let mut handle = curl::easy::Easy::new();
            
            // Configure GET request with bot-fight enhancements
            handle.url(&url).map_err(|e| format!("URL set failed: {}", e))?;
            handle.get(true).map_err(|e| format!("GET method failed: {}", e))?; // GET method
            handle.timeout(std::time::Duration::from_millis(timeout_ms as u64))
                .map_err(|e| format!("Timeout set failed: {}", e))?;
            handle.http_version(curl::easy::HttpVersion::V2TLS)
                .map_err(|e| format!("HTTP/2 version failed: {}", e))?;
            handle.accept_encoding("gzip, deflate, br")
                .map_err(|e| format!("Accept-Encoding failed: {}", e))?;
            handle.useragent("claude-cli/1.0.93 (external, cli)")
                .map_err(|e| format!("User-Agent failed: {}", e))?;
            handle.cookie_file("").map_err(|e| format!("Cookie engine failed: {}", e))?; // In-memory cookies
            
            // Set headers for bot-fight mitigation
            let mut header_list = curl::easy::List::new();
            header_list.append("Accept: application/json")
                .map_err(|e| format!("Accept header failed: {}", e))?;
            header_list.append("Accept-Language: en-US,en;q=0.9")
                .map_err(|e| format!("Accept-Language header failed: {}", e))?;
            handle.http_headers(header_list)
                .map_err(|e| format!("Headers set failed: {}", e))?;
            
            // Capture response using Arc<Mutex<>> for thread-safe shared ownership
            use std::sync::{Arc, Mutex};
            
            let response_body = Arc::new(Mutex::new(Vec::new()));
            let response_headers = Arc::new(Mutex::new(std::collections::HashMap::new()));
            
            {
                let body_clone = response_body.clone();
                handle.write_function(move |data| {
                    body_clone.lock().unwrap().extend_from_slice(data);
                    Ok(data.len())
                }).map_err(|e| format!("Write function failed: {}", e))?;
            }
            
            {
                let headers_clone = response_headers.clone();
                handle.header_function(move |data| {
                    if let Ok(header_str) = std::str::from_utf8(data) {
                        if let Some((key, value)) = header_str.split_once(':') {
                            headers_clone.lock().unwrap().insert(
                                key.trim().to_lowercase(),
                                value.trim().to_string()
                            );
                        }
                    }
                    true
                }).map_err(|e| format!("Header function failed: {}", e))?;
            }
            
            // Execute request
            handle.perform().map_err(|e| format!("Request perform failed: {}", e))?;
            
            // Extract timings and status
            let status_code = handle.response_code().map_err(|e| format!("Response code failed: {}", e))? as u16;
            
            // Extract phase timings from libcurl (in seconds, convert to ms)
            let dns_time = handle.namelookup_time()
                .map_err(|e| format!("DNS time failed: {}", e))?
                .as_secs_f64();
            let connect_time = handle.connect_time()
                .map_err(|e| format!("Connect time failed: {}", e))?
                .as_secs_f64();
            let appconnect_time = handle.appconnect_time()
                .map_err(|e| format!("App connect time failed: {}", e))?
                .as_secs_f64();
            let starttransfer_time = handle.starttransfer_time()
                .map_err(|e| format!("Start transfer time failed: {}", e))?
                .as_secs_f64();
            let total_time = handle.total_time()
                .map_err(|e| format!("Total time failed: {}", e))?
                .as_secs_f64();
            
            // Calculate phase durations and convert to milliseconds
            let dns_ms = (dns_time * 1000.0).max(0.0) as u32;
            let tcp_ms = ((connect_time - dns_time).max(0.0) * 1000.0) as u32;
            let tls_ms = ((appconnect_time - connect_time).max(0.0) * 1000.0) as u32;
            let ttfb_ms = ((starttransfer_time - appconnect_time).max(0.0) * 1000.0) as u32;
            let total_ms = (total_time * 1000.0).max(0.0) as u32;
            
            // Extract data from Arc<Mutex<>>
            let final_body = response_body.lock().unwrap().clone();
            let final_headers = response_headers.lock().unwrap().clone();
            
            // Build response and timings
            let health_response = HealthResponse {
                status_code,
                body: final_body,
                duration: std::time::Duration::from_secs_f64(total_time),
                headers: final_headers,
            };
            
            let phase_timings = PhaseTimings {
                status: status_code,
                dns_ms,
                tcp_ms,
                tls_ms,
                ttfb_ms,
                total_ms,
            };
            
            Ok((health_response, phase_timings))
        }).await
        .map_err(|e| format!("Curl GET task failed: {}", e))?
        .map_err(|e| e)?;
        
        Ok(result)
    }
}

/// Curl-based health check client with enhanced timing capabilities
/// 
/// Provides detailed phase timings for proxy health checks when timings-curl feature is enabled.
/// Falls back to standard HealthResponse interface while internally capturing PhaseTimings.
#[cfg(all(feature = "network-monitoring", feature = "timings-curl"))]
pub struct CurlHealthCheckClient {
    runner: CurlGetRunner,
}

#[cfg(all(feature = "network-monitoring", feature = "timings-curl"))]
#[async_trait::async_trait]
impl HealthCheckClient for CurlHealthCheckClient {
    async fn get_health(&self, url: String, timeout_ms: u32) -> Result<HealthResponse, String> {
        // Use CurlGetRunner for enhanced timing, but only return HealthResponse for interface compatibility
        let (health_response, _phase_timings) = self.runner
            .get_health_with_timings(&url, timeout_ms)
            .await?;
            
        Ok(health_response)
    }
}

#[cfg(all(feature = "network-monitoring", feature = "timings-curl"))]
impl CurlHealthCheckClient {
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            runner: CurlGetRunner,
        })
    }
    
    /// Get health check with detailed phase timings (curl-specific enhancement)
    /// 
    /// This method provides access to the enhanced timing information that curl captures,
    /// beyond what the standard HealthCheckClient interface exposes.
    pub async fn get_health_with_timings(
        &self,
        url: &str,
        timeout_ms: u32,
    ) -> Result<(HealthResponse, PhaseTimings), String> {
        self.runner.get_health_with_timings(url, timeout_ms).await
    }
}

#[cfg(feature = "network-monitoring")]
impl IsahcHealthCheckClient {
    pub fn new() -> Result<Self, NetworkError> {
        let client = HttpClient::builder()
            .redirect_policy(RedirectPolicy::None) // Global redirect policy
            .cookies() // Enable in-memory cookies for session continuity
            .build()
            .map_err(|e| {
                NetworkError::HttpError(format!("Failed to create health check client: {}", e))
            })?;
        Ok(Self { client })
    }
}

/// Mock health check client implementation when network-monitoring feature is disabled
#[cfg(not(feature = "network-monitoring"))]
#[derive(Default)]
pub struct MockHealthCheckClient;

#[cfg(not(feature = "network-monitoring"))]
#[async_trait::async_trait]
impl HealthCheckClient for MockHealthCheckClient {
    async fn get_health(&self, _url: String, _timeout_ms: u32) -> Result<HealthResponse, String> {
        // Return mock healthy response
        let duration = Duration::from_millis(200);
        let body = r#"{"status": "healthy"}"#.as_bytes().to_vec();
        let headers = HashMap::new(); // Empty headers for mock
        Ok(HealthResponse {
            status_code: 200,
            body,
            duration,
            headers,
        })
    }
}