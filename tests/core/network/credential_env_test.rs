use ccstatus::core::network::{
    credential::CredentialManager,
    types::{ApiCredentials, CredentialSource},
};
use std::env;

use crate::common::IsolatedEnv;

/// Test environment variable combination rules
/// Tests the priority hierarchy for environment variables:
/// - Base URL priority: ANTHROPIC_BASE_URL > ANTHROPIC_BEDROCK_BASE_URL > ANTHROPIC_VERTEX_BASE_URL  
/// - Token priority: ANTHROPIC_AUTH_TOKEN > ANTHROPIC_API_KEY
/// - Return credentials when one base URL and one token are both present

#[tokio::test]
async fn test_env_primary_combination() {
    let _isolated = IsolatedEnv::new();

    // Case 1: Only ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN → Some
    env::set_var("ANTHROPIC_BASE_URL", "https://api.anthropic.com");
    env::set_var("ANTHROPIC_AUTH_TOKEN", "test-token-123");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");

    assert!(
        result.is_some(),
        "Should find primary environment combination"
    );
    let creds = result.unwrap();
    assert_eq!(creds.base_url, "https://api.anthropic.com");
    assert_eq!(creds.auth_token, "test-token-123");
    assert!(matches!(creds.source, CredentialSource::Environment));
}

#[tokio::test]
async fn test_env_secondary_combination() {
    let _isolated = IsolatedEnv::new();

    // Case 2: ANTHROPIC_BEDROCK_BASE_URL + ANTHROPIC_API_KEY → Some
    env::set_var(
        "ANTHROPIC_BEDROCK_BASE_URL",
        "https://bedrock.amazonaws.com",
    );
    env::set_var("ANTHROPIC_API_KEY", "test-api-key-456");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");

    assert!(
        result.is_some(),
        "Should find secondary environment combination"
    );
    let creds = result.unwrap();
    assert_eq!(creds.base_url, "https://bedrock.amazonaws.com");
    assert_eq!(creds.auth_token, "test-api-key-456");
    assert!(matches!(creds.source, CredentialSource::Environment));
}

#[tokio::test]
async fn test_env_tertiary_combination() {
    let _isolated = IsolatedEnv::new();

    // Test ANTHROPIC_VERTEX_BASE_URL + ANTHROPIC_API_KEY → Some
    env::set_var(
        "ANTHROPIC_VERTEX_BASE_URL",
        "https://vertex-ai.googleapis.com",
    );
    env::set_var("ANTHROPIC_API_KEY", "test-vertex-key-789");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");

    assert!(
        result.is_some(),
        "Should find tertiary environment combination"
    );
    let creds = result.unwrap();
    assert_eq!(creds.base_url, "https://vertex-ai.googleapis.com");
    assert_eq!(creds.auth_token, "test-vertex-key-789");
    assert!(matches!(creds.source, CredentialSource::Environment));
}

#[tokio::test]
async fn test_env_incomplete_combinations() {
    let _isolated = IsolatedEnv::new();

    // Case 3: Only base URL or only token → None

    // Test: Only base URL, no token
    env::set_var("ANTHROPIC_BASE_URL", "https://api.anthropic.com");
    // No token set

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");
    assert!(
        result.is_none(),
        "Should not find credentials with only base URL"
    );

    // Clean up and test opposite case
    env::remove_var("ANTHROPIC_BASE_URL");
    env::set_var("ANTHROPIC_AUTH_TOKEN", "test-token-123");
    // No base URL set

    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");
    assert!(
        result.is_none(),
        "Should not find credentials with only token"
    );
}

#[tokio::test]
async fn test_env_priority_rules() {
    let _isolated = IsolatedEnv::new();

    // Case 4: All present → prefer primary keys (BASE_URL + AUTH_TOKEN)
    env::set_var("ANTHROPIC_BASE_URL", "https://api.anthropic.com"); // Primary (highest)
    env::set_var(
        "ANTHROPIC_BEDROCK_BASE_URL",
        "https://bedrock.amazonaws.com",
    ); // Secondary
    env::set_var(
        "ANTHROPIC_VERTEX_BASE_URL",
        "https://vertex-ai.googleapis.com",
    ); // Tertiary

    env::set_var("ANTHROPIC_AUTH_TOKEN", "primary-token-123"); // Primary (highest)
    env::set_var("ANTHROPIC_API_KEY", "secondary-api-key-456"); // Secondary

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");

    assert!(
        result.is_some(),
        "Should find credentials when all are present"
    );
    let creds = result.unwrap();

    // Should prefer primary base URL and primary token
    assert_eq!(
        creds.base_url, "https://api.anthropic.com",
        "Should prefer ANTHROPIC_BASE_URL"
    );
    assert_eq!(
        creds.auth_token, "primary-token-123",
        "Should prefer ANTHROPIC_AUTH_TOKEN"
    );
    assert!(matches!(creds.source, CredentialSource::Environment));
}

#[tokio::test]
async fn test_env_mixed_priority_combinations() {
    let _isolated = IsolatedEnv::new();

    // Test: Primary base URL missing, secondary present + primary token
    env::set_var(
        "ANTHROPIC_BEDROCK_BASE_URL",
        "https://bedrock.amazonaws.com",
    ); // Secondary base URL
    env::set_var("ANTHROPIC_AUTH_TOKEN", "primary-token-123"); // Primary token

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");

    assert!(result.is_some(), "Should find mixed priority combination");
    let creds = result.unwrap();
    assert_eq!(
        creds.base_url, "https://bedrock.amazonaws.com",
        "Should use available secondary base URL"
    );
    assert_eq!(
        creds.auth_token, "primary-token-123",
        "Should use primary token"
    );
    assert!(matches!(creds.source, CredentialSource::Environment));
}

#[tokio::test]
async fn test_env_all_secondary_combinations() {
    let _isolated = IsolatedEnv::new();

    // Test: Primary missing, only secondary available
    env::set_var(
        "ANTHROPIC_VERTEX_BASE_URL",
        "https://vertex-ai.googleapis.com",
    ); // Tertiary base URL
    env::set_var("ANTHROPIC_API_KEY", "secondary-api-key-456"); // Secondary token

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");

    assert!(result.is_some(), "Should find all-secondary combination");
    let creds = result.unwrap();
    assert_eq!(
        creds.base_url, "https://vertex-ai.googleapis.com",
        "Should use tertiary base URL"
    );
    assert_eq!(
        creds.auth_token, "secondary-api-key-456",
        "Should use secondary token"
    );
    assert!(matches!(creds.source, CredentialSource::Environment));
}

#[tokio::test]
async fn test_env_empty_values() {
    let _isolated = IsolatedEnv::new();

    // Test that empty strings are treated as missing
    env::set_var("ANTHROPIC_BASE_URL", "");
    env::set_var("ANTHROPIC_AUTH_TOKEN", "valid-token");

    let cm = CredentialManager::new().expect("Failed to create CredentialManager");
    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");

    assert!(
        result.is_none(),
        "Should not find credentials with empty base URL"
    );

    // Test opposite case
    env::set_var("ANTHROPIC_BASE_URL", "https://api.anthropic.com");
    env::set_var("ANTHROPIC_AUTH_TOKEN", "");

    let result = cm
        .get_from_environment()
        .expect("Environment check should not fail");
    assert!(
        result.is_none(),
        "Should not find credentials with empty token"
    );
}
