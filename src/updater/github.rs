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
        if should_use_static_binary() {
            "linux-x64-static.tar.gz".to_string()
        } else {
            "linux-x64.tar.gz".to_string()
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
                    if let Some((major, minor)) = parse_version(version_part) {
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
    let url = "https://api.github.com/repos/MaurUppi/CCstatus/releases/latest";

    let mut response = ureq::get(url)
        .header(
            "User-Agent",
            &format!("CCstatus/{}", env!("CARGO_PKG_VERSION")),
        )
        .call()?;

    if response.status() == 200 {
        let release: GitHubRelease = response.body_mut().read_json()?;

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
        Err(format!("HTTP {}", response.status()).into())
    }
}

