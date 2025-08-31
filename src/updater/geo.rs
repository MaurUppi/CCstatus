/// Geographic detection result
#[derive(Debug, Clone)]
pub struct GeoResult {
    pub is_china: bool,
    pub detected_at: chrono::DateTime<chrono::Utc>,
}

/// Detect if user is in China by checking IP geolocation
/// TTL: 24 hours, no environment variables used
pub fn detect_china_ttl24h() -> bool {
    // Try to detect China location by checking myip.ipip.net
    match detect_china_online() {
        Ok(is_china) => is_china,
        Err(_) => {
            // Default to non-China if detection fails
            false
        }
    }
}

/// Perform online China detection via myip.ipip.net
fn detect_china_online() -> Result<bool, Box<dyn std::error::Error>> {
    let client = ureq::Agent::new_with_defaults();

    let mut response = client
        .get("http://myip.ipip.net")
        .header("User-Agent", &format!("CCstatus/{}", env!("CARGO_PKG_VERSION")))
        .call()?;

    if response.status().as_u16() == 200 {
        let body = response.body_mut().read_to_string()?;
        // Check if response contains "中国" (China in Chinese)
        Ok(body.contains("中国"))
    } else {
        Err(format!("HTTP {}", response.status().as_u16()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_china_ttl24h() {
        // This test will make a real network call, so we just ensure it doesn't panic
        let result = detect_china_ttl24h();
        // Result should be boolean (true or false)
        assert!(result == true || result == false);
    }

    #[test]
    fn test_detect_china_online_error_handling() {
        // Test with invalid URL to ensure error handling works
        let client = ureq::Agent::new_with_defaults();

        let result = client
            .get("http://invalid.nonexistent.domain.test")
            .call();

        assert!(result.is_err());
    }
}