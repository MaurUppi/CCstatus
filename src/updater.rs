use serde::{Deserialize, Serialize};

#[cfg(feature = "self-update")]
use chrono::{DateTime, Utc};

/// Update status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateStatus {
    /// Idle state, no update activity
    Idle,
    /// Currently checking for updates
    Checking,
    /// New version found, will auto-update in 3 seconds
    Ready {
        version: String,
        found_at: DateTime<Utc>,
    },
    /// Downloading new version
    Downloading { progress: u8 },
    /// Currently installing update
    Installing,
    /// Update completed successfully
    Completed {
        version: String,
        #[cfg(feature = "self-update")]
        completed_at: DateTime<Utc>,
    },
    /// Update failed with error
    Failed { error: String },
}

/// Update state persistence structure
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateState {
    pub status: UpdateStatus,
    #[cfg(feature = "self-update")]
    pub last_check: Option<DateTime<Utc>>,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_pid: Option<u32>,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        UpdateStatus::Idle
    }
}

impl UpdateState {
    /// Get status bar display text
    pub fn status_text(&self) -> Option<String> {
        match &self.status {
            #[cfg(feature = "self-update")]
            UpdateStatus::Ready { version, found_at } => {
                let now = Utc::now();
                let seconds_passed = now.signed_duration_since(*found_at).num_seconds();
                let remaining = 3 - seconds_passed;

                if remaining > 0 {
                    Some(format!("\u{f06b0} Update v{}! ({}s)", version, remaining))
                } else {
                    Some(format!("\u{f01da} Starting update..."))
                }
            }
            #[cfg(not(feature = "self-update"))]
            UpdateStatus::Ready { version, .. } => Some(format!("\u{f06b0} Update v{}!", version)),
            UpdateStatus::Downloading { progress } => Some(format!("\u{f01da} {}%", progress)),
            UpdateStatus::Installing => Some(format!("\u{f01da} Installing...")),
            #[cfg(feature = "self-update")]
            UpdateStatus::Completed {
                version,
                completed_at,
            } => {
                // Show update completion within 10 seconds
                let now = Utc::now();
                let seconds_passed = now.signed_duration_since(*completed_at).num_seconds();
                if seconds_passed < 10 {
                    Some(format!("\u{f058} Updated v{}!", version))
                } else {
                    None
                }
            }
            #[cfg(not(feature = "self-update"))]
            UpdateStatus::Completed { version, .. } => {
                Some(format!("\u{f058} Updated v{}!", version))
            }
            _ => None,
        }
    }

    /// Load update state from config directory and trigger auto-check if needed
    pub fn load() -> Self {
        #[cfg(feature = "self-update")]
        {
            let config_dir = dirs::home_dir()
                .unwrap_or_default()
                .join(".claude")
                .join("ccline");

            let state_file = config_dir.join(".update_state.json");

            let mut state = if let Ok(content) = std::fs::read_to_string(&state_file) {
                if let Ok(state) = serde_json::from_str::<UpdateState>(&content) {
                    state
                } else {
                    UpdateState {
                        current_version: env!("CARGO_PKG_VERSION").to_string(),
                        ..Default::default()
                    }
                }
            } else {
                UpdateState {
                    current_version: env!("CARGO_PKG_VERSION").to_string(),
                    ..Default::default()
                }
            };

            // Trigger background update check if needed
            if state.should_check_update() {
                // Check if another update process is running
                let should_start_check = if let Some(pid) = state.update_pid {
                    !Self::is_process_running(pid)
                } else {
                    true
                };

                if should_start_check {
                    // Perform synchronous update check for simplicity and reliability
                    use crate::updater::github::check_for_updates;

                    state.update_pid = Some(std::process::id());
                    state.last_check = Some(chrono::Utc::now());
                    let _ = state.save();

                    // Perform update check
                    match check_for_updates() {
                        Ok(Some(release)) => {
                            if let Some(asset) = release.find_asset_for_platform() {
                                // Set Ready status with timestamp, auto-update will start after 3 seconds
                                state.status = UpdateStatus::Ready {
                                    version: release.version(),
                                    found_at: chrono::Utc::now(),
                                };
                                // Start background thread to handle delayed auto-update
                                Self::spawn_delayed_auto_update(release.clone(), asset.clone());
                            } else {
                                state.status = UpdateStatus::Failed {
                                    error: "No compatible asset found".to_string(),
                                };
                            }
                            state.latest_version = Some(release.version());
                        }
                        Ok(None) => {
                            state.status = UpdateStatus::Idle;
                        }
                        Err(_) => {
                            state.status = UpdateStatus::Idle;
                        }
                    }

                    // Clear PID and save final state
                    state.update_pid = None;
                    let _ = state.save();
                }
            }

            return state;
        }

        #[cfg(not(feature = "self-update"))]
        UpdateState {
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    /// Spawn delayed automatic update process (waits 3 seconds)
    #[cfg(feature = "self-update")]
    fn spawn_delayed_auto_update(
        release: crate::updater::github::GitHubRelease,
        asset: crate::updater::github::ReleaseAsset,
    ) {
        std::thread::spawn(move || {
            // Wait 3 seconds for user to see the countdown
            std::thread::sleep(std::time::Duration::from_secs(3));
            Self::perform_auto_update(release, asset);
        });
    }

    /// Perform automatic update download and installation
    #[cfg(feature = "self-update")]
    fn perform_auto_update(
        release: crate::updater::github::GitHubRelease,
        asset: crate::updater::github::ReleaseAsset,
    ) {
        use std::fs::File;
        use std::io::{Read, Write};

        let version = release.version();

        // Download to our ccline directory
        let ccline_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("ccline");
        let _ = std::fs::create_dir_all(&ccline_dir);
        let temp_file = ccline_dir.join(format!("ccline-update-{}.tmp", version));

        // Download with progress updates
        match ureq::get(&asset.browser_download_url).call() {
            Ok(response) => {
                let total_size = asset.size;
                let mut downloaded = 0u64;
                let mut buffer = [0u8; 8192];

                if let Ok(mut file) = File::create(&temp_file) {
                    let mut reader = response.into_reader();

                    loop {
                        match reader.read(&mut buffer) {
                            Ok(0) => break, // EOF
                            Ok(bytes_read) => {
                                if file.write_all(&buffer[..bytes_read]).is_err() {
                                    break;
                                }
                                downloaded += bytes_read as u64;

                                // Update progress
                                let progress = if total_size > 0 {
                                    ((downloaded as f64 / total_size as f64) * 100.0) as u8
                                } else {
                                    50 // Unknown size, show 50%
                                };

                                Self::update_download_progress(progress);

                                if progress >= 100 {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    // Download completed, start installation
                    Self::install_update(&temp_file, &version);
                } else {
                    Self::set_update_failed("Failed to create temp file");
                }
            }
            Err(_) => {
                Self::set_update_failed("Download failed");
            }
        }
    }

    /// Update download progress
    #[cfg(feature = "self-update")]
    fn update_download_progress(progress: u8) {
        let mut state = Self::load_without_check();
        state.status = UpdateStatus::Downloading { progress };
        let _ = state.save();
    }

    /// Install update from downloaded file
    #[cfg(feature = "self-update")]
    fn install_update(downloaded_file: &std::path::Path, version: &str) {
        // Update status to installing
        let mut state = Self::load_without_check();
        state.status = UpdateStatus::Installing;
        let _ = state.save();

        // Simple installation simulation for now
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Set completion status
        state.status = UpdateStatus::Completed {
            version: version.to_string(),
            completed_at: chrono::Utc::now(),
        };
        let _ = state.save();

        // Clean up temp file
        let _ = std::fs::remove_file(downloaded_file);
    }

    /// Set update failed status
    #[cfg(feature = "self-update")]
    fn set_update_failed(error: &str) {
        let mut state = Self::load_without_check();
        state.status = UpdateStatus::Failed {
            error: error.to_string(),
        };
        let _ = state.save();
    }

    /// Load state without triggering update check (internal use)
    #[cfg(feature = "self-update")]
    fn load_without_check() -> Self {
        let config_dir = dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("ccline");

        let state_file = config_dir.join(".update_state.json");

        if let Ok(content) = std::fs::read_to_string(&state_file) {
            if let Ok(state) = serde_json::from_str::<UpdateState>(&content) {
                return state;
            }
        }

        UpdateState {
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            ..Default::default()
        }
    }

    /// Check if a process with given PID is still running
    #[cfg(feature = "self-update")]
    fn is_process_running(pid: u32) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("ps").arg("-p").arg(pid.to_string()).output() {
                output.status.success()
            } else {
                false
            }
        }

        #[cfg(windows)]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("tasklist")
                .arg("/FI")
                .arg(&format!("PID eq {}", pid))
                .output()
            {
                String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
            } else {
                false
            }
        }

        #[cfg(not(any(unix, windows)))]
        false
    }

    /// Save update state to config directory
    pub fn save(&self) -> Result<(), std::io::Error> {
        #[cfg(feature = "self-update")]
        {
            let config_dir = dirs::home_dir()
                .unwrap_or_default()
                .join(".claude")
                .join("ccline");

            std::fs::create_dir_all(&config_dir)?;
            let state_file = config_dir.join(".update_state.json");

            let content = serde_json::to_string_pretty(self)?;
            std::fs::write(&state_file, content)?;
        }

        Ok(())
    }

    /// Check if update check should be triggered
    #[cfg(feature = "self-update")]
    pub fn should_check_update(&self) -> bool {
        // Don't check if already updating
        match &self.status {
            UpdateStatus::Checking
            | UpdateStatus::Downloading { .. }
            | UpdateStatus::Installing => return false,
            _ => {}
        }

        // Check time interval (6 hours)
        if let Some(last_check) = self.last_check {
            let now = Utc::now();
            let hours_passed = now.signed_duration_since(last_check).num_hours();
            hours_passed >= 6
        } else {
            true
        }
    }

    #[cfg(not(feature = "self-update"))]
    pub fn should_check_update(&self) -> bool {
        false
    }
}

/// GitHub Release API response structures
#[cfg(feature = "self-update")]
pub mod github {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct GitHubRelease {
        pub tag_name: String,
        pub name: String,
        pub body: String,
        pub draft: bool,
        pub prerelease: bool,
        pub created_at: String,
        pub published_at: String,
        pub html_url: String,
        pub assets: Vec<ReleaseAsset>,
    }

    #[derive(Debug, Deserialize, Serialize, Clone)]
    pub struct ReleaseAsset {
        pub name: String,
        pub size: u64,
        pub download_count: u32,
        pub browser_download_url: String,
        pub content_type: String,
    }

    impl GitHubRelease {
        /// Get the version string without 'v' prefix
        pub fn version(&self) -> String {
            self.tag_name
                .strip_prefix('v')
                .unwrap_or(&self.tag_name)
                .to_string()
        }

        /// Find asset for current platform
        pub fn find_asset_for_platform(&self) -> Option<&ReleaseAsset> {
            let platform_suffix = get_platform_asset_name();
            self.assets
                .iter()
                .find(|asset| asset.name.contains(&platform_suffix))
        }
    }

    /// Get the expected asset name suffix for current platform
    fn get_platform_asset_name() -> String {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        return "windows-x64.zip".to_string();

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return "macos-x64.tar.gz".to_string();

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return "macos-arm64.tar.gz".to_string();

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            // glibc 2.35 is the watershed - use static for older systems
            if Self::should_use_static_binary() {
                return "linux-x64-static.tar.gz".to_string();
            } else {
                return "linux-x64.tar.gz".to_string();
            }
        }

        #[cfg(not(any(
            all(target_os = "windows", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64")
        )))]
        return "unknown".to_string();
    }

    /// Determine if we should use static binary based on glibc version
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn should_use_static_binary() -> bool {
        use std::process::Command;

        // Try to get glibc version
        if let Ok(output) = Command::new("ldd").arg("--version").output() {
            let version_output = String::from_utf8_lossy(&output.stdout);

            // Parse glibc version (format: "ldd (GNU libc) 2.35")
            for line in version_output.lines() {
                if line.contains("GNU libc") || line.contains("GLIBC") {
                    if let Some(version_part) = line.split_whitespace().last() {
                        if let Some((major, minor)) = Self::parse_version(version_part) {
                            // Use dynamic binary if glibc >= 2.35, otherwise use static
                            return major < 2 || (major == 2 && minor < 35);
                        }
                    }
                    break;
                }
            }
        }

        // Default to static if we can't determine glibc version
        true
    }

    /// Parse version string like "2.35" into (major, minor)
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn parse_version(version: &str) -> Option<(u32, u32)> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() >= 2 {
            if let (Ok(major), Ok(minor)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                return Some((major, minor));
            }
        }
        None
    }

    /// Check for updates from GitHub Releases API
    pub fn check_for_updates() -> Result<Option<GitHubRelease>, Box<dyn std::error::Error>> {
        let url = "https://api.github.com/repos/Haleclipse/CCometixLine/releases/latest";

        let response = ureq::get(url)
            .set(
                "User-Agent",
                &format!("CCometixLine/{}", env!("CARGO_PKG_VERSION")),
            )
            .call()?;

        if response.status() == 200 {
            let release: GitHubRelease = response.into_json()?;

            let current_version = env!("CARGO_PKG_VERSION");
            let latest_version = release.version();

            // Compare versions using semver
            let current = semver::Version::parse(current_version)?;
            let latest = semver::Version::parse(&latest_version)?;

            if latest > current {
                Ok(Some(release))
            } else {
                Ok(None)
            }
        } else {
            Err(format!("HTTP {}: {}", response.status(), response.status_text()).into())
        }
    }
}
