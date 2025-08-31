use ccstatus::updater::manifest::{Manifest, ManifestClient};
use std::collections::HashMap;

#[test]
fn test_manifest_parse_json() {
    let json = r#"{
        "version": "2.2.3",
        "notes_url": "https://github.com/MaurUppi/CCstatus/releases/tag/v2.2.3",
        "channel": "stable",
        "published_at": "2025-09-01T00:00:00Z",
        "assets": []
    }"#;
    
    let manifest = Manifest::from_json(json).unwrap();
    assert_eq!(manifest.version, "2.2.3");
    assert_eq!(manifest.channel, "stable");
    assert!(manifest.notes_url.contains("v2.2.3"));
    assert!(manifest.assets.is_empty());
}

#[test]
fn test_manifest_serialize_json() {
    let manifest = Manifest {
        version: "2.2.3".to_string(),
        notes_url: "https://github.com/MaurUppi/CCstatus/releases/tag/v2.2.3".to_string(),
        channel: "stable".to_string(),
        published_at: "2025-09-01T00:00:00Z".to_string(),
        assets: vec![],
    };
    
    let json = manifest.to_json().unwrap();
    assert!(json.contains("2.2.3"));
    assert!(json.contains("stable"));
    
    // Verify round-trip
    let parsed = Manifest::from_json(&json).unwrap();
    assert_eq!(parsed.version, manifest.version);
    assert_eq!(parsed.channel, manifest.channel);
}

#[test]
fn test_manifest_client_creation() {
    let client = ManifestClient::new();
    // Should create without panic
    // Internal state testing would require exposing internals
}

#[test]
fn test_manifest_client_version_comparison() {
    let client = ManifestClient::new();
    
    // Test version comparison logic
    // This assumes CARGO_PKG_VERSION is 2.2.2
    assert!(client.is_newer_version("2.2.3").unwrap()); // newer
    assert!(client.is_newer_version("3.0.0").unwrap()); // much newer
    assert!(!client.is_newer_version("2.2.2").unwrap()); // same
    assert!(!client.is_newer_version("2.2.1").unwrap()); // older
    assert!(!client.is_newer_version("1.0.0").unwrap()); // much older
}

#[test]
fn test_manifest_client_invalid_version() {
    let client = ManifestClient::new();
    
    // Should handle invalid semver gracefully
    assert!(client.is_newer_version("invalid-version").is_err());
    assert!(client.is_newer_version("").is_err());
    assert!(client.is_newer_version("v2.2.3-not-semver").is_err());
}

// Note: Network-dependent tests (HTTP 200/304/timeout) are handled separately
// in integration tests to avoid unreliable test results from network issues
// Mock-based testing would require refactoring to inject HTTP client dependencies