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
}

impl ManifestClient {
    /// Create new manifest client with strict timeouts for silent failure
    pub fn new() -> Self {
        let client: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(3)))
            .build()
            .into();

        Self { client }
    }


    /// Compare version with current using semver
    pub fn is_newer_version(&self, manifest_version: &str) -> Result<bool, semver::Error> {
        let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
        let latest = semver::Version::parse(manifest_version)?;
        Ok(latest > current)
    }

    /// Uniform helper for extracting headers from HTTP responses
    fn get_header(response: &ureq::Response, header_name: &str) -> Option<String> {
        response.header(header_name).map(|v| v.to_string())
    }

    /// Fetch manifest with persistent host-based caching from UpdateStateFile
    pub fn fetch_manifest_with_persistent_cache(
        &mut self,
        url: &str,
        persistent_etag_map: &std::collections::HashMap<String, String>,
        persistent_last_modified_map: &std::collections::HashMap<String, String>,
    ) -> Result<(Option<Manifest>, Option<String>, Option<String>), Box<dyn std::error::Error>> {
        use crate::updater::url_resolver;

        let host = url_resolver::extract_host_from_url(url).unwrap_or_else(|| url.to_string());
        
        let mut request = self.client.get(url).header(
            "User-Agent",
            &format!("CCstatus/{}", env!("CARGO_PKG_VERSION")),
        );

        // Use persistent cache values for conditional requests
        if let Some(etag) = persistent_etag_map.get(&host) {
            request = request.header("If-None-Match", etag);
        }
        if let Some(last_modified) = persistent_last_modified_map.get(&host) {
            request = request.header("If-Modified-Since", last_modified);
        }

        match request.call() {
            Ok(response) => {
                if response.status().as_u16() == 200 {
                    // Extract new cache headers for persistence
                    let new_etag = Self::get_header(&response, "ETag");
                    let new_last_modified = Self::get_header(&response, "Last-Modified");

                    let manifest_text = response.into_string()?;
                    let manifest = Manifest::from_json(&manifest_text)?;
                    Ok((Some(manifest), new_etag, new_last_modified))
                } else if response.status().as_u16() == 304 {
                    // Not modified, no new version
                    Ok((None, None, None))
                } else {
                    Err(format!("HTTP {}", response.status().as_u16()).into())
                }
            }
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for ManifestClient {
    fn default() -> Self {
        Self::new()
    }
}
