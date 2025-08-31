/// URL resolver for manifest fetching with geographic optimization
///
/// For China users: Try hk.gh-proxy.com first, fallback to original, then give up
/// For non-China users: Use original URL directly

use std::fmt;

/// URL resolution error for silent failure semantics
#[derive(Debug, Clone)]
pub enum UrlResolverError {
    /// No URLs provided to try
    EmptyUrlList,
    /// All URLs failed with the last error
    AllUrlsFailed(String),
}

impl fmt::Display for UrlResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UrlResolverError::EmptyUrlList => write!(f, "No URLs provided"),
            UrlResolverError::AllUrlsFailed(err) => write!(f, "All URLs failed: {}", err),
        }
    }
}

impl std::error::Error for UrlResolverError {}
pub fn resolve_manifest_url(is_china: bool) -> Vec<String> {
    let original_url = "https://raw.githubusercontent.com/MaurUppi/CCstatus/main/latest.json";

    if is_china {
        vec![
            format!("https://hk.gh-proxy.com/{}", original_url),
            original_url.to_string(),
        ]
    } else {
        vec![original_url.to_string()]
    }
}

/// Resolve URLs and return the host for caching purposes
pub fn extract_host_from_url(url: &str) -> Option<String> {
    if let Ok(parsed) = url::Url::parse(url) {
        parsed.host_str().map(|h| h.to_string())
    } else {
        None
    }
}

/// Try multiple URLs in sequence until one succeeds
pub fn try_urls_in_sequence<F, T>(urls: &[String], mut fetch_fn: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnMut(&str) -> Result<T, Box<dyn std::error::Error>>,
{
    if urls.is_empty() {
        return Err(Box::new(UrlResolverError::EmptyUrlList));
    }

    let mut last_error_msg = String::new();

    for url in urls {
        match fetch_fn(url) {
            Ok(result) => return Ok(result),
            Err(e) => last_error_msg = e.to_string(),
        }
    }

    // All URLs failed
    Err(Box::new(UrlResolverError::AllUrlsFailed(last_error_msg)))
}

