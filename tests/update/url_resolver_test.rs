use ccstatus::updater::url_resolver::{resolve_manifest_url, extract_host_from_url, try_urls_in_sequence};

#[test]
fn test_resolve_manifest_url_china() {
    let urls = resolve_manifest_url(true);
    
    assert_eq!(urls.len(), 2);
    assert!(urls[0].contains("hk.gh-proxy.com"));
    assert!(urls[0].contains("raw.githubusercontent.com"));
    assert!(urls[1].contains("raw.githubusercontent.com"));
    assert!(!urls[1].contains("hk.gh-proxy.com"));
}

#[test]
fn test_resolve_manifest_url_non_china() {
    let urls = resolve_manifest_url(false);
    
    assert_eq!(urls.len(), 1);
    assert!(urls[0].contains("raw.githubusercontent.com"));
    assert!(!urls[0].contains("hk.gh-proxy.com"));
}

#[test]
fn test_extract_host_from_url_valid() {
    let host = extract_host_from_url("https://hk.gh-proxy.com/path/to/file");
    assert_eq!(host, Some("hk.gh-proxy.com".to_string()));
    
    let host = extract_host_from_url("https://raw.githubusercontent.com/repo/file");
    assert_eq!(host, Some("raw.githubusercontent.com".to_string()));
    
    let host = extract_host_from_url("http://myip.ipip.net");
    assert_eq!(host, Some("myip.ipip.net".to_string()));
}

#[test]
fn test_extract_host_from_url_invalid() {
    let host = extract_host_from_url("invalid-url");
    assert_eq!(host, None);
    
    let host = extract_host_from_url("");
    assert_eq!(host, None);
}

#[test]
fn test_try_urls_in_sequence_first_succeeds() {
    let urls = vec![
        "https://success.com".to_string(),
        "https://backup.com".to_string(),
    ];
    
    let result = try_urls_in_sequence(&urls, |url| {
        if url.contains("success") {
            Ok("Found it!")
        } else {
            Err("Failed")
        }
    });
    
    assert_eq!(result, Ok("Found it!"));
}

#[test]
fn test_try_urls_in_sequence_fallback() {
    let urls = vec![
        "https://primary.com".to_string(),
        "https://backup.com".to_string(),
        "https://tertiary.com".to_string(),
    ];
    
    let result = try_urls_in_sequence(&urls, |url| {
        if url.contains("backup") {
            Ok("Backup worked!")
        } else {
            Err("Failed")
        }
    });
    
    assert_eq!(result, Ok("Backup worked!"));
}

#[test]
fn test_try_urls_in_sequence_all_fail() {
    let urls = vec![
        "https://fail1.com".to_string(),
        "https://fail2.com".to_string(),
    ];
    
    let result: Result<&str, &str> = try_urls_in_sequence(&urls, |_url| {
        Err("Always fails")
    });
    
    assert_eq!(result, Err("Always fails"));
}

#[test]
fn test_hk_proxy_url_format() {
    let urls = resolve_manifest_url(true);
    let hk_url = &urls[0];
    let original_url = &urls[1];
    
    // Verify the hk proxy URL is correctly formatted
    assert!(hk_url.starts_with("https://hk.gh-proxy.com/"));
    assert!(hk_url.ends_with("latest.json"));
    
    // Verify original URL is preserved
    assert_eq!(original_url, "https://raw.githubusercontent.com/MaurUppi/CCstatus/main/latest.json");
}