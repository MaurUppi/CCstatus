/// URL resolver for manifest fetching with geographic optimization and robust fallback strategy
///
/// This module provides intelligent URL resolution for fetching update manifests with:
/// - Geographic optimization for China users via proxy servers
/// - Multiple CDN fallbacks for high availability  
/// - Sequential URL trying with proper error handling
///
/// Strategy:
/// - China users: hk.gh-proxy.com → jsDelivr CDN → GitHub Raw
/// - Non-China users: GitHub Raw → jsDelivr CDN
use std::fmt;
use url::Url;

// Constants for better maintainability
const GITHUB_RAW_BASE: &str =
    "https://raw.githubusercontent.com/MaurUppi/CCstatus/master/latest.json";
const JSDELIVR_CDN_BASE: &str = "https://cdn.jsdelivr.net/gh/MaurUppi/CCstatus@master/latest.json";
const CHINA_PROXY_PREFIX: &str = "https://hk.gh-proxy.com/";

/// URL resolution error for silent failure semantics
#[derive(Debug, Clone)]
pub enum UrlResolverError {
    /// No URLs provided to try
    EmptyUrlList,
    /// All URLs failed with the last error
    AllUrlsFailed(String),
    /// Invalid URL construction
    InvalidUrl(String),
}

impl fmt::Display for UrlResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UrlResolverError::EmptyUrlList => write!(f, "No URLs provided"),
            UrlResolverError::AllUrlsFailed(err) => write!(f, "All URLs failed: {}", err),
            UrlResolverError::InvalidUrl(url) => write!(f, "Invalid URL construction: {}", url),
        }
    }
}

impl std::error::Error for UrlResolverError {}

/// Resolve manifest URLs based on geographic location with intelligent fallback strategy
///
/// Returns a prioritized list of URLs to try in order:
/// - For China users: proxy → CDN → direct (3 URLs for maximum reliability)  
/// - For non-China users: direct → CDN (2 URLs for efficiency)
///
/// # Arguments
/// * `is_china` - Whether the user is detected to be in China
///
/// # Returns
/// Vector of URLs in priority order (most reliable first)
///
/// # Examples
/// ```rust
/// let urls = resolve_manifest_url(true);  // China: 3 fallback URLs
/// let urls = resolve_manifest_url(false); // Non-China: 2 fallback URLs  
/// ```
pub fn resolve_manifest_url(is_china: bool) -> Vec<String> {
    // Align with CI: latest.json is committed to master (see .github/workflows/release.yml)
    // Prefer simple branch path over explicit refs/heads for readability.

    if is_china {
        // China chain: hk.gh-proxy → jsDelivr → GitHub Raw (maximum reliability)
        vec![
            format!("{}{}", CHINA_PROXY_PREFIX, GITHUB_RAW_BASE),
            JSDELIVR_CDN_BASE.to_string(),
            GITHUB_RAW_BASE.to_string(),
        ]
    } else {
        // Non-China: prefer GitHub Raw, fallback to jsDelivr (efficiency first)
        vec![GITHUB_RAW_BASE.to_string(), JSDELIVR_CDN_BASE.to_string()]
    }
}

/// Extract hostname from URL for caching and debugging purposes
///
/// Used for:
/// - ETag/Last-Modified caching by host
/// - Error reporting and debugging  
/// - Connection pooling optimization
///
/// # Arguments
/// * `url` - The URL to extract hostname from
///
/// # Returns
/// Some(hostname) if URL is valid, None if parsing fails
///
/// # Examples
/// ```rust
/// assert_eq!(
///     extract_host_from_url("https://cdn.jsdelivr.net/path"),
///     Some("cdn.jsdelivr.net".to_string())
/// );
/// ```
pub fn extract_host_from_url(url: &str) -> Option<String> {
    match Url::parse(url) {
        Ok(parsed) => parsed.host_str().map(|h| h.to_string()),
        Err(_) => None,
    }
}

/// Try multiple URLs in sequence until one succeeds with proper error propagation
///
/// This function implements the sequential fallback strategy by trying each URL
/// until one succeeds or all fail. Errors from failed URLs are preserved to
/// aid in debugging connectivity issues.
///
/// # Type Parameters
/// * `F` - Function type that takes a URL and returns a result
/// * `T` - Success result type
///
/// # Arguments  
/// * `urls` - Slice of URLs to try in order
/// * `fetch_fn` - Function to call for each URL
///
/// # Returns
/// * `Ok(T)` - First successful result
/// * `Err` - Error if all URLs fail or list is empty
///
/// # Examples
/// ```rust
/// let urls = vec!["https://primary.com".to_string(), "https://backup.com".to_string()];
/// let result = try_urls_in_sequence(&urls, |url| {
///     // Your fetch logic here
///     Ok("success")
/// })?;
/// ```
pub fn try_urls_in_sequence<F, T>(
    urls: &[String],
    mut fetch_fn: F,
) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnMut(&str) -> Result<T, Box<dyn std::error::Error>>,
{
    if urls.is_empty() {
        return Err(Box::new(UrlResolverError::EmptyUrlList));
    }

    let mut last_error_msg = String::new();

    for (index, url) in urls.iter().enumerate() {
        match fetch_fn(url) {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error_msg = format!("URL {}/{} ({}): {}", index + 1, urls.len(), url, e);
            }
        }
    }

    // All URLs failed - provide detailed context
    Err(Box::new(UrlResolverError::AllUrlsFailed(last_error_msg)))
}
