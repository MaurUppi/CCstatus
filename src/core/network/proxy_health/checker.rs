//! Proxy Health Assessment Logic
//!
//! Main orchestration for proxy health checking with:
//! - Root-based and path-based URL attempts  
//! - Fallback logic for improved success rates
//! - Optional redirect following with security validation
//! - Detailed outcome reporting for debugging

use crate::core::network::types::ProxyHealthDetail;
use crate::core::network::proxy_health::{
    client::{HealthCheckClient, HealthResponse},
    config::{ProxyHealthLevel, ProxyHealthOptions},
    parsing::{parse_health_response, detect_cloudflare_challenge},
    url::{build_root_health_url, build_path_health_url, extract_host, is_official_base_url},
};

#[derive(Debug, thiserror::Error)]
pub enum ProxyHealthError {
    #[error("URL construction failed: {0}")]
    UrlError(#[from] crate::core::network::proxy_health::url::UrlError),
    #[error("All health check attempts failed")]
    AllAttemptsFailed,
    #[error("Redirect validation failed: {0}")]
    RedirectValidationFailed(String),
}

// ProxyHealthDetail is now imported from types.rs

/// Comprehensive proxy health assessment outcome
#[derive(Debug, Clone)]
pub struct ProxyHealthOutcome {
    /// Determined health level (None = no health endpoint found)
    pub level: Option<ProxyHealthLevel>,
    /// Detailed information about the check process  
    pub detail: Option<ProxyHealthDetail>,
    /// Raw HTTP status code from successful response
    pub status_code: Option<u16>,
    /// Response body size in bytes
    pub response_size: Option<usize>,
}

/// Build ProxyHealthOutcome with optional response data
fn build_outcome_with_response(
    level: Option<ProxyHealthLevel>,
    detail: ProxyHealthDetail,
    response: &HealthResponse,
) -> ProxyHealthOutcome {
    ProxyHealthOutcome {
        level,
        detail: Some(detail),
        status_code: Some(response.status_code),
        response_size: Some(response.body.len()),
    }
}

/// Build ProxyHealthOutcome without response data
fn build_outcome_no_response(
    level: Option<ProxyHealthLevel>,
    detail: Option<ProxyHealthDetail>,
) -> ProxyHealthOutcome {
    ProxyHealthOutcome {
        level,
        detail,
        status_code: None,
        response_size: None,
    }
}

/// Assess proxy health using configurable strategy
///
/// Performs health check with fallback and redirect logic based on configuration.
/// Returns comprehensive outcome with detailed attempt information.
///
/// # Arguments
/// * `base_url` - Base URL of the proxy to check
/// * `options` - Configuration for URL construction and behavior
/// * `client` - HTTP client for making requests
///
/// # Returns  
/// * `Ok(ProxyHealthOutcome)` - Assessment completed (level may be None)
/// * `Err(ProxyHealthError)` - Unable to perform assessment
///
/// # Behavior
/// 1. Skip check if base_url is official Anthropic endpoint
/// 2. Try primary URL strategy (root vs path based on config)
/// 3. Try fallback URL if enabled and primary fails with 404
/// 4. Follow single redirect if enabled and response is 3xx
/// 5. Parse response body to determine health level
pub async fn assess_proxy_health(
    base_url: &str,
    options: &ProxyHealthOptions,
    client: &dyn HealthCheckClient,
) -> Result<ProxyHealthOutcome, ProxyHealthError> {
    let start_time = std::time::Instant::now();
    let checked_at = chrono::Local::now().to_rfc3339();

    // Skip proxy health check for official Anthropic endpoint
    if is_official_base_url(base_url) {
        return Ok(build_outcome_no_response(None, None));
    }

    // Determine primary and fallback URLs based on configuration
    let (primary_url, fallback_url) = if options.use_root_urls {
        (
            build_root_health_url(base_url)?,
            if options.try_fallback {
                Some(build_path_health_url(base_url))
            } else {
                None
            },
        )
    } else {
        (
            build_path_health_url(base_url),
            if options.try_fallback {
                Some(build_root_health_url(base_url)?)
            } else {
                None
            },
        )
    };

    let mut detail = ProxyHealthDetail {
        primary_url: primary_url.clone(),
        fallback_url: fallback_url.clone(),
        redirect_url: None,
        success_method: None,
        checked_at,
        response_time_ms: 0,
        reason: None,
    };

    let mut had_network_error = false;
    let mut had_404_response = false;
    
    // Attempt 1: Primary URL
    match client.get_health(primary_url.clone(), options.timeout_ms).await {
        Ok(response) => {
            if response.status_code == 404 {
                had_404_response = true;
            }
            
            // Check for redirect first (before consuming response)
            let is_redirect = (300..400).contains(&response.status_code);
            
            // Handle redirect from primary if enabled
            if is_redirect && options.follow_redirect_once {
                if let Some(outcome) = handle_redirect(&response, options, client, base_url, &mut detail, start_time).await? {
                    return Ok(outcome);
                }
            } else if let Some(outcome) = handle_response(response, "primary", &mut detail, start_time)? {
                return Ok(outcome);
            }
        }
        Err(_) => {
            had_network_error = true;
        }
    }

    // Attempt 2: Fallback URL (if configured)
    if let Some(fallback_url) = fallback_url {
        match client.get_health(fallback_url.clone(), options.timeout_ms).await {
            Ok(response) => {
                if response.status_code == 404 {
                    had_404_response = true;
                }
                
                // Check for redirect first (before consuming response)
                let is_redirect = (300..400).contains(&response.status_code);
                
                // Handle redirect from fallback if enabled
                if is_redirect && options.follow_redirect_once {
                    if let Some(outcome) = handle_redirect(&response, options, client, base_url, &mut detail, start_time).await? {
                        return Ok(outcome);
                    }
                } else if let Some(outcome) = handle_response(response, "fallback", &mut detail, start_time)? {
                    return Ok(outcome);
                }
            }
            Err(_) => {
                had_network_error = true;
            }
        }
    }

    // All attempts failed - determine appropriate response
    detail.response_time_ms = start_time.elapsed().as_millis() as u64;
    
    // If we only had network errors, treat as no proxy detected
    if had_network_error && !had_404_response {
        detail.reason = Some("timeout".to_string());
        return Ok(build_outcome_no_response(None, Some(detail)));
    }
    
    // If we only had 404 responses, treat as no health endpoint
    if had_404_response && !had_network_error {
        // Reason already set to "no_endpoint_404" in handle_response
        return Ok(ProxyHealthOutcome {
            level: None, // No health endpoint found
            detail: Some(detail),
            status_code: Some(404),
            response_size: Some(0),
        });
    }
    
    // Mixed failures - treat as no proxy detected (not Bad)
    detail.reason = Some("timeout".to_string());
    Ok(build_outcome_no_response(None, Some(detail)))
}

/// Handle successful HTTP response and determine health level
fn handle_response(
    response: HealthResponse,
    method: &str,
    detail: &mut ProxyHealthDetail,
    start_time: std::time::Instant,
) -> Result<Option<ProxyHealthOutcome>, ProxyHealthError> {
    detail.response_time_ms = start_time.elapsed().as_millis() as u64;
    
    match response.status_code {
        404 => {
            // No health endpoint, continue to fallback/redirect attempts
            detail.reason = Some("no_endpoint_404".to_string());
            Ok(None)
        }
        200 => {
            // Parse response body for health level
            let level = parse_health_response(&response.body);
            detail.success_method = Some(method.to_string());
            
            // Set reason based on parsing outcome
            match &level {
                Some(ProxyHealthLevel::Bad) => {
                    // Check if it's invalid JSON vs unknown schema
                    if serde_json::from_slice::<serde_json::Value>(&response.body).is_err() {
                        detail.reason = Some("invalid_json_200".to_string());
                    } else {
                        detail.reason = Some("unknown_schema_200".to_string());
                    }
                },
                Some(_) => {
                    // Healthy/Degraded - successful parsing
                    detail.reason = None;
                },
                None => {
                    // Empty/whitespace response - treat as no endpoint
                    detail.reason = Some("no_endpoint_404".to_string());
                }
            }
            
            Ok(Some(build_outcome_with_response(
                level,
                detail.clone(),
                &response,
            )))
        }
        403 | 503 | 429 => {
            // Check for Cloudflare challenge first
            if detect_cloudflare_challenge(response.status_code, &response.headers, &response.body) {
                detail.success_method = Some(method.to_string());
                detail.reason = Some("cloudflare_challenge".to_string());
                
                Ok(Some(build_outcome_with_response(
                    Some(ProxyHealthLevel::Unknown),
                    detail.clone(),
                    &response,
                )))
            } else if response.status_code == 429 {
                // Rate limited - proxy exists but degraded (unchanged)
                detail.success_method = Some(method.to_string());
                detail.reason = None; // Keep existing behavior
                
                Ok(Some(build_outcome_with_response(
                    Some(ProxyHealthLevel::Degraded),
                    detail.clone(),
                    &response,
                )))
            } else {
                // 403/503 without CF indicators - map to None (not Bad)
                detail.reason = Some("non_200_no_cf".to_string());
                Ok(None)
            }
        }
        500..=599 => {
            // Server errors - proxy exists but unhealthy (Bad level)
            detail.success_method = Some(method.to_string());
            detail.reason = Some("server_error".to_string());
            
            Ok(Some(build_outcome_with_response(
                Some(ProxyHealthLevel::Bad),
                detail.clone(),
                &response,
            )))
        }
        300..=399 if response.status_code != 304 => {
            // Redirect response, let caller handle it
            Ok(None)
        }
        _ => {
            // Other HTTP errors (3xx, 4xx/5xx except 404/429) - map to None (no proxy detected)
            detail.reason = Some("non_200_no_cf".to_string());
            Ok(None)
        }
    }
}

/// Handle redirect response if redirect following is enabled
async fn handle_redirect(
    response: &HealthResponse,
    options: &ProxyHealthOptions,
    client: &dyn HealthCheckClient,
    base_url: &str,
    detail: &mut ProxyHealthDetail,
    start_time: std::time::Instant,
) -> Result<Option<ProxyHealthOutcome>, ProxyHealthError> {
    // Extract Location header from response headers
    let location_url = match extract_location_header(&response.headers) {
        Some(url) => url,
        None => return Ok(None), // No location header, can't redirect
    };
    
    // Validate same-host redirect for security
    validate_redirect_host(base_url, &location_url)?;
    
    // Follow redirect once
    match client.get_health(location_url.clone(), options.timeout_ms).await {
        Ok(redirect_response) => {
            // Set redirect URL in detail for tracking
            detail.redirect_url = Some(location_url);
            
            // Handle redirect response (no further redirects allowed)
            let result = handle_response(redirect_response, "redirect", detail, start_time)?;
            
            // If redirect was successful (returned Some outcome), mark as redirect_followed
            if let Some(ref outcome) = result {
                if outcome.level.is_some() {
                    detail.reason = Some("redirect_followed".to_string());
                }
            }
            
            Ok(result)
        }
        Err(_) => {
            // Redirect failed, continue with other attempts
            detail.reason = Some("timeout".to_string());
            Ok(None)
        }
    }
}

/// Extract Location header from response headers
fn extract_location_header(headers: &std::collections::HashMap<String, String>) -> Option<String> {
    headers
        .iter()
        .find(|(key, _)| key.to_lowercase() == "location")
        .map(|(_, value)| value.clone())
}

/// Validate that redirect URL points to same host (security check)
fn validate_redirect_host(original_url: &str, redirect_url: &str) -> Result<(), ProxyHealthError> {
    let original_host = extract_host(original_url)
        .map_err(|e| ProxyHealthError::RedirectValidationFailed(format!("Invalid original URL: {}", e)))?;
        
    let redirect_host = extract_host(redirect_url)
        .map_err(|e| ProxyHealthError::RedirectValidationFailed(format!("Invalid redirect URL: {}", e)))?;

    if original_host != redirect_host {
        return Err(ProxyHealthError::RedirectValidationFailed(
            format!("Redirect to different host: {} -> {}", original_host, redirect_host)
        ));
    }

    Ok(())
}

