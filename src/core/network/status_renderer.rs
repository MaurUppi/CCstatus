// Statusline UI rendering for network monitoring
use crate::core::network::types::{NetworkMetrics, NetworkStatus};
use crate::core::network::proxy_health::config::ProxyHealthLevel;

/// Renders network status for statusline display
pub struct StatusRenderer;

impl StatusRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render status for statusline display
    /// Emoji: 🟢/🟡/🔴/⚪ map to `healthy/degraded/error/Unknown`
    /// Text: 🟢 shows P95; 🟡 shows P95+breakdown; 🔴 shows breakdown; wraps long content to next line
    /// Proxy prefix: 🟢 |/🟡 |/🔴 |/⚪ | prepended when proxy health check is available (tri-state support + Unknown)
    /// Shield: 🛡️ indicators for bot challenges (GET and/or POST)
    pub fn render_status(&self, status: &NetworkStatus, metrics: &NetworkMetrics) -> String {
        // Check for bot challenges first - they take precedence over normal rendering
        let proxy_has_bot_challenge = metrics.proxy_health_detail.as_ref()
            .and_then(|detail| detail.reason.as_ref())
            .map(|reason| reason == "cloudflare_challenge")
            .unwrap_or(false);
            
        let post_has_bot_challenge = metrics.error_type.as_ref()
            .map(|et| et == "bot_challenge")
            .unwrap_or(false);
            
        // Bot challenge rendering takes precedence
        if proxy_has_bot_challenge || post_has_bot_challenge {
            return self.render_bot_challenge(proxy_has_bot_challenge, post_has_bot_challenge, metrics);
        }
        // Determine proxy health prefix based on enhanced tri-state levels with fallback
        let proxy_prefix = match metrics.get_proxy_health_level() {
            Some(ProxyHealthLevel::Healthy) => Some("🟢 | "),  // Healthy proxy
            Some(ProxyHealthLevel::Degraded) => Some("🟡 | "), // Degraded proxy
            Some(ProxyHealthLevel::Bad) => Some("🔴 | "),     // Unhealthy proxy
            Some(ProxyHealthLevel::Unknown) => Some("⚪ | "),  // Unknown proxy (Cloudflare challenges, etc.)
            None => None, // No proxy health check (official endpoint or no health endpoint)
        };

        let core = match status {
            NetworkStatus::Healthy => {
                // healthy: show P95 (N/A if zero)
                let p95_display = if metrics.p95_latency_ms == 0 {
                    "P95:N/A".to_string()
                } else {
                    format!("P95:{}ms", metrics.p95_latency_ms)
                };
                format!("🟢 {}", p95_display)
            }
            NetworkStatus::Degraded => {
                // degraded: show P95 and breakdown (wrap if long)
                let p95_display = if metrics.p95_latency_ms == 0 {
                    "P95:N/A".to_string()
                } else {
                    format!("P95:{}ms", metrics.p95_latency_ms)
                };
                let base = format!("🟡 {}", p95_display);
                self.format_with_breakdown(base, &metrics.breakdown)
            }
            NetworkStatus::Error => {
                // error: show breakdown (wrap if long)
                self.format_with_breakdown("🔴".to_string(), &metrics.breakdown)
            }
            NetworkStatus::Unknown => "⚪ Env vars NOT Found".to_string(),
        };

        // Prepend proxy health prefix if available
        match proxy_prefix {
            Some(prefix) => format!("{}{}", prefix, core),
            None => core,
        }
    }

    /// Format status with breakdown, wrapping to next line if too long
    fn format_with_breakdown(&self, base: String, breakdown: &str) -> String {
        if breakdown.is_empty() {
            base
        } else {
            let max_line_length = 80;
            if base.len() + breakdown.len() + 1 > max_line_length {
                format!("{}\n{}", base, breakdown)
            } else {
                format!("{} {}", base, breakdown)
            }
        }
    }

    /// Render shield indicators for bot challenges
    /// GET bot challenge: 🛡️ Bot challenge
    /// POST bot challenge: 🛡️ Total: XXms  
    /// Both combined when applicable
    fn render_bot_challenge(&self, proxy_blocked: bool, post_blocked: bool, metrics: &NetworkMetrics) -> String {
        match (proxy_blocked, post_blocked) {
            (true, true) => {
                // Both GET and POST blocked
                format!("GET 🛡️ Bot challenge | POST 🛡️ Total: {}ms", metrics.latency_ms)
            }
            (true, false) => {
                // Only GET blocked - show proxy challenge with normal P95 info
                let p95_display = if metrics.p95_latency_ms == 0 {
                    "P95:N/A".to_string()
                } else {
                    format!("P95:{}ms", metrics.p95_latency_ms)
                };
                format!("🛡️ Bot challenge | {}", p95_display)
            }
            (false, true) => {
                // Only POST blocked - show total time suppressed breakdown
                format!("🛡️ Total: {}ms", metrics.latency_ms)
            }
            (false, false) => {
                // Neither blocked (shouldn't reach here)
                "🛡️ Bot challenge detected".to_string()
            }
        }
    }
}

impl Default for StatusRenderer {
    fn default() -> Self {
        Self::new()
    }
}
