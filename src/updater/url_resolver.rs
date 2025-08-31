/// URL resolver for manifest fetching with geographic optimization
/// 
/// For China users: Try hk.gh-proxy.com first, fallback to original, then give up
/// For non-China users: Use original URL directly
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
pub fn try_urls_in_sequence<F, T, E>(urls: &[String], mut fetch_fn: F) -> Result<T, E>
where
    F: FnMut(&str) -> Result<T, E>,
{
    let mut last_error = None;
    
    for url in urls {
        match fetch_fn(url) {
            Ok(result) => return Ok(result),
            Err(e) => last_error = Some(e),
        }
    }
    
    // If we get here, all URLs failed
    match last_error {
        Some(e) => Err(e),
        None => panic!("No URLs provided"), // This should never happen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_manifest_url_china() {
        let urls = resolve_manifest_url(true);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("hk.gh-proxy.com"));
        assert!(urls[1].contains("raw.githubusercontent.com"));
    }

    #[test]
    fn test_resolve_manifest_url_non_china() {
        let urls = resolve_manifest_url(false);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("raw.githubusercontent.com"));
    }

    #[test]
    fn test_extract_host_from_url() {
        let host = extract_host_from_url("https://hk.gh-proxy.com/path/to/file");
        assert_eq!(host, Some("hk.gh-proxy.com".to_string()));

        let host = extract_host_from_url("https://raw.githubusercontent.com/repo/file");
        assert_eq!(host, Some("raw.githubusercontent.com".to_string()));

        let host = extract_host_from_url("invalid-url");
        assert_eq!(host, None);
    }

    #[test]
    fn test_try_urls_in_sequence() {
        let urls = vec![
            "https://fail1.com".to_string(),
            "https://success.com".to_string(),
            "https://fail2.com".to_string(),
        ];

        let result = try_urls_in_sequence(&urls, |url| {
            if url.contains("success") {
                Ok("Success!")
            } else {
                Err("Failed")
            }
        });

        assert_eq!(result, Ok("Success!"));
    }

    #[test]
    fn test_try_urls_in_sequence_all_fail() {
        let urls = vec![
            "https://fail1.com".to_string(),
            "https://fail2.com".to_string(),
        ];

        let result = try_urls_in_sequence(&urls, |_url| {
            Err("Always fails")
        });

        assert_eq!(result, Err("Always fails"));
    }
}