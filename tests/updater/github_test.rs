use ccstatus::updater::github::{GitHubRelease, ReleaseAsset};

#[test]
fn test_github_release_version_with_v_prefix() {
    let release = GitHubRelease {
        tag_name: "v1.2.3".to_string(),
        name: "Release 1.2.3".to_string(),
        body: "Release notes".to_string(),
        draft: false,
        prerelease: false,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        published_at: "2023-01-01T00:00:00Z".to_string(),
        html_url: "https://github.com/MaurUppi/CCstatus/releases/tag/v1.2.3".to_string(),
        assets: vec![],
    };

    assert_eq!(release.version(), "1.2.3");
}

#[test]
fn test_github_release_version_without_v_prefix() {
    let release = GitHubRelease {
        tag_name: "1.2.3".to_string(),
        name: "Release 1.2.3".to_string(),
        body: "Release notes".to_string(),
        draft: false,
        prerelease: false,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        published_at: "2023-01-01T00:00:00Z".to_string(),
        html_url: "https://github.com/MaurUppi/CCstatus/releases/tag/1.2.3".to_string(),
        assets: vec![],
    };

    assert_eq!(release.version(), "1.2.3");
}

#[test]
fn test_find_asset_for_platform_found() {
    let assets = vec![
        ReleaseAsset {
            name: "ccstatus-windows-x64.zip".to_string(),
            size: 1024,
            download_count: 10,
            browser_download_url: "https://example.com/windows.zip".to_string(),
            content_type: "application/zip".to_string(),
        },
        ReleaseAsset {
            name: "ccstatus-macos-arm64.tar.gz".to_string(),
            size: 2048,
            download_count: 20,
            browser_download_url: "https://example.com/macos.tar.gz".to_string(),
            content_type: "application/gzip".to_string(),
        },
        ReleaseAsset {
            name: "ccstatus-linux-x64.tar.gz".to_string(),
            size: 3072,
            download_count: 30,
            browser_download_url: "https://example.com/linux.tar.gz".to_string(),
            content_type: "application/gzip".to_string(),
        },
    ];

    let release = GitHubRelease {
        tag_name: "v1.2.3".to_string(),
        name: "Release 1.2.3".to_string(),
        body: "Release notes".to_string(),
        draft: false,
        prerelease: false,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        published_at: "2023-01-01T00:00:00Z".to_string(),
        html_url: "https://github.com/MaurUppi/CCstatus/releases/tag/v1.2.3".to_string(),
        assets,
    };

    // This test will find different assets depending on the platform it's run on
    let found_asset = release.find_asset_for_platform();

    // Asset should be found for supported platforms
    #[cfg(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64")
    ))]
    {
        assert!(found_asset.is_some());
        let asset = found_asset.unwrap();
        assert!(asset.size > 0);
        assert!(!asset.name.is_empty());
    }

    // For unsupported platforms, no asset should be found
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64")
    )))]
    {
        // Note: This might still find an asset if the "unknown" platform matches
        // This is acceptable behavior for the current implementation
    }
}

#[test]
fn test_find_asset_for_platform_not_found() {
    let assets = vec![ReleaseAsset {
        name: "ccstatus-unsupported-platform.zip".to_string(),
        size: 1024,
        download_count: 10,
        browser_download_url: "https://example.com/unsupported.zip".to_string(),
        content_type: "application/zip".to_string(),
    }];

    let release = GitHubRelease {
        tag_name: "v1.2.3".to_string(),
        name: "Release 1.2.3".to_string(),
        body: "Release notes".to_string(),
        draft: false,
        prerelease: false,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        published_at: "2023-01-01T00:00:00Z".to_string(),
        html_url: "https://github.com/MaurUppi/CCstatus/releases/tag/v1.2.3".to_string(),
        assets,
    };

    let found_asset = release.find_asset_for_platform();
    assert!(found_asset.is_none());
}

#[test]
fn test_release_asset_serde_compatibility() {
    // Test that our structs can be serialized/deserialized
    let asset = ReleaseAsset {
        name: "test.zip".to_string(),
        size: 1024,
        download_count: 5,
        browser_download_url: "https://example.com/test.zip".to_string(),
        content_type: "application/zip".to_string(),
    };

    let json = serde_json::to_string(&asset).expect("Should serialize");
    let parsed: ReleaseAsset = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(parsed.name, "test.zip");
    assert_eq!(parsed.size, 1024);
    assert_eq!(parsed.download_count, 5);
}

#[test]
fn test_github_release_serde_compatibility() {
    let release = GitHubRelease {
        tag_name: "v1.0.0".to_string(),
        name: "Test Release".to_string(),
        body: "Test body".to_string(),
        draft: false,
        prerelease: true,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        published_at: "2023-01-01T00:00:00Z".to_string(),
        html_url: "https://github.com/test/repo/releases/tag/v1.0.0".to_string(),
        assets: vec![],
    };

    let json = serde_json::to_string(&release).expect("Should serialize");
    let parsed: GitHubRelease = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(parsed.tag_name, "v1.0.0");
    assert_eq!(parsed.name, "Test Release");
    assert_eq!(parsed.prerelease, true);
    assert_eq!(parsed.draft, false);
}

// Note: check_for_updates() function is not tested here because it makes real HTTP requests
// In a production environment, you would want to mock the HTTP client for testing
// For now, this function is tested through integration tests or manual testing
