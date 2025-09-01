use ccstatus::updater::url_resolver::{
    extract_host_from_url, resolve_manifest_url, try_urls_in_sequence, UrlResolverError,
};

#[test]
fn test_resolve_manifest_url_china() {
    let urls = resolve_manifest_url(true);

    // China should have 3 fallback URLs for maximum reliability
    assert_eq!(urls.len(), 3);

    // First URL should be hk.gh-proxy.com
    assert!(urls[0].contains("hk.gh-proxy.com"));
    assert!(urls[0].contains("raw.githubusercontent.com"));

    // Second URL should be jsDelivr CDN
    assert!(urls[1].contains("jsdelivr.net"));
    assert!(urls[1].contains("@master"));

    // Third URL should be direct GitHub Raw
    assert!(urls[2].contains("raw.githubusercontent.com"));
    assert!(urls[2].contains("/master/"));
    assert!(!urls[2].contains("hk.gh-proxy.com"));
    assert!(!urls[2].contains("jsdelivr.net"));
}

#[test]
fn test_resolve_manifest_url_non_china() {
    let urls = resolve_manifest_url(false);

    // Non-China should have 2 fallback URLs for efficiency
    assert_eq!(urls.len(), 2);

    // First URL should be direct GitHub Raw
    assert!(urls[0].contains("raw.githubusercontent.com"));
    assert!(urls[0].contains("/master/"));
    assert!(!urls[0].contains("hk.gh-proxy.com"));
    assert!(!urls[0].contains("jsdelivr.net"));

    // Second URL should be jsDelivr CDN fallback
    assert!(urls[1].contains("jsdelivr.net"));
    assert!(urls[1].contains("@master"));
}

#[test]
fn test_master_branch_alignment() {
    let china_urls = resolve_manifest_url(true);
    let non_china_urls = resolve_manifest_url(false);

    // All URLs should reference master branch, not main
    for url in china_urls.iter().chain(non_china_urls.iter()) {
        if url.contains("raw.githubusercontent.com") {
            assert!(
                url.contains("/master/"),
                "URL should reference master branch: {}",
                url
            );
            assert!(
                !url.contains("/main/"),
                "URL should not reference main branch: {}",
                url
            );
        }
        if url.contains("jsdelivr.net") {
            assert!(
                url.contains("@master"),
                "CDN URL should reference @master: {}",
                url
            );
        }
    }
}

#[test]
fn test_extract_host_from_url_valid() {
    let host = extract_host_from_url("https://hk.gh-proxy.com/path/to/file");
    assert_eq!(host, Some("hk.gh-proxy.com".to_string()));

    let host = extract_host_from_url("https://raw.githubusercontent.com/repo/file");
    assert_eq!(host, Some("raw.githubusercontent.com".to_string()));

    let host = extract_host_from_url("https://cdn.jsdelivr.net/gh/repo@branch/file");
    assert_eq!(host, Some("cdn.jsdelivr.net".to_string()));

    let host = extract_host_from_url("http://myip.ipip.net");
    assert_eq!(host, Some("myip.ipip.net".to_string()));
}

#[test]
fn test_extract_host_from_url_invalid() {
    let host = extract_host_from_url("invalid-url");
    assert_eq!(host, None);

    let host = extract_host_from_url("");
    assert_eq!(host, None);

    let host = extract_host_from_url("not-a-url");
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
            Err("Failed".into())
        }
    });

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Found it!");
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
            Err("Primary failed".into())
        }
    });

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Backup worked!");
}

#[test]
fn test_try_urls_in_sequence_all_fail() {
    let urls = vec![
        "https://fail1.com".to_string(),
        "https://fail2.com".to_string(),
    ];

    let result: Result<&str, Box<dyn std::error::Error>> =
        try_urls_in_sequence(&urls, |_url| Err("Always fails".into()));

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("All URLs failed"));
    assert!(error_msg.contains("URL 2/2"));
    assert!(error_msg.contains("https://fail2.com"));
}

#[test]
fn test_try_urls_in_sequence_empty_list() {
    let urls: Vec<String> = vec![];

    let result: Result<&str, Box<dyn std::error::Error>> =
        try_urls_in_sequence(&urls, |_url| Ok("Should not be called"));

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No URLs provided"));
}

#[test]
fn test_hk_proxy_url_format() {
    let urls = resolve_manifest_url(true);
    let hk_url = &urls[0];
    let jsdelivr_url = &urls[1];
    let github_url = &urls[2];

    // Verify the hk proxy URL is correctly formatted
    assert!(hk_url.starts_with("https://hk.gh-proxy.com/"));
    assert!(hk_url.ends_with("latest.json"));
    assert!(hk_url.contains("/master/"));

    // Verify jsDelivr CDN URL
    assert!(jsdelivr_url.starts_with("https://cdn.jsdelivr.net/"));
    assert!(jsdelivr_url.contains("@master"));
    assert!(jsdelivr_url.ends_with("latest.json"));

    // Verify GitHub Raw URL uses master branch
    assert_eq!(
        github_url,
        "https://raw.githubusercontent.com/MaurUppi/CCstatus/master/latest.json"
    );
}

#[test]
fn test_jsdelivr_cdn_fallback() {
    let china_urls = resolve_manifest_url(true);
    let non_china_urls = resolve_manifest_url(false);

    // Both China and non-China should include jsDelivr as fallback
    assert!(china_urls.iter().any(|url| url.contains("jsdelivr.net")));
    assert!(non_china_urls
        .iter()
        .any(|url| url.contains("jsdelivr.net")));

    // jsDelivr URLs should use the correct format
    for urls in [&china_urls, &non_china_urls] {
        for url in urls {
            if url.contains("jsdelivr.net") {
                assert!(url.contains("cdn.jsdelivr.net/gh/"));
                assert!(url.contains("@master"));
                assert!(url.ends_with("latest.json"));
            }
        }
    }
}

#[test]
fn test_url_priority_ordering() {
    let china_urls = resolve_manifest_url(true);
    let non_china_urls = resolve_manifest_url(false);

    // China priority: proxy first, then CDN, then direct
    assert!(china_urls[0].contains("hk.gh-proxy.com"));
    assert!(china_urls[1].contains("jsdelivr.net"));
    assert!(china_urls[2].contains("raw.githubusercontent.com"));
    assert!(!china_urls[2].contains("hk.gh-proxy.com"));
    assert!(!china_urls[2].contains("jsdelivr.net"));

    // Non-China priority: direct first, then CDN
    assert!(non_china_urls[0].contains("raw.githubusercontent.com"));
    assert!(!non_china_urls[0].contains("jsdelivr.net"));
    assert!(non_china_urls[1].contains("jsdelivr.net"));
}

#[test]
fn test_error_context_in_sequential_trying() {
    let urls = vec![
        "https://first.com".to_string(),
        "https://second.com".to_string(),
        "https://third.com".to_string(),
    ];

    let result: Result<&str, Box<dyn std::error::Error>> = try_urls_in_sequence(&urls, |url| {
        Err(format!("Connection failed for {}", url).into())
    });

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();

    // Should contain context about the last failed URL
    assert!(error_msg.contains("All URLs failed"));
    assert!(error_msg.contains("URL 3/3"));
    assert!(error_msg.contains("https://third.com"));
    assert!(error_msg.contains("Connection failed"));
}
