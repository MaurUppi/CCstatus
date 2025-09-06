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
pub const CLAUDE_CODE_SYSTEM_PROMPT: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

/// Configuration environment variable names for OAuth masquerade headers
/// These allow runtime overrides of default header values to prevent drift
pub const TEST_HEADERS_FILE: &str = "CCSTATUS_TEST_HEADERS_FILE";
const TEST_UA: &str = "CCSTATUS_USER_AGENT";
const TEST_BETA_HEADER: &str = "CCSTATUS_ANTHROPIC_BETA";
const TEST_STAINLESS_PACKAGE_VERSION: &str = "CCSTATUS_STAINLESS_PACKAGE_VERSION";
const TEST_STAINLESS_OS: &str = "CCSTATUS_STAINLESS_OS";
const TEST_STAINLESS_ARCH: &str = "CCSTATUS_STAINLESS_ARCH";
const TEST_STAINLESS_RUNTIME: &str = "CCSTATUS_STAINLESS_RUNTIME";
const TEST_STAINLESS_RUNTIME_VERSION: &str = "CCSTATUS_STAINLESS_RUNTIME_VERSION";

/// Build headers for OAuth masquerade request
pub fn build_headers(opts: &OauthMasqueradeOptions) -> HashMap<String, String> {
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
    let package_version = env::var(TEST_STAINLESS_PACKAGE_VERSION).unwrap_or_else(|_| {
        DEFAULT_HEADER_PROFILE
            .x_stainless_package_version
            .to_string()
    });
    headers.insert("X-Stainless-Package-Version".to_string(), package_version);

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
pub fn build_request_body(opts: &OauthMasqueradeOptions) -> Result<Vec<u8>, NetworkError> {
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

/// Redact response headers using allowlist approach for security
/// Only returns headers that are safe to log and don't contain sensitive information
pub fn redact_response_headers(headers: &HashMap<String, String>) -> HashMap<String, String> {
    // Allowlist of safe response headers that don't contain sensitive information
    const ALLOWED_HEADERS: &[&str] = &[
        "server",
        "date",
        "cache-control",
        "via",
        "cf-ray",
        "age",
        "content-type",
        "content-length",
        "content-encoding",
        "x-request-id",
        "x-trace-id",
        "cf-cache-status",
        "cf-connecting-ip",
        "vary",
        "etag",
        "last-modified",
        "expires",
        "x-ratelimit-limit",
        "x-ratelimit-remaining",
        "x-ratelimit-reset",
        "retry-after",
    ];

    headers
        .iter()
        .filter(|(key, _)| {
            ALLOWED_HEADERS
                .iter()
                .any(|allowed| key.eq_ignore_ascii_case(allowed))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Check if OAuth token is expired
pub fn is_token_expired(expires_at: Option<i64>) -> bool {
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

/// Check for expired OAuth token and handle debug logging
/// Returns error if token is expired, otherwise returns Ok(())
async fn check_token_expiry_with_logging(opts: &OauthMasqueradeOptions) -> Result<(), NetworkError> {
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
                use crate::core::network::debug_logger::get_debug_logger;
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

        return Err(NetworkError::CredentialError("OAuth token expired".to_string()));
    }
    Ok(())
}

/// Log OAuth masquerade entry decision debug information
async fn log_entry_decision(opts: &OauthMasqueradeOptions) {
    if is_debug_enabled() {
        use crate::core::network::debug_logger::get_debug_logger;
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
}

/// Log OAuth masquerade request construction debug information
async fn log_request_construction(endpoint: &str, headers: &std::collections::HashMap<String, String>, body: &[u8], opts: &OauthMasqueradeOptions) {
    if is_debug_enabled() {
        use crate::core::network::debug_logger::get_debug_logger;
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
    check_token_expiry_with_logging(opts).await?;

    // Debug logging: Entry decision
    log_entry_decision(opts).await;

    // Build headers and body for first-party-shaped request
    let headers = build_headers(opts);
    let body = build_request_body(opts)?;

    // Construct endpoint URL
    let endpoint = format!("{}/v1/messages", opts.base_url);

    // Debug logging: Request construction details
    log_request_construction(&endpoint, &headers, &body, opts).await;

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

                // Note: curl branch doesn't capture response headers or HTTP version in current implementation
                // Setting http_version=None to avoid misleading diagnostics about version negotiation
                let empty_headers = std::collections::HashMap::new();
                let http_version = None; // Unknown version - curl implementation doesn't capture this

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

    // Create redacted response headers using allowlist for security
    let redacted_response_headers = redact_response_headers(&response_headers);

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
    check_token_expiry_with_logging(opts).await?;

    // Debug logging: Entry decision
    log_entry_decision(opts).await;

    // Build headers and body for first-party-shaped request
    let headers = build_headers(opts);
    let body = build_request_body(opts)?;

    // Construct endpoint URL
    let endpoint = format!("{}/v1/messages", opts.base_url);

    // Debug logging: Request construction details
    log_request_construction(&endpoint, &headers, &body, opts).await;

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

    // Create redacted response headers using allowlist for security
    let redacted_response_headers = redact_response_headers(&response_headers);

    Ok(OauthMasqueradeResult {
        status,
        duration_ms: duration.as_millis() as u32,
        breakdown,
        response_headers: redacted_response_headers,
        http_version,
    })
}

/// Test-only public exports for external test modules
/// These functions are only exported during testing to allow unit tests in other files
pub mod testing {
    pub use super::{
        build_headers, build_request_body, is_token_expired, redact_response_headers,
        OauthMasqueradeOptions, OauthMasqueradeResult, CLAUDE_CODE_SYSTEM_PROMPT,
        TEST_HEADERS_FILE,
    };
}

