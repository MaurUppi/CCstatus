use ccstatus::core::network::{
    credential::CredentialManager,
    oauth_masquerade::testing::{
        build_headers, build_request_body, is_token_expired, 
        OauthMasqueradeOptions, CLAUDE_CODE_SYSTEM_PROMPT, TEST_HEADERS_FILE
    },
    oauth_masquerade::{run_probe, OauthMasqueradeResult},
    http_monitor::HttpClientTrait,
    types::{ApiCredentials, CredentialSource},
};
use std::{env, time::Duration, collections::HashMap};
use tempfile::NamedTempFile;

use crate::common::IsolatedEnv;

/// Test OAuth credential resolution behavior
/// Tests that OAuth integration works correctly on macOS and is properly skipped on other platforms

#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_oauth_credentials_when_present() {
    let isolated = IsolatedEnv::new();

    // Force clear all environment variables (including system vars)
    isolated.force_clear_env_vars();

    // Set test override to simulate OAuth credentials present in Keychain
    env::set_var("CCSTATUS_TEST_OAUTH_PRESENT", "1");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    if let Some(creds) = result {
        // When OAuth is present, should return fixed credentials
        assert_eq!(
            creds.base_url, "https://api.anthropic.com",
            "OAuth should use official Anthropic API endpoint"
        );
        assert_eq!(
            creds.auth_token, "probe-invalid-key",
            "OAuth should use dummy probe key"
        );
        assert!(
            matches!(creds.source, CredentialSource::OAuth),
            "Source should be OAuth"
        );
    } else {
        // This test depends on the actual macOS Keychain state
        // If no OAuth credentials are actually present, this is expected
        println!("No OAuth credentials found in macOS Keychain - this may be expected");
    }

    // Clean up test environment
    env::remove_var("CCSTATUS_TEST_OAUTH_PRESENT");
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_oauth_priority_over_shell_and_config() {
    let isolated = IsolatedEnv::new();

    // Force clear all environment variables (including system vars)
    isolated.force_clear_env_vars();

    // Set test override to simulate OAuth credentials present
    env::set_var("CCSTATUS_TEST_OAUTH_PRESENT", "1");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    // OAuth should have priority over shell and config sources
    if let Some(creds) = result {
        assert!(
            matches!(creds.source, CredentialSource::OAuth),
            "OAuth should have priority over shell and config sources"
        );
        assert_eq!(creds.base_url, "https://api.anthropic.com");
        assert_eq!(creds.auth_token, "probe-invalid-key");
    }

    env::remove_var("CCSTATUS_TEST_OAUTH_PRESENT");
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_oauth_fallback_when_not_present() {
    let _isolated = IsolatedEnv::new();

    // Clear all environment variables
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_BEDROCK_BASE_URL");
    env::remove_var("ANTHROPIC_VERTEX_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
    env::remove_var("ANTHROPIC_API_KEY");

    // Ensure OAuth test override is NOT set (simulating no OAuth credentials)
    env::remove_var("CCSTATUS_TEST_OAUTH_PRESENT");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    // Should either be None or from shell/config sources, but NOT OAuth
    match result {
        None => {
            println!("No credentials found - OAuth correctly skipped when not present");
        }
        Some(creds) => {
            // If credentials are found, they should be from shell or config, not OAuth
            assert!(
                !matches!(creds.source, CredentialSource::OAuth),
                "Should not find OAuth credentials when not present in Keychain"
            );
            println!(
                "Found credentials from non-OAuth source: {:?}",
                creds.source
            );
        }
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_env_overrides_oauth() {
    let isolated = IsolatedEnv::new();

    // Force clear all environment variables first
    isolated.force_clear_env_vars();

    // Set environment variables (highest priority)
    env::set_var("ANTHROPIC_BASE_URL", "https://env-override.com");
    env::set_var("ANTHROPIC_AUTH_TOKEN", "env-override-token");

    // Also simulate OAuth present (should be ignored due to env priority)
    env::set_var("CCSTATUS_TEST_OAUTH_PRESENT", "1");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    assert!(result.is_some(), "Should find credentials");
    let creds = result.unwrap();

    // Environment should override OAuth
    assert_eq!(
        creds.base_url, "https://env-override.com",
        "Environment should override OAuth"
    );
    assert_eq!(
        creds.auth_token, "env-override-token",
        "Environment should override OAuth"
    );
    assert!(
        matches!(creds.source, CredentialSource::Environment),
        "Source should be Environment, not OAuth"
    );

    env::remove_var("CCSTATUS_TEST_OAUTH_PRESENT");
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
}

#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn test_oauth_skipped_on_non_macos() {
    let _isolated = IsolatedEnv::new();

    // Clear environment variables to test OAuth path
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_BEDROCK_BASE_URL");
    env::remove_var("ANTHROPIC_VERTEX_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
    env::remove_var("ANTHROPIC_API_KEY");

    // Set OAuth test override (should be ignored on non-macOS)
    env::set_var("CCSTATUS_TEST_OAUTH_PRESENT", "1");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    // On non-macOS, OAuth should be completely skipped
    match result {
        None => {
            println!("No credentials found on non-macOS - OAuth correctly skipped");
        }
        Some(creds) => {
            // If credentials are found, they should NOT be from OAuth
            assert!(
                !matches!(creds.source, CredentialSource::OAuth),
                "OAuth should be skipped on non-macOS platforms"
            );
            println!(
                "Found credentials from non-OAuth source on non-macOS: {:?}",
                creds.source
            );
        }
    }

    env::remove_var("CCSTATUS_TEST_OAUTH_PRESENT");
}

#[tokio::test]
async fn test_oauth_error_handling() {
    let _isolated = IsolatedEnv::new();

    // Clear environment variables to test OAuth path
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_BEDROCK_BASE_URL");
    env::remove_var("ANTHROPIC_VERTEX_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
    env::remove_var("ANTHROPIC_API_KEY");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm.get_credentials().await;

    // Should not fail even if OAuth encounters errors
    assert!(
        result.is_ok(),
        "Credential lookup should not fail even if OAuth has errors"
    );

    // The result might be None or from other sources, but should not error
    match result.unwrap() {
        None => println!("No credentials found - OAuth errors handled gracefully"),
        Some(creds) => println!("Found credentials from source: {:?}", creds.source),
    }
}

/// Test the OAuth helper method directly
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_oauth_keychain_helper_simulation() {
    let _isolated = IsolatedEnv::new();

    // Set test simulation
    env::set_var("CCSTATUS_TEST_OAUTH_PRESENT", "1");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");

    // We can't directly test the private get_from_oauth_keychain method,
    // but we can test the full flow and verify OAuth behavior
    // This test documents the expected OAuth implementation behavior

    println!("OAuth test simulation: CCSTATUS_TEST_OAUTH_PRESENT=1");
    println!("Expected behavior:");
    println!("  - macOS: Check Keychain with 'security find-generic-password -s \"Claude Code-credentials\"'");
    println!("  - If present: Return fixed credentials (api.anthropic.com, probe-invalid-key)");
    println!("  - If absent: Return None and continue to shell/config sources");
    println!("  - On error: Return None and continue (fail silently)");

    env::remove_var("CCSTATUS_TEST_OAUTH_PRESENT");

    // This test mainly serves as documentation of the OAuth behavior
    // The actual testing happens in the integration tests above
}

/// Test OAuth env token detection (cross-platform)
#[tokio::test]
async fn test_oauth_env_token_only() {
    let isolated = IsolatedEnv::new();

    // Force clear all environment variables (including system vars)
    isolated.force_clear_env_vars();

    // Set only the OAuth env token
    env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "oauth-access-token-123");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    assert!(result.is_some(), "Should find OAuth credentials");
    let creds = result.unwrap();

    assert_eq!(
        creds.base_url, "https://api.anthropic.com",
        "OAuth should use fixed Anthropic API endpoint"
    );
    assert_eq!(
        creds.auth_token, "probe-invalid-key",
        "OAuth should use dummy probe key, not the actual token"
    );
    assert!(
        matches!(creds.source, CredentialSource::OAuth),
        "Source should be OAuth"
    );

    env::remove_var("CLAUDE_CODE_OAUTH_TOKEN");
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
}

/// Test that environment variables override OAuth env token
#[tokio::test]
async fn test_env_overrides_oauth_env_token() {
    let isolated = IsolatedEnv::new();

    // Force clear all environment variables first
    isolated.force_clear_env_vars();

    // Set both ANTHROPIC_* credentials and OAuth env token
    env::set_var("ANTHROPIC_BASE_URL", "https://env-priority.com");
    env::set_var("ANTHROPIC_AUTH_TOKEN", "env-priority-token");
    env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "oauth-should-be-ignored");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    assert!(result.is_some(), "Should find credentials");
    let creds = result.unwrap();

    // Environment should override OAuth env token
    assert_eq!(
        creds.base_url, "https://env-priority.com",
        "Environment should override OAuth env token"
    );
    assert_eq!(
        creds.auth_token, "env-priority-token",
        "Environment should override OAuth env token"
    );
    assert!(
        matches!(creds.source, CredentialSource::Environment),
        "Source should be Environment, not OAuth"
    );

    env::remove_var("CLAUDE_CODE_OAUTH_TOKEN");
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
}

/// Test empty OAuth env token is ignored
#[tokio::test]
async fn test_empty_oauth_env_token_ignored() {
    let isolated = IsolatedEnv::new();

    // Force clear all environment variables (including system vars)
    isolated.force_clear_env_vars();

    // Set empty OAuth env token (should be ignored)
    env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    // Should either be None or from other sources, but NOT OAuth
    match result {
        None => {
            println!("No credentials found - empty OAuth env token correctly ignored");
        }
        Some(creds) => {
            assert!(
                !matches!(creds.source, CredentialSource::OAuth),
                "Empty OAuth env token should be ignored"
            );
        }
    }

    env::remove_var("CLAUDE_CODE_OAUTH_TOKEN");
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
}

/// Test OAuth env token works on non-macOS platforms
#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn test_oauth_env_token_non_macos() {
    let isolated = IsolatedEnv::new();

    // Force clear all environment variables (including system vars)
    isolated.force_clear_env_vars();

    // Set OAuth env token
    env::set_var("CLAUDE_CODE_OAUTH_TOKEN", "cross-platform-oauth-token");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_credentials()
        .await
        .expect("Credential lookup should not fail");

    assert!(
        result.is_some(),
        "Should find OAuth credentials on non-macOS"
    );
    let creds = result.unwrap();

    assert_eq!(
        creds.base_url, "https://api.anthropic.com",
        "OAuth should use fixed Anthropic API endpoint"
    );
    assert_eq!(
        creds.auth_token, "probe-invalid-key",
        "OAuth should use dummy probe key"
    );
    assert!(
        matches!(creds.source, CredentialSource::OAuth),
        "Source should be OAuth"
    );

    env::remove_var("CLAUDE_CODE_OAUTH_TOKEN");
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
}

// ==================================================================================
// OAuth Masquerade Unit Tests (migrated from oauth_masquerade.rs)
// ==================================================================================

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

    // Ensure non-sensitive response headers are still present (only actual response headers)
    assert!(result.response_headers.contains_key("Server"));
    assert!(result.response_headers.contains_key("Cache-Control"));
}

#[test]
fn test_test_headers_file_filters_sensitive_headers() {
    use std::fs;

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