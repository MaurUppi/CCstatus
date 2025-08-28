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
            .header("User-Agent", "claude-cli/1.0.80 (external, cli)")
            .header("Accept", "application/json")
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

#[cfg(feature = "network-monitoring")]
impl IsahcHealthCheckClient {
    pub fn new() -> Result<Self, NetworkError> {
        let client = HttpClient::builder()
            .redirect_policy(RedirectPolicy::None) // Global redirect policy
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