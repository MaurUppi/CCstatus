// OAuth Masquerade Module - First-Party Probe for OAuth Environments
//
// This module provides OAuth masquerade functionality that shapes a first-party-like
// POST request to https://api.anthropic.com/v1/messages when OAuth credentials are
// present and unexpired. It maintains the existing x-api-key flow unchanged.

use crate::core::network::types::NetworkError;
use std::collections::HashMap;
use std::env;

#[cfg(feature = "timings-curl")]
use crate::core::network::http_monitor::CurlProbeRunner;

/// Configuration options for OAuth masquerade probe
pub struct OauthMasqueradeOptions {
    /// Base URL for the API endpoint (expect https://api.anthropic.com)
    pub base_url: String,
    /// OAuth access token (oat01)
    pub access_token: String,
    /// Token expiry timestamp in milliseconds since epoch
    pub expires_at: Option<i64>,
    /// Whether to add stream=true and helper header
    pub stream: bool,
}

/// Result from OAuth masquerade probe execution
pub struct OauthMasqueradeResult {
    /// HTTP status code
    pub status: u16,
    /// Request duration in milliseconds
    pub duration_ms: u32,
    /// Timing breakdown string (DNS:...|TCP:...|TLS:...|ServerTTFB:...|Total:..)
    pub breakdown: String,
    /// Response headers
    pub response_headers: HashMap<String, String>,
    /// HTTP version used (HTTP/1.1 or HTTP/2.0)
    pub http_version: Option<String>,
}

/// Default header profile constants matching demo defaults
struct HeaderProfile {
    // Core headers
    pub content_type: &'static str,
    pub accept: &'static str,
    pub anthropic_version: &'static str,
    pub user_agent: &'static str,
    pub dangerous_direct_browser_access: &'static str,
    pub x_app: &'static str,

    // X-Stainless headers
    pub x_stainless_retry_count: &'static str,
    pub x_stainless_timeout: &'static str,
    pub x_stainless_lang: &'static str,
    pub x_stainless_package_version: &'static str,
    pub x_stainless_os: &'static str,
    pub x_stainless_arch: &'static str,
    pub x_stainless_runtime: &'static str,
    pub x_stainless_runtime_version: &'static str,

    // Accept headers
    pub accept_language: &'static str,
    pub accept_encoding: &'static str,

    // Beta features
    pub anthropic_beta: &'static str,

    // Stream helper method (when stream=true)
    pub x_stainless_helper_method: &'static str,
}

/// Default header profile matching successful demo
const DEFAULT_HEADER_PROFILE: HeaderProfile = HeaderProfile {
    content_type: "application/json",
    accept: "application/json",
    anthropic_version: "2023-06-01",
    user_agent: "claude-cli/1.0.103 (external, cli)",
    dangerous_direct_browser_access: "true",
    x_app: "cli",

    x_stainless_retry_count: "0",
    x_stainless_timeout: "600",
    x_stainless_lang: "js",
    x_stainless_package_version: "0.60.0",
    x_stainless_os: "MacOS",
    x_stainless_arch: "arm64",
    x_stainless_runtime: "node",
    x_stainless_runtime_version: "v24.4.1",

    accept_language: "*",
    accept_encoding: "br, gzip, deflate",

    anthropic_beta: "claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14",

    x_stainless_helper_method: "stream",
};

/// Claude Code system prompt that must always be at system[0]
const CLAUDE_CODE_SYSTEM_PROMPT: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// Test environment variable names (internal use only)
const TEST_HEADERS_FILE: &str = "CCSTATUS_TEST_HEADERS_FILE";
const TEST_UA: &str = "CCSTATUS_TEST_UA";
const TEST_BETA_HEADER: &str = "CCSTATUS_TEST_BETA_HEADER";
const TEST_STAINLESS_OS: &str = "CCSTATUS_TEST_STAINLESS_OS";
const TEST_STAINLESS_ARCH: &str = "CCSTATUS_TEST_STAINLESS_ARCH";
const TEST_STAINLESS_RUNTIME: &str = "CCSTATUS_TEST_STAINLESS_RUNTIME";
const TEST_STAINLESS_RUNTIME_VERSION: &str = "CCSTATUS_TEST_STAINLESS_RUNTIME_VERSION";

/// Build headers for OAuth masquerade request
fn build_headers(opts: &OauthMasqueradeOptions) -> HashMap<String, String> {
    let mut headers = HashMap::new();

    // Authorization header with Bearer token
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {}", opts.access_token),
    );

    // Core headers
    headers.insert(
        "Content-Type".to_string(),
        DEFAULT_HEADER_PROFILE.content_type.to_string(),
    );
    headers.insert(
        "Accept".to_string(),
        DEFAULT_HEADER_PROFILE.accept.to_string(),
    );
    headers.insert(
        "anthropic-version".to_string(),
        DEFAULT_HEADER_PROFILE.anthropic_version.to_string(),
    );

    // User-Agent (test override available)
    let user_agent =
        env::var(TEST_UA).unwrap_or_else(|_| DEFAULT_HEADER_PROFILE.user_agent.to_string());
    headers.insert("User-Agent".to_string(), user_agent);

    headers.insert(
        "anthropic-dangerous-direct-browser-access".to_string(),
        DEFAULT_HEADER_PROFILE
            .dangerous_direct_browser_access
            .to_string(),
    );
    headers.insert(
        "x-app".to_string(),
        DEFAULT_HEADER_PROFILE.x_app.to_string(),
    );

    // X-Stainless headers (test overrides available)
    headers.insert(
        "X-Stainless-Retry-Count".to_string(),
        DEFAULT_HEADER_PROFILE.x_stainless_retry_count.to_string(),
    );
    headers.insert(
        "X-Stainless-Timeout".to_string(),
        DEFAULT_HEADER_PROFILE.x_stainless_timeout.to_string(),
    );
    headers.insert(
        "X-Stainless-Lang".to_string(),
        DEFAULT_HEADER_PROFILE.x_stainless_lang.to_string(),
    );
    headers.insert(
        "X-Stainless-Package-Version".to_string(),
        DEFAULT_HEADER_PROFILE
            .x_stainless_package_version
            .to_string(),
    );

    let os = env::var(TEST_STAINLESS_OS)
        .unwrap_or_else(|_| DEFAULT_HEADER_PROFILE.x_stainless_os.to_string());
    headers.insert("X-Stainless-OS".to_string(), os);

    let arch = env::var(TEST_STAINLESS_ARCH)
        .unwrap_or_else(|_| DEFAULT_HEADER_PROFILE.x_stainless_arch.to_string());
    headers.insert("X-Stainless-Arch".to_string(), arch);

    let runtime = env::var(TEST_STAINLESS_RUNTIME)
        .unwrap_or_else(|_| DEFAULT_HEADER_PROFILE.x_stainless_runtime.to_string());
    headers.insert("X-Stainless-Runtime".to_string(), runtime);

    let runtime_version = env::var(TEST_STAINLESS_RUNTIME_VERSION).unwrap_or_else(|_| {
        DEFAULT_HEADER_PROFILE
            .x_stainless_runtime_version
            .to_string()
    });
    headers.insert("X-Stainless-Runtime-Version".to_string(), runtime_version);

    // Accept headers
    headers.insert(
        "accept-language".to_string(),
        DEFAULT_HEADER_PROFILE.accept_language.to_string(),
    );
    headers.insert(
        "accept-encoding".to_string(),
        DEFAULT_HEADER_PROFILE.accept_encoding.to_string(),
    );

    // Beta header (test override available)
    let beta = env::var(TEST_BETA_HEADER)
        .unwrap_or_else(|_| DEFAULT_HEADER_PROFILE.anthropic_beta.to_string());
    headers.insert("anthropic-beta".to_string(), beta);

    // Stream helper method header when stream=true
    if opts.stream {
        headers.insert(
            "x-stainless-helper-method".to_string(),
            DEFAULT_HEADER_PROFILE.x_stainless_helper_method.to_string(),
        );
    }

    // Append test-only headers from file (internal testing)
    if let Ok(headers_file) = env::var(TEST_HEADERS_FILE) {
        if let Ok(content) = std::fs::read_to_string(&headers_file) {
            for line in content.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();

                    // Skip security-sensitive headers
                    if key.eq_ignore_ascii_case("Authorization")
                        || key.eq_ignore_ascii_case("Host")
                        || key.eq_ignore_ascii_case("Content-Length")
                        || key.eq_ignore_ascii_case("Cookie")
                        || key.eq_ignore_ascii_case("Set-Cookie")
                        || key.eq_ignore_ascii_case("X-Api-Key")
                        || key.eq_ignore_ascii_case("Proxy-Authorization")
                    {
                        continue;
                    }

                    headers.insert(key.to_string(), value.to_string());
                }
            }
        }
    }

    headers
}

/// Build request body for OAuth masquerade request
fn build_request_body(opts: &OauthMasqueradeOptions) -> Result<Vec<u8>, NetworkError> {
    let mut body = serde_json::json!({
        "model": "claude-3-5-haiku-20241022",
        "max_tokens": 1,
        "system": [
            {
                "type": "text",
                "text": CLAUDE_CODE_SYSTEM_PROMPT
            }
        ],
        "messages": [
            {
                "role": "user",
                "content": "Hi"
            }
        ]
    });

    // Add stream parameter when requested
    if opts.stream {
        body["stream"] = serde_json::Value::Bool(true);
    }

    serde_json::to_vec(&body).map_err(|e| {
        // Debug logging for serialization errors
        if is_debug_enabled() {
            eprintln!("OAuth masquerade body serialization error: {}", e);
        }
        NetworkError::HttpError(format!("OAuth masquerade body serialization failed: {}", e))
    })
}

/// Check if debug mode is enabled via CCSTATUS_DEBUG environment variable
fn is_debug_enabled() -> bool {
    env::var("CCSTATUS_DEBUG")
        .unwrap_or_default()
        .to_uppercase()
        == "TRUE"
}

/// Check if OAuth token is expired
fn is_token_expired(expires_at: Option<i64>) -> bool {
    if let Some(expiry_ms) = expires_at {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        expiry_ms <= now_ms
    } else {
        false // No expiry means never expired
    }
}

/// Run OAuth masquerade probe
///
/// This function executes a first-party-shaped POST request to the Claude API
/// when valid OAuth credentials are present and unexpired.
///
/// When `timings-curl` feature is enabled and a curl_runner is provided, uses curl
/// for detailed phase timings (DNS|TCP|TLS|TTFB breakdown), otherwise falls back
/// to isahc-based transport with simplified "Total:...ms" timing.
#[cfg(feature = "timings-curl")]
pub async fn run_probe(
    opts: &OauthMasqueradeOptions,
    http_client: &dyn crate::core::network::http_monitor::HttpClientTrait,
    curl_runner: Option<&dyn CurlProbeRunner>,
) -> Result<OauthMasqueradeResult, NetworkError> {
    use crate::core::network::debug_logger::get_debug_logger;

    // Check for expired token (hard gate)
    if is_token_expired(opts.expires_at) {
        // Log expiry skip when CCSTATUS_DEBUG=TRUE
        if is_debug_enabled() {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            let expiry_desc = opts
                .expires_at
                .map(|exp| format!("{}", exp))
                .unwrap_or_else(|| "none".to_string());

            tokio::spawn(async move {
                let logger = get_debug_logger();
                let _ = logger
                    .debug(
                        "OauthMasquerade",
                        &format!(
                            "reason=expired_token now_ms={} expires_at_ms={} action=skip_no_probe",
                            now_ms, expiry_desc
                        ),
                    )
                    .await;
            });
        }

        return Err(NetworkError::CredentialError(
            "OAuth token expired".to_string(),
        ));
    }

    // Debug logging: Entry decision
    if is_debug_enabled() {
        let logger = get_debug_logger();
        let user_agent =
            env::var(TEST_UA).unwrap_or_else(|_| DEFAULT_HEADER_PROFILE.user_agent.to_string());
        let beta_present =
            env::var(TEST_BETA_HEADER).is_ok() || !DEFAULT_HEADER_PROFILE.anthropic_beta.is_empty();
        let token_len = opts.access_token.len();
        let expires_desc = opts
            .expires_at
            .map(|exp| format!("{}", exp))
            .unwrap_or_else(|| "none".to_string());

        let _ = logger.debug(
            "OauthMasquerade",
            &format!(
                "masquerade=true base={} stream={} ua=\"{}\" beta_present={} headers_profile=default token_len={} expires_at_ms={}",
                opts.base_url, opts.stream, user_agent, beta_present, token_len, expires_desc
            )
        ).await;
    }

    // Build headers and body for first-party-shaped request
    let headers = build_headers(opts);
    let body = build_request_body(opts)?;

    // Construct endpoint URL
    let endpoint = format!("{}/v1/messages", opts.base_url);

    // Debug logging: Request construction details
    if is_debug_enabled() {
        let logger = get_debug_logger();
        let header_count = headers.len();
        let body_size = body.len();
        let has_test_overrides = env::var(TEST_HEADERS_FILE).is_ok()
            || env::var(TEST_UA).is_ok()
            || env::var(TEST_BETA_HEADER).is_ok();

        let _ = logger.debug(
            "OauthMasquerade",
            &format!(
                "request_construction endpoint={} headers_count={} body_size={} test_overrides={} stream={}",
                endpoint, header_count, body_size, has_test_overrides, opts.stream
            )
        ).await;
    }

    // Try curl first when available for detailed phase timings, fallback to isahc
    let debug_logger = get_debug_logger();
    if let Some(curl_runner) = curl_runner {
        let _ = debug_logger
            .debug(
                "OauthMasquerade",
                "Trying OAuth masquerade probe with curl for phase timings",
            )
            .await;

        // Convert HashMap headers to Vec<(&str, String)> format expected by curl
        let headers_vec: Vec<(&str, String)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();

        // Try curl first for detailed phase timings
        match curl_runner.run(&endpoint, &headers_vec, &body, 10000).await {
            Ok(phase_timings) => {
                let breakdown = format!(
                    "DNS:{}ms|TCP:{}ms|TLS:{}ms|TTFB:{}ms|Total:{}ms",
                    phase_timings.dns_ms,
                    phase_timings.tcp_ms,
                    phase_timings.tls_ms,
                    phase_timings.ttfb_ms,
                    phase_timings.total_ms
                );

                // Note: curl branch doesn't capture response headers in current implementation
                let empty_headers = std::collections::HashMap::new();
                // HTTP/2 is typically negotiated with curl when using HTTP/2 TLS version
                let http_version = Some("HTTP/2.0".to_string());

                return Ok(OauthMasqueradeResult {
                    status: phase_timings.status,
                    duration_ms: phase_timings.total_ms,
                    breakdown,
                    response_headers: empty_headers,
                    http_version,
                });
            }
            Err(curl_error) => {
                // Log curl failure and fallback to isahc for resiliency
                let _ = debug_logger
                    .error(
                        "OauthMasquerade",
                        &format!(
                            "OAuth curl probe failed, falling back to isahc: {}",
                            curl_error
                        ),
                    )
                    .await;
                // Fall through to isahc path below
            }
        }
    }

    // Execute actual HTTP request using the provided HTTP client (isahc fallback)
    let _ = debug_logger
        .debug(
            "OauthMasquerade",
            "Executing OAuth masquerade probe with isahc HTTP transport",
        )
        .await;

    // Execute the request through the HTTP client
    let (status, duration, breakdown, response_headers, http_version) = http_client
        .execute_request(endpoint, headers, body, 10000) // 10 second timeout for OAuth probes
        .await
        .map_err(|e| NetworkError::HttpError(format!("OAuth HTTP request failed: {}", e)))?;

    // Create redacted response headers without sensitive information
    let redacted_response_headers = response_headers
        .iter()
        .filter(|(key, _)| {
            !key.eq_ignore_ascii_case("Authorization")
                && !key.eq_ignore_ascii_case("X-Api-Key")
                && !key.eq_ignore_ascii_case("Proxy-Authorization")
                && !key.eq_ignore_ascii_case("Cookie")
                && !key.eq_ignore_ascii_case("Set-Cookie")
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<HashMap<String, String>>();

    Ok(OauthMasqueradeResult {
        status,
        duration_ms: duration.as_millis() as u32,
        breakdown,
        response_headers: redacted_response_headers,
        http_version,
    })
}

/// Run OAuth masquerade probe (without curl timing support)
///
/// This function executes a first-party-shaped POST request to the Claude API
/// when valid OAuth credentials are present and unexpired.
///
/// Uses isahc-based transport with simplified "Total:...ms" timing format.
#[cfg(not(feature = "timings-curl"))]
pub async fn run_probe(
    opts: &OauthMasqueradeOptions,
    http_client: &dyn crate::core::network::http_monitor::HttpClientTrait,
) -> Result<OauthMasqueradeResult, NetworkError> {
    use crate::core::network::debug_logger::get_debug_logger;

    // Check for expired token (hard gate)
    if is_token_expired(opts.expires_at) {
        // Log expiry skip when CCSTATUS_DEBUG=TRUE
        if is_debug_enabled() {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            let expiry_desc = opts
                .expires_at
                .map(|exp| format!("{}", exp))
                .unwrap_or_else(|| "none".to_string());

            tokio::spawn(async move {
                let logger = get_debug_logger();
                let _ = logger
                    .debug(
                        "OauthMasquerade",
                        &format!(
                            "reason=expired_token now_ms={} expires_at_ms={} action=skip_no_probe",
                            now_ms, expiry_desc
                        ),
                    )
                    .await;
            });
        }

        return Err(NetworkError::CredentialError(
            "OAuth token expired".to_string(),
        ));
    }

    // Debug logging: Entry decision
    if is_debug_enabled() {
        let logger = get_debug_logger();
        let user_agent =
            env::var(TEST_UA).unwrap_or_else(|_| DEFAULT_HEADER_PROFILE.user_agent.to_string());
        let beta_present =
            env::var(TEST_BETA_HEADER).is_ok() || !DEFAULT_HEADER_PROFILE.anthropic_beta.is_empty();
        let token_len = opts.access_token.len();
        let expires_desc = opts
            .expires_at
            .map(|exp| format!("{}", exp))
            .unwrap_or_else(|| "none".to_string());

        let _ = logger.debug(
            "OauthMasquerade",
            &format!(
                "masquerade=true base={} stream={} ua=\"{}\" beta_present={} headers_profile=default token_len={} expires_at_ms={}",
                opts.base_url, opts.stream, user_agent, beta_present, token_len, expires_desc
            )
        ).await;
    }

    // Build headers and body for first-party-shaped request
    let headers = build_headers(opts);
    let body = build_request_body(opts)?;

    // Construct endpoint URL
    let endpoint = format!("{}/v1/messages", opts.base_url);

    // Debug logging: Request construction details
    if is_debug_enabled() {
        let logger = get_debug_logger();
        let header_count = headers.len();
        let body_size = body.len();
        let has_test_overrides = env::var(TEST_HEADERS_FILE).is_ok()
            || env::var(TEST_UA).is_ok()
            || env::var(TEST_BETA_HEADER).is_ok();

        let _ = logger.debug(
            "OauthMasquerade",
            &format!(
                "request_construction endpoint={} headers_count={} body_size={} test_overrides={} stream={}",
                endpoint, header_count, body_size, has_test_overrides, opts.stream
            )
        ).await;
    }

    // Execute HTTP request using the provided HTTP client (isahc)
    let debug_logger = get_debug_logger();
    let _ = debug_logger
        .debug(
            "OauthMasquerade",
            "Executing OAuth masquerade probe with isahc HTTP transport",
        )
        .await;

    // Execute the request through the HTTP client
    let (status, duration, breakdown, response_headers, http_version) = http_client
        .execute_request(endpoint, headers, body, 10000) // 10 second timeout for OAuth probes
        .await
        .map_err(|e| NetworkError::HttpError(format!("OAuth HTTP request failed: {}", e)))?;

    // Create redacted response headers without sensitive information
    let redacted_response_headers = response_headers
        .iter()
        .filter(|(key, _)| {
            !key.eq_ignore_ascii_case("Authorization")
                && !key.eq_ignore_ascii_case("X-Api-Key")
                && !key.eq_ignore_ascii_case("Proxy-Authorization")
                && !key.eq_ignore_ascii_case("Cookie")
                && !key.eq_ignore_ascii_case("Set-Cookie")
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<HashMap<String, String>>();

    Ok(OauthMasqueradeResult {
        status,
        duration_ms: duration.as_millis() as u32,
        breakdown,
        response_headers: redacted_response_headers,
        http_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::network::http_monitor::HttpClientTrait;
    use std::time::Duration;

    /// Mock HTTP client for testing OAuth masquerade
    struct MockHttpClient;

    #[async_trait::async_trait]
    impl HttpClientTrait for MockHttpClient {
        async fn execute_request(
            &self,
            _url: String,
            headers: std::collections::HashMap<String, String>,
            _body: Vec<u8>,
            _timeout_ms: u32,
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
            // Return mock response that includes request headers to test redaction
            let mut response_headers = HashMap::new();
            // Include some request headers as if they were response headers (for testing redaction)
            for (key, value) in &headers {
                response_headers.insert(key.clone(), value.clone());
            }
            // Add some typical response headers
            response_headers.insert("Server".to_string(), "anthropic".to_string());
            response_headers.insert("Cache-Control".to_string(), "no-cache".to_string());

            Ok((
                200,
                Duration::from_millis(250),
                "DNS:5ms|TCP:10ms|TLS:50ms|TTFB:200ms|Total:265ms".to_string(),
                response_headers,
                Some("HTTP/2.0".to_string()),
            ))
        }
    }

    #[test]
    fn test_build_headers_includes_required_names() {
        let opts = OauthMasqueradeOptions {
            base_url: "https://api.anthropic.com".to_string(),
            access_token: "test_token".to_string(),
            expires_at: None,
            stream: false,
        };

        let headers = build_headers(&opts);

        // Verify required headers are present
        assert!(headers.contains_key("Authorization"));
        assert!(headers.contains_key("Content-Type"));
        assert!(headers.contains_key("Accept"));
        assert!(headers.contains_key("anthropic-version"));
        assert!(headers.contains_key("User-Agent"));
        assert!(headers.contains_key("X-Stainless-Retry-Count"));
        assert!(headers.contains_key("anthropic-beta"));

        // Verify Authorization header format
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer test_token");
    }

    #[test]
    fn test_build_headers_stream_adds_helper_method() {
        let opts = OauthMasqueradeOptions {
            base_url: "https://api.anthropic.com".to_string(),
            access_token: "test_token".to_string(),
            expires_at: None,
            stream: true,
        };

        let headers = build_headers(&opts);
        assert!(headers.contains_key("x-stainless-helper-method"));
        assert_eq!(headers.get("x-stainless-helper-method").unwrap(), "stream");
    }

    #[test]
    fn test_build_request_body_includes_system_prompt() {
        let opts = OauthMasqueradeOptions {
            base_url: "https://api.anthropic.com".to_string(),
            access_token: "test_token".to_string(),
            expires_at: None,
            stream: false,
        };

        let body_bytes = build_request_body(&opts).unwrap();
        let body_str = String::from_utf8(body_bytes).unwrap();

        assert!(body_str.contains(CLAUDE_CODE_SYSTEM_PROMPT));
        assert!(body_str.contains("\"max_tokens\":1"));
        assert!(body_str.contains("\"role\":\"user\""));
    }

    #[test]
    fn test_build_request_body_stream_parameter() {
        let opts = OauthMasqueradeOptions {
            base_url: "https://api.anthropic.com".to_string(),
            access_token: "test_token".to_string(),
            expires_at: None,
            stream: true,
        };

        let body_bytes = build_request_body(&opts).unwrap();
        let body_str = String::from_utf8(body_bytes).unwrap();

        assert!(body_str.contains("\"stream\":true"));
    }

    #[test]
    fn test_is_token_expired() {
        // No expiry = never expired
        assert!(!is_token_expired(None));

        // Far future = not expired
        let future_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64)
            + 3600000; // +1 hour
        assert!(!is_token_expired(Some(future_ms)));

        // Past = expired
        let past_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64)
            - 3600000; // -1 hour
        assert!(is_token_expired(Some(past_ms)));
    }

    #[tokio::test]
    async fn test_result_headers_exclude_sensitive_data() {
        let opts = OauthMasqueradeOptions {
            base_url: "https://api.anthropic.com".to_string(),
            access_token: "test_sensitive_token".to_string(),
            expires_at: None,
            stream: false,
        };

        let mock_client = MockHttpClient;

        #[cfg(feature = "timings-curl")]
        let result = run_probe(&opts, &mock_client, None).await.unwrap();

        #[cfg(not(feature = "timings-curl"))]
        let result = run_probe(&opts, &mock_client).await.unwrap();

        // Ensure sensitive headers are NOT present in the result
        assert!(!result.response_headers.contains_key("Authorization"));
        assert!(!result.response_headers.contains_key("authorization"));
        assert!(!result.response_headers.contains_key("X-Api-Key"));
        assert!(!result.response_headers.contains_key("x-api-key"));
        assert!(!result.response_headers.contains_key("Proxy-Authorization"));
        assert!(!result.response_headers.contains_key("proxy-authorization"));
        assert!(!result.response_headers.contains_key("Cookie"));
        assert!(!result.response_headers.contains_key("cookie"));
        assert!(!result.response_headers.contains_key("Set-Cookie"));
        assert!(!result.response_headers.contains_key("set-cookie"));

        // Ensure non-sensitive headers are still present
        assert!(result.response_headers.contains_key("Content-Type"));
        assert!(result.response_headers.contains_key("Accept"));
        assert!(result.response_headers.contains_key("User-Agent"));
    }

    #[test]
    fn test_test_headers_file_filters_sensitive_headers() {
        use std::env;
        use std::fs;
        use tempfile::NamedTempFile;

        // Create a temporary file with both safe and sensitive headers
        let temp_file = NamedTempFile::new().unwrap();
        let content = r#"Custom-Header: safe-value
Authorization: Bearer malicious-token
X-Custom: another-safe-value
Cookie: session=dangerous
Set-Cookie: token=evil
X-Api-Key: malicious-api-key
Proxy-Authorization: Basic evil-creds
Host: malicious-host.com
Content-Length: 9999
Safe-Header: totally-fine"#;
        fs::write(temp_file.path(), content).unwrap();

        // Set the test environment variable
        env::set_var(TEST_HEADERS_FILE, temp_file.path().to_str().unwrap());

        let opts = OauthMasqueradeOptions {
            base_url: "https://api.anthropic.com".to_string(),
            access_token: "test_token".to_string(),
            expires_at: None,
            stream: false,
        };

        let headers = build_headers(&opts);

        // Verify the Authorization header exists (from build_headers) but not overridden by file
        assert!(headers.contains_key("Authorization"));
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer test_token"); // Original, not from file
        assert!(!headers.contains_key("Cookie"));
        assert!(!headers.contains_key("Set-Cookie"));
        assert!(!headers.contains_key("X-Api-Key"));
        assert!(!headers.contains_key("Proxy-Authorization"));
        assert!(!headers.contains_key("Host"));
        assert!(!headers.contains_key("Content-Length"));

        // Verify safe headers from file ARE included
        assert!(headers.contains_key("Custom-Header"));
        assert_eq!(headers.get("Custom-Header").unwrap(), "safe-value");
        assert!(headers.contains_key("X-Custom"));
        assert_eq!(headers.get("X-Custom").unwrap(), "another-safe-value");
        assert!(headers.contains_key("Safe-Header"));
        assert_eq!(headers.get("Safe-Header").unwrap(), "totally-fine");

        // Clean up
        env::remove_var(TEST_HEADERS_FILE);
    }
}
