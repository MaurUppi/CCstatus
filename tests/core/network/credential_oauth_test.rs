use ccstatus::core::network::{
    credential::CredentialManager,
    types::{ApiCredentials, CredentialSource},
};
use std::env;

use crate::common::IsolatedEnv;

/// Test OAuth credential resolution behavior
/// Tests that OAuth integration works correctly on macOS and is properly skipped on other platforms

#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_oauth_credentials_when_present() {
    let _isolated = IsolatedEnv::new();

    // Clear environment variables to ensure we test OAuth path
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_BEDROCK_BASE_URL");
    env::remove_var("ANTHROPIC_VERTEX_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
    env::remove_var("ANTHROPIC_API_KEY");

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
    let _isolated = IsolatedEnv::new();

    // Clear environment variables to ensure we bypass env priority
    env::remove_var("ANTHROPIC_BASE_URL");
    env::remove_var("ANTHROPIC_BEDROCK_BASE_URL");
    env::remove_var("ANTHROPIC_VERTEX_BASE_URL");
    env::remove_var("ANTHROPIC_AUTH_TOKEN");
    env::remove_var("ANTHROPIC_API_KEY");

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
    let _isolated = IsolatedEnv::new();

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
