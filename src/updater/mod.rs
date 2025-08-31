use serde::{Deserialize, Serialize};

#[cfg(feature = "self-update")]
use chrono::{DateTime, Utc};

/// Update status enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum UpdateStatus {
    /// Idle state, no update activity
    #[default]
    Idle,
    /// Currently checking for updates
    Checking,
    /// New version found, manual update required
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

impl UpdateState {
    /// Get status bar display text
    pub fn status_text(&self) -> Option<String> {
        match &self.status {
            #[cfg(feature = "self-update")]
            UpdateStatus::Ready { version, .. } => Some(format!("\u{f06b0} Update v{}!", version)),
            #[cfg(not(feature = "self-update"))]
            UpdateStatus::Ready { version, .. } => Some(format!("\u{f06b0} Update v{}!", version)),
            UpdateStatus::Downloading { progress } => Some(format!("\u{f01da} {}%", progress)),
            UpdateStatus::Installing => Some("\u{f01da} Installing...".to_string()),
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
                .join("ccstatus");

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
                            if release.find_asset_for_platform().is_some() {
                                // Set Ready status with timestamp, user must run --update manually
                                state.status = UpdateStatus::Ready {
                                    version: release.version(),
                                    found_at: chrono::Utc::now(),
                                };
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

            state
        }

        #[cfg(not(feature = "self-update"))]
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
                .join("ccstatus");

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

        // Check time interval (1 hour)
        if let Some(last_check) = self.last_check {
            let now = Utc::now();
            let hours_passed = now.signed_duration_since(last_check).num_hours();
            hours_passed >= 1
        } else {
            true
        }
    }

    #[cfg(not(feature = "self-update"))]
    pub fn should_check_update(&self) -> bool {
        false
    }
}

/// GitHub Release API response structures (moved to submodule)
#[cfg(feature = "self-update")]
pub mod github;

/// New V1 update system modules
#[cfg(feature = "self-update")]
pub mod manifest;
#[cfg(feature = "self-update")]
pub mod state;
#[cfg(feature = "self-update")]
pub mod geo;
#[cfg(feature = "self-update")]
pub mod url_resolver;

/// Re-export public types for compatibility
#[cfg(feature = "self-update")]
pub use manifest::{Manifest, ManifestClient};
#[cfg(feature = "self-update")]
pub use state::UpdateStateFile;
