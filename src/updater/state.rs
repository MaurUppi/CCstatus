use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Update state file structure (stored as ccstatus-update.json)
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateStateFile {
    /// Last time we checked for updates
    pub last_check: Option<DateTime<Utc>>,
    /// Last version we prompted the user about
    pub last_prompted_version: Option<String>,
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
}

impl Default for UpdateStateFile {
    fn default() -> Self {
        Self {
            last_check: None,
            last_prompted_version: None,
            etag_map: HashMap::new(),
            last_modified_map: HashMap::new(),
            geo_verdict: None,
            geo_checked_at: None,
            green_ticks_since_check: 0,
        }
    }
}

impl UpdateStateFile {
    /// Load state from ccstatus-update.json
    pub fn load() -> Self {
        let config_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("ccstatus");

        let state_file = config_dir.join("ccstatus-update.json");

        if let Ok(content) = std::fs::read_to_string(&state_file) {
            if let Ok(state) = serde_json::from_str::<UpdateStateFile>(&content) {
                state
            } else {
                Default::default()
            }
        } else {
            Default::default()
        }
    }

    /// Update system triggered from COLD window
    pub fn tick_from_cold(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.should_check_for_updates() {
            return Ok(()); // Throttled
        }

        // Perform update check with short timeout and silent failure
        match self.check_for_updates_internal() {
            Ok(_) => {
                self.update_last_check();
                self.save().ok(); // Silent failure on save
            }
            Err(_) => {
                // Silent failure as specified in plan
            }
        }
        Ok(())
    }

    /// Update system triggered from GREEN window
    pub fn tick_from_green(&mut self, _green_window_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.increment_green_ticks();
        
        if self.should_trigger_green_check() {
            // Perform update check when threshold reached
            if let Ok(_) = self.check_for_updates_internal() {
                self.update_last_check();
            }
            self.reset_green_ticks();
            self.save().ok(); // Silent failure on save
        }
        Ok(())
    }

    /// Internal update check implementation
    fn check_for_updates_internal(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        use crate::updater::{geo, url_resolver, manifest::ManifestClient};
        
        // Get or update geographic detection
        let is_china = if self.is_geo_verdict_valid() {
            self.geo_verdict.unwrap_or(false)
        } else {
            let detected = geo::detect_china_ttl24h();
            self.update_geo_verdict(detected);
            detected
        };

        // Resolve URLs based on geography
        let urls = url_resolver::resolve_manifest_url(is_china);
        
        // Try each URL in sequence
        let mut client = ManifestClient::new();
        for url in &urls {
            if let Ok(host) = url::Url::parse(url).map(|u| u.host_str().unwrap_or("").to_string()) {
                // Set cached headers if available
                if let Some(etag) = self.etag_map.get(&host) {
                    // ManifestClient handles ETag internally
                }
            }
        }
        
        // Use url_resolver helper to try URLs in sequence
        match url_resolver::try_urls_in_sequence(&urls, |url| {
            client.fetch_manifest(url)
        }) {
            Ok(Some(manifest)) => {
                // Check if version is newer
                if client.is_newer_version(&manifest.version)? {
                    if self.should_prompt_for_version(&manifest.version) {
                        self.mark_version_prompted(manifest.version.clone());
                        // In V1, we only check and save state, no actual update
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Ok(None) => {
                // No new version or 304 not modified
                Ok(false)
            }
            Err(e) => Err(e)
        }
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
        if let Some(ref last_prompted) = self.last_prompted_version {
            if last_prompted == version {
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

    /// Mark version as prompted
    pub fn mark_version_prompted(&mut self, version: String) {
        self.last_prompted_version = Some(version);
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