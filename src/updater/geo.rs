/// Geographic detection result
#[derive(Debug, Clone)]
pub struct GeoResult {
    pub is_china: bool,
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

/// Detect if user is in China by checking IP geolocation
/// TTL: 24 hours, with CI environment mocking support
pub fn detect_china_ttl24h() -> bool {
    // Mock geo detection in CI environments to avoid real network calls
    if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
        // Default to false (non-China) in CI unless explicitly overridden
        let mock_value = std::env::var("CCSTATUS_TEST_CHINA_GEO")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        return mock_value;
    }
    
    // Try to detect China location by checking myip.ipip.net
    detect_china_online().unwrap_or_default()
}

/// Perform online China detection with fallback endpoints
fn detect_china_online() -> Result<bool, Box<dyn std::error::Error>> {
    let client: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(3)))
        .build()
        .into();

    // Primary: myip.ipip.net (plaintext with Chinese location info)
    match client
        .get("http://myip.ipip.net")
        .header(
            "User-Agent",
            &format!("CCstatus/{}", env!("CARGO_PKG_VERSION")),
        )
        .call()
    {
        Ok(mut response) if response.status().as_u16() == 200 => {
            let body = response.body_mut().read_to_string()?;
            return Ok(body.contains("中国"));
        }
        _ => {}
    }

    // Fallback: cip.cc (plaintext, includes lines like "地址 : 中国 ...")
    match client
        .get("http://cip.cc")
        .header(
            "User-Agent",
            &format!("CCstatus/{}", env!("CARGO_PKG_VERSION")),
        )
        .call()
    {
        Ok(mut response) if response.status().as_u16() == 200 => {
            let body = response.body_mut().read_to_string()?;
            return Ok(body.contains("中国"));
        }
        Ok(response) => Err(format!("HTTP {}", response.status().as_u16()).into()),
        Err(e) => Err(e.into()),
    }
}
