use std::fs;
use tempfile::TempDir;
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use ccstatus::updater::state::UpdateStateFile;

fn create_test_state_file(temp_dir: &TempDir, content: &str) -> std::path::PathBuf {
    let state_file = temp_dir.path().join("ccstatus-update.json");
    fs::write(&state_file, content).unwrap();
    state_file
}

#[test]
fn test_update_state_file_load_empty() {
    let state = UpdateStateFile::load();
    assert!(state.last_check.is_none());
    assert!(state.version_prompt_dates.is_empty());
    assert_eq!(state.green_ticks_since_check, 0);
    assert!(state.geo_verdict.is_none());
}

#[test]
fn test_update_state_file_save_and_load() {
    let temp_dir = tempfile::tempdir().unwrap();
    
    // Set up temporary state file path 
    let original_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", temp_dir.path());
    
    let mut state = UpdateStateFile::default();
    state.last_check = Some(Utc::now());
    state.mark_version_prompted("2.2.3".to_string());
    state.green_ticks_since_check = 5;
    state.geo_verdict = Some(true);
    state.geo_checked_at = Some(Utc::now());
    
    // Save and reload
    state.save().unwrap();
    let loaded_state = UpdateStateFile::load();
    
    assert!(loaded_state.last_check.is_some());
    assert!(loaded_state.version_prompt_dates.contains_key("2.2.3"));
    assert_eq!(loaded_state.green_ticks_since_check, 5);
    assert_eq!(loaded_state.geo_verdict, Some(true));
    
    // Restore original HOME
    if let Some(home) = original_home {
        std::env::set_var("HOME", home);
    } else {
        std::env::remove_var("HOME");
    }
}

#[test]
fn test_throttling_logic() {
    let mut state = UpdateStateFile::default();
    
    // Should check when no last_check
    assert!(state.should_check_for_updates());
    
    // Should not check immediately after setting last_check
    state.last_check = Some(Utc::now());
    assert!(!state.should_check_for_updates());
    
    // Should check after 60+ minutes
    state.last_check = Some(Utc::now() - ChronoDuration::minutes(61));
    assert!(state.should_check_for_updates());
    
    // Should not check within 60 minutes
    state.last_check = Some(Utc::now() - ChronoDuration::minutes(59));
    assert!(!state.should_check_for_updates());
}

#[test]
fn test_version_prompt_deduplication() {
    let mut state = UpdateStateFile::default();
    
    // Should prompt for new version
    assert!(state.should_prompt_for_version("2.2.3"));
    
    // Mark version as prompted
    state.mark_version_prompted("2.2.3".to_string());
    
    // Should not prompt for same version again (same day)
    assert!(!state.should_prompt_for_version("2.2.3"));
    
    // Should prompt for different version
    assert!(state.should_prompt_for_version("2.2.4"));
    
    // Simulate next day by modifying the stored date
    let yesterday = chrono::Utc::now() - chrono::Duration::days(1);
    state.version_prompt_dates.insert("2.2.5".to_string(), yesterday);
    
    // Should prompt again for version from yesterday
    assert!(state.should_prompt_for_version("2.2.5"));
}

#[test]
fn test_geo_verdict_ttl() {
    let mut state = UpdateStateFile::default();
    
    // No verdict initially
    assert!(!state.is_geo_verdict_valid());
    
    // Set fresh verdict
    state.update_geo_verdict(true);
    assert!(state.is_geo_verdict_valid());
    assert_eq!(state.geo_verdict, Some(true));
    
    // Simulate expired TTL (25 hours ago)
    state.geo_checked_at = Some(Utc::now() - ChronoDuration::hours(25));
    assert!(!state.is_geo_verdict_valid());
    
    // Fresh verdict should be valid (23 hours ago)
    state.geo_checked_at = Some(Utc::now() - ChronoDuration::hours(23));
    assert!(state.is_geo_verdict_valid());
}

#[test]
fn test_green_ticks_threshold() {
    let mut state = UpdateStateFile::default();
    
    // Should not trigger initially
    assert!(!state.should_trigger_green_check());
    assert_eq!(state.green_ticks_since_check, 0);
    
    // Increment to 11 - should not trigger
    for _ in 0..11 {
        state.increment_green_ticks();
    }
    assert_eq!(state.green_ticks_since_check, 11);
    assert!(!state.should_trigger_green_check());
    
    // Increment to 12 - should trigger
    state.increment_green_ticks();
    assert_eq!(state.green_ticks_since_check, 12);
    assert!(state.should_trigger_green_check());
    
    // Reset should clear counter
    state.reset_green_ticks();
    assert_eq!(state.green_ticks_since_check, 0);
    assert!(!state.should_trigger_green_check());
}

#[test]
fn test_etag_and_last_modified_caching() {
    let mut state = UpdateStateFile::default();
    
    // Set ETag for different hosts
    state.set_etag("raw.githubusercontent.com".to_string(), "W/\"abc123\"".to_string());
    state.set_etag("hk.gh-proxy.com".to_string(), "W/\"def456\"".to_string());
    
    // Set Last-Modified for different hosts
    state.set_last_modified("raw.githubusercontent.com".to_string(), "Wed, 21 Oct 2015 07:28:00 GMT".to_string());
    state.set_last_modified("hk.gh-proxy.com".to_string(), "Thu, 22 Oct 2015 08:30:00 GMT".to_string());
    
    // Verify host isolation
    assert_eq!(state.get_etag("raw.githubusercontent.com"), Some(&"W/\"abc123\"".to_string()));
    assert_eq!(state.get_etag("hk.gh-proxy.com"), Some(&"W/\"def456\"".to_string()));
    assert_eq!(state.get_etag("unknown.com"), None);
    
    assert_eq!(state.get_last_modified("raw.githubusercontent.com"), Some(&"Wed, 21 Oct 2015 07:28:00 GMT".to_string()));
    assert_eq!(state.get_last_modified("hk.gh-proxy.com"), Some(&"Thu, 22 Oct 2015 08:30:00 GMT".to_string()));
    assert_eq!(state.get_last_modified("unknown.com"), None);
}

#[test]
fn test_legacy_migration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let original_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", temp_dir.path());
    
    // Create a legacy state file with last_prompted_version
    let config_dir = temp_dir.path().join(".claude").join("ccstatus");
    fs::create_dir_all(&config_dir).unwrap();
    let state_file = config_dir.join("ccstatus-update.json");
    
    let legacy_content = r#"{
      "last_check": "2024-01-01T12:00:00Z",
      "last_prompted_version": "2.2.1",
      "green_ticks_since_check": 3,
      "etag_map": {},
      "last_modified_map": {},
      "geo_verdict": null,
      "geo_checked_at": null
    }"#;
    
    fs::write(&state_file, legacy_content).unwrap();
    
    // Load state - should migrate legacy field
    let state = UpdateStateFile::load();
    
    // Verify migration occurred
    assert!(state.last_prompted_version.is_none()); // Legacy field should be cleared
    assert!(state.version_prompt_dates.contains_key("2.2.1")); // Should be migrated
    assert_eq!(state.green_ticks_since_check, 3);
    
    // Restore original HOME
    if let Some(home) = original_home {
        std::env::set_var("HOME", home);
    } else {
        std::env::remove_var("HOME");
    }
}