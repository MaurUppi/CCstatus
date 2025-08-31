use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Update state file structure (stored as ccstatus-update.json)
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateStateFile {
    /// Last time we checked for updates
    pub last_check: Option<DateTime<Utc>>,
    /// Map of version → date when we last prompted about it (for daily de-duplication)
    pub version_prompt_dates: HashMap<String, DateTime<Utc>>,
    /// ETag cache by host
    pub etag_map: HashMap<String, String>,
    /// Last-Modified cache by host
    pub last_modified_map: HashMap<String, String>,
    /// Geographic detection result (true = China)
    pub geo_verdict: Option<bool>,
    /// When geographic detection was last performed
    pub geo_checked_at: Option<DateTime<Utc>>,
    /// Count of GREEN ticks since last update check
    pub green_ticks_since_check: u32,
    
    /// Legacy field for backward compatibility (migrate to version_prompt_dates)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_prompted_version: Option<String>,
}

impl UpdateStateFile {
    /// Load state from ccstatus-update.json with backward compatibility migration
    pub fn load() -> Self {
        let config_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("ccstatus");

        let state_file = config_dir.join("ccstatus-update.json");

        if let Ok(content) = std::fs::read_to_string(&state_file) {
            let mut state = serde_json::from_str::<UpdateStateFile>(&content).unwrap_or_default();
            // Migrate legacy last_prompted_version to version_prompt_dates
            if let Some(legacy_version) = state.last_prompted_version.take() {
                // Use yesterday's date to ensure it doesn't block today's prompt
                let yesterday = Utc::now() - chrono::Duration::days(1);
                state.version_prompt_dates.insert(legacy_version, yesterday);
            }
            state
        } else {
            Default::default()
        }
    }

    /// Update system triggered from COLD window
    pub fn tick_from_cold(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Debug logging when CCSTATUS_DEBUG=true
        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
            eprintln!("[DEBUG] UpdateStateFile::tick_from_cold() - COLD window update trigger activated");
        }

        if !self.should_check_for_updates() {
            if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                eprintln!("[DEBUG] UpdateStateFile::tick_from_cold() - throttled, skipping update check");
            }
            return Ok(()); // Throttled
        }

        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
            eprintln!("[DEBUG] UpdateStateFile::tick_from_cold() - performing update check");
        }

        // Perform update check with short timeout and silent failure
        match self.check_for_updates_internal() {
            Ok(update_available) => {
                if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                    eprintln!("[DEBUG] UpdateStateFile::tick_from_cold() - update check succeeded, update_available: {}", update_available);
                }
                self.update_last_check();
                self.save().ok(); // Silent failure on save
            }
            Err(e) => {
                if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                    eprintln!("[DEBUG] UpdateStateFile::tick_from_cold() - update check failed: {}", e);
                }
                // Silent failure as specified in plan
            }
        }
        Ok(())
    }

    /// Update system triggered from GREEN window
    pub fn tick_from_green(
        &mut self,
        _green_window_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Debug logging when CCSTATUS_DEBUG=true
        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
            eprintln!("[DEBUG] UpdateStateFile::tick_from_green() - GREEN window update trigger activated");
        }

        self.increment_green_ticks();

        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
            eprintln!("[DEBUG] UpdateStateFile::tick_from_green() - green_ticks_since_check: {}", self.green_ticks_since_check);
        }

        if self.should_trigger_green_check() {
            if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                eprintln!("[DEBUG] UpdateStateFile::tick_from_green() - threshold reached (12 ticks), performing update check");
            }
            // Perform update check when threshold reached
            match self.check_for_updates_internal() {
                Ok(update_available) => {
                    if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                        eprintln!("[DEBUG] UpdateStateFile::tick_from_green() - update check succeeded, update_available: {}", update_available);
                    }
                    self.update_last_check();
                }
                Err(e) => {
                    if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                        eprintln!("[DEBUG] UpdateStateFile::tick_from_green() - update check failed: {}", e);
                    }
                }
            }
            self.reset_green_ticks();
            self.save().ok(); // Silent failure on save
        } else {
            if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                eprintln!("[DEBUG] UpdateStateFile::tick_from_green() - threshold not reached, waiting for more GREEN ticks");
            }
        }
        Ok(())
    }

    /// Internal update check implementation
    fn check_for_updates_internal(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::updater::{geo, manifest::ManifestClient, url_resolver};

        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
            eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - starting update check");
        }

        // Get or update geographic detection
        let is_china = if self.is_geo_verdict_valid() {
            let cached_verdict = self.geo_verdict.unwrap_or(false);
            if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - using cached geo verdict: is_china={}", cached_verdict);
            }
            cached_verdict
        } else {
            if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - geo cache expired, detecting location");
            }
            let detected = geo::detect_china_ttl24h();
            if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - geo detection completed: is_china={}", detected);
            }
            self.update_geo_verdict(detected);
            detected
        };

        // Resolve URLs based on geography
        let urls = url_resolver::resolve_manifest_url(is_china);
        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
            eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - resolved {} URLs for is_china={}", urls.len(), is_china);
        }

        // Try each URL in sequence with persistent caching
        let mut client = ManifestClient::new();
        
        // Try URLs manually to track which one succeeds for proper host caching
        for (index, url) in urls.iter().enumerate() {
            if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - trying URL {}/{}: {}", index + 1, urls.len(), url);
            }

            match client.fetch_manifest_with_persistent_cache(url, &self.etag_map, &self.last_modified_map) {
                Ok((Some(manifest), new_etag, new_last_modified)) => {
                    if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                        eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - manifest fetched successfully from URL {}, version: {}", index + 1, manifest.version);
                    }

                    // Update persistent caches with new headers from successful URL
                    if let Some(host) = url_resolver::extract_host_from_url(url) {
                        if let Some(etag) = new_etag {
                            self.set_etag(host.clone(), etag);
                        }
                        if let Some(last_modified) = new_last_modified {
                            self.set_last_modified(host, last_modified);
                        }
                    }
                
                    // Check if version is newer
                    if client.is_newer_version(&manifest.version)?
                        && self.should_prompt_for_version(&manifest.version)
                    {
                        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                            eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - newer version available: {}, marking for prompt", manifest.version);
                        }
                        self.mark_version_prompted(manifest.version.clone());
                        // In V1, we only check and save state, no actual update
                        return Ok(true);
                    }
                    
                    if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                        eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - version {} is not newer or already prompted today", manifest.version);
                    }
                    return Ok(false);
                }
                Ok((None, _, _)) => {
                    if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                        eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - URL {} returned 304 Not Modified, no update available", index + 1);
                    }
                    // 304 Not Modified - short-circuit since all URLs point to same resource
                    return Ok(false);
                }
                Err(e) => {
                    if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
                        eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - URL {} failed: {}, trying next", index + 1, e);
                    }
                    // This URL failed, try next one
                    continue;
                }
            }
        }
        
        if crate::core::network::types::parse_env_bool("CCSTATUS_DEBUG") {
            eprintln!("[DEBUG] UpdateStateFile::check_for_updates_internal() - all URLs failed, update check unsuccessful");
        }
        
        // All URLs failed
        Ok(false)
    }

    /// Save state to ccstatus-update.json
    pub fn save(&self) -> Result<(), std::io::Error> {
        let config_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("ccstatus");

        std::fs::create_dir_all(&config_dir)?;
        let state_file = config_dir.join("ccstatus-update.json");

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&state_file, content)?;
        Ok(())
    }

    /// Check if we should throttle update checks (minimum 60 minutes)
    pub fn should_check_for_updates(&self) -> bool {
        if let Some(last_check) = self.last_check {
            let now = Utc::now();
            let minutes_passed = now.signed_duration_since(last_check).num_minutes();
            minutes_passed >= 60
        } else {
            true
        }
    }

    /// Check if we should prompt for this version (only once per day per version)
    pub fn should_prompt_for_version(&self, version: &str) -> bool {
        if let Some(last_prompted_date) = self.version_prompt_dates.get(version) {
            let now = Utc::now();
            let same_day = last_prompted_date.date_naive() == now.date_naive();
            if same_day {
                // Already prompted for this version today, don't prompt again
                return false;
            }
        }
        true
    }

    /// Update last check timestamp
    pub fn update_last_check(&mut self) {
        self.last_check = Some(Utc::now());
    }

    /// Mark version as prompted with current date
    pub fn mark_version_prompted(&mut self, version: String) {
        self.version_prompt_dates.insert(version, Utc::now());
    }

    /// Get ETag for host
    pub fn get_etag(&self, host: &str) -> Option<&String> {
        self.etag_map.get(host)
    }

    /// Set ETag for host
    pub fn set_etag(&mut self, host: String, etag: String) {
        self.etag_map.insert(host, etag);
    }

    /// Get Last-Modified for host
    pub fn get_last_modified(&self, host: &str) -> Option<&String> {
        self.last_modified_map.get(host)
    }

    /// Set Last-Modified for host
    pub fn set_last_modified(&mut self, host: String, last_modified: String) {
        self.last_modified_map.insert(host, last_modified);
    }

    /// Check if geographic detection is still valid (TTL 24h)
    pub fn is_geo_verdict_valid(&self) -> bool {
        if let Some(checked_at) = self.geo_checked_at {
            let now = Utc::now();
            let hours_passed = now.signed_duration_since(checked_at).num_hours();
            hours_passed < 24
        } else {
            false
        }
    }

    /// Update geographic detection result
    pub fn update_geo_verdict(&mut self, is_china: bool) {
        self.geo_verdict = Some(is_china);
        self.geo_checked_at = Some(Utc::now());
    }

    /// Increment GREEN tick counter
    pub fn increment_green_ticks(&mut self) {
        self.green_ticks_since_check += 1;
    }

    /// Check if GREEN ticks threshold reached (12 ticks)
    pub fn should_trigger_green_check(&self) -> bool {
        self.green_ticks_since_check >= 12
    }

    /// Reset GREEN tick counter
    pub fn reset_green_ticks(&mut self) {
        self.green_ticks_since_check = 0;
    }
}
