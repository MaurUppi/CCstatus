use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Manifest structure for update information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub notes_url: String,
    pub channel: String,
    pub published_at: String,
    #[serde(default)]
    pub assets: Vec<ManifestAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestAsset {
    pub name: String,
    pub size: u64,
    pub download_url: String,
    pub checksum: Option<String>,
}

impl Manifest {
    /// Parse manifest from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize manifest to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Manifest client for fetching update information
pub struct ManifestClient {
    client: ureq::Agent,
    etag_cache: HashMap<String, String>,
    last_modified_cache: HashMap<String, String>,
}

impl ManifestClient {
    /// Create new manifest client with timeouts
    pub fn new() -> Self {
        let client = ureq::Agent::new_with_defaults();

        Self {
            client,
            etag_cache: HashMap::new(),
            last_modified_cache: HashMap::new(),
        }
    }

    /// Fetch manifest from URL with ETag/Last-Modified caching
    pub fn fetch_manifest(&mut self, url: &str) -> Result<Option<Manifest>, Box<dyn std::error::Error>> {
        let mut request = self.client.get(url)
            .header("User-Agent", &format!("CCstatus/{}", env!("CARGO_PKG_VERSION")));

        // Add conditional headers if we have cached values
        if let Some(etag) = self.etag_cache.get(url) {
            request = request.header("If-None-Match", etag);
        }
        if let Some(last_modified) = self.last_modified_cache.get(url) {
            request = request.header("If-Modified-Since", last_modified);
        }

        match request.call() {
            Ok(mut response) => {
                if response.status().as_u16() == 200 {
                    // Update cache with new ETag/Last-Modified
                    if let Some(etag) = response.headers().get("ETag") {
                        self.etag_cache.insert(url.to_string(), etag.to_str().unwrap_or("").to_string());
                    }
                    if let Some(last_modified) = response.headers().get("Last-Modified") {
                        self.last_modified_cache.insert(url.to_string(), last_modified.to_str().unwrap_or("").to_string());
                    }

                    let manifest_text = response.body_mut().read_to_string()?;
                    let manifest = Manifest::from_json(&manifest_text)?;
                    Ok(Some(manifest))
                } else if response.status().as_u16() == 304 {
                    // Not modified, no new version
                    Ok(None)
                } else {
                    Err(format!("HTTP {}", response.status().as_u16()).into())
                }
            }
            Err(e) => Err(e.into())
        }
    }

    /// Compare version with current using semver
    pub fn is_newer_version(&self, manifest_version: &str) -> Result<bool, semver::Error> {
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
        let latest = semver::Version::parse(manifest_version)?;
        Ok(latest > current)
    }
}

impl Default for ManifestClient {
    fn default() -> Self {
        Self::new()
    }
}