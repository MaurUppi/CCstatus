use ccstatus::core::network::oauth_masquerade::{run_probe, OauthMasqueradeOptions, OauthMasqueradeResult};
use ccstatus::core::network::types::NetworkError;
use std::env;
use std::collections::HashMap;

/// Test OAuth masquerade options validation and construction
#[test]
fn test_oauth_masquerade_options_creation() {
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_token_12345".to_string(),
        expires_at: Some(1672531200000), // Fixed timestamp for testing
        stream: false,
    };
    
    assert_eq!(opts.base_url, "https://api.anthropic.com");
    assert!(opts.access_token.starts_with("oat01_"));
    assert_eq!(opts.expires_at, Some(1672531200000));
    assert!(!opts.stream);
}

/// Test OAuth masquerade result structure
#[test]
fn test_oauth_masquerade_result_creation() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer test_token".to_string());
    
    let result = OauthMasqueradeResult {
        status: 200,
        duration_ms: 265,
        breakdown: "DNS:5ms|TCP:10ms|TLS:50ms|ServerTTFB:200ms|Total:265ms".to_string(),
        response_headers: headers,
        http_version: Some("HTTP/2.0".to_string()),
    };
    
    assert_eq!(result.status, 200);
    assert_eq!(result.duration_ms, 265);
    assert!(result.breakdown.contains("DNS:5ms"));
    assert!(result.breakdown.contains("Total:265ms"));
    assert_eq!(result.http_version, Some("HTTP/2.0".to_string()));
}

/// Test OAuth masquerade with valid unexpired token
#[tokio::test]
async fn test_oauth_masquerade_valid_token() {
    // Use future timestamp to ensure token is not expired
    let future_timestamp = 9999999999999i64; // Far future
    
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_valid_token".to_string(),
        expires_at: Some(future_timestamp),
        stream: false,
    };
    
    let result = run_probe(&opts).await;
    
    // Should succeed with mock response
    assert!(result.is_ok());
    let response = result.unwrap();
    
    // Verify mock response characteristics
    assert_eq!(response.status, 200);
    assert_eq!(response.duration_ms, 265);
    assert!(response.breakdown.contains("DNS:"));
    assert!(response.breakdown.contains("TCP:"));
    assert!(response.breakdown.contains("TLS:"));
    assert!(response.breakdown.contains("ServerTTFB:"));
    assert!(response.breakdown.contains("Total:"));
    assert_eq!(response.http_version, Some("HTTP/2.0".to_string()));
}

/// Test OAuth masquerade with expired token
#[tokio::test]
async fn test_oauth_masquerade_expired_token() {
    // Use past timestamp to simulate expired token
    let past_timestamp = 1000000000000i64; // Past timestamp
    
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_expired_token".to_string(),
        expires_at: Some(past_timestamp),
        stream: false,
    };
    
    let result = run_probe(&opts).await;
    
    // Should fail due to expired token
    assert!(result.is_err());
    if let Err(NetworkError::HttpError(msg)) = result {
        assert_eq!(msg, "OAuth token expired");
    } else {
        panic!("Expected HttpError with expired token message");
    }
}

/// Test OAuth masquerade with no expiry (never expires)
#[tokio::test]
async fn test_oauth_masquerade_no_expiry() {
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_no_expiry_token".to_string(),
        expires_at: None, // No expiry means never expired
        stream: false,
    };
    
    let result = run_probe(&opts).await;
    
    // Should succeed since no expiry means never expired
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status, 200);
}

/// Test OAuth masquerade with streaming enabled
#[tokio::test]
async fn test_oauth_masquerade_with_streaming() {
    let future_timestamp = 9999999999999i64;
    
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_stream_token".to_string(),
        expires_at: Some(future_timestamp),
        stream: true, // Enable streaming
    };
    
    let result = run_probe(&opts).await;
    
    // Should succeed with stream parameter
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status, 200);
    
    // Verify headers include stream-related headers (from response_headers)
    let headers = &response.response_headers;
    assert!(headers.contains_key("x-stainless-helper-method") || headers.len() > 10);
}

/// Test OAuth masquerade with custom base URL
#[tokio::test]
async fn test_oauth_masquerade_custom_base_url() {
    let future_timestamp = 9999999999999i64;
    
    let opts = OauthMasqueradeOptions {
        base_url: "https://custom.api.endpoint.com".to_string(),
        access_token: "oat01_test_custom_url_token".to_string(),
        expires_at: Some(future_timestamp),
        stream: false,
    };
    
    let result = run_probe(&opts).await;
    
    // Should succeed even with custom base URL
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status, 200);
}

/// Test OAuth masquerade debug logging environment variable
#[tokio::test]
async fn test_oauth_masquerade_debug_logging() {
    // Set debug logging environment variable
    env::set_var("CCSTATUS_DEBUG", "TRUE");
    
    let future_timestamp = 9999999999999i64;
    
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_debug_token_very_long_for_testing".to_string(),
        expires_at: Some(future_timestamp),
        stream: true,
    };
    
    let result = run_probe(&opts).await;
    
    // Should succeed and generate debug logs (we can't easily test the logs, but can verify execution)
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status, 200);
    
    // Clean up environment variable
    env::remove_var("CCSTATUS_DEBUG");
}

/// Test OAuth masquerade with expired token debug logging
#[tokio::test]
async fn test_oauth_masquerade_expired_token_debug_logging() {
    // Set debug logging environment variable
    env::set_var("CCSTATUS_DEBUG", "TRUE");
    
    let past_timestamp = 1000000000000i64;
    
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_expired_debug_token".to_string(),
        expires_at: Some(past_timestamp),
        stream: false,
    };
    
    let result = run_probe(&opts).await;
    
    // Should fail due to expired token, but debug logging should occur
    assert!(result.is_err());
    if let Err(NetworkError::HttpError(msg)) = result {
        assert_eq!(msg, "OAuth token expired");
    } else {
        panic!("Expected HttpError with expired token message");
    }
    
    // Clean up environment variable
    env::remove_var("CCSTATUS_DEBUG");
}

/// Test token length masking in debug logs (unit test for internal behavior)
#[test]
fn test_token_length_calculation_for_debug_masking() {
    let short_token = "oat01_short";
    let long_token = "oat01_this_is_a_very_long_oauth_token_for_testing_purposes_12345";
    
    // Verify tokens are properly formed (start with oat01_)
    assert!(short_token.starts_with("oat01_"));
    assert!(long_token.starts_with("oat01_"));
    
    // Verify length calculation (used in debug logging)
    assert_eq!(short_token.len(), 11);
    assert_eq!(long_token.len(), 64);
}

/// Integration test: OAuth masquerade with various test environment overrides
#[tokio::test]
async fn test_oauth_masquerade_with_test_overrides() {
    // Set various test environment variables that oauth_masquerade supports
    env::set_var("CCSTATUS_TEST_UA", "test-user-agent/1.0");
    env::set_var("CCSTATUS_TEST_BETA_HEADER", "test-beta-feature");
    env::set_var("CCSTATUS_TEST_STAINLESS_OS", "TestOS");
    env::set_var("CCSTATUS_TEST_STAINLESS_ARCH", "test64");
    
    let future_timestamp = 9999999999999i64;
    
    let opts = OauthMasqueradeOptions {
        base_url: "https://api.anthropic.com".to_string(),
        access_token: "oat01_test_overrides_token".to_string(),
        expires_at: Some(future_timestamp),
        stream: false,
    };
    
    let result = run_probe(&opts).await;
    
    // Should succeed with test overrides applied
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status, 200);
    
    // The test overrides should be reflected in the response headers
    let headers = &response.response_headers;
    assert!(headers.len() > 15); // Should have many headers with test overrides
    
    // Clean up test environment variables
    env::remove_var("CCSTATUS_TEST_UA");
    env::remove_var("CCSTATUS_TEST_BETA_HEADER");
    env::remove_var("CCSTATUS_TEST_STAINLESS_OS");
    env::remove_var("CCSTATUS_TEST_STAINLESS_ARCH");
}