// Statusline UI rendering for network monitoring
use crate::core::network::types::{NetworkStatus, NetworkMetrics};

/// Renders network status for statusline display
pub struct StatusRenderer;

impl StatusRenderer {
    pub fn new() -> Self {
        Self
    }
    
    /// Render status for statusline display
    /// Emoji: 🟢/🟡/🔴/⚪ map to `healthy/degraded/error/Unknown`
    /// Text: 🟢 shows P95; 🟡 shows P95+breakdown; 🔴 shows breakdown; wraps long content to next line
    /// Proxy prefix: 🟢 | or 🔴 | prepended when proxy health check is available
    pub fn render_status(&self, status: &NetworkStatus, metrics: &NetworkMetrics) -> String {
        // Determine proxy health prefix based on proxy_healthy field
        let proxy_prefix = match metrics.proxy_healthy {
            Some(true) => Some("🟢 | "),   // Healthy proxy
            Some(false) => Some("🔴 | "),  // Unhealthy proxy
            None => None,                   // No proxy or official endpoint
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
            },
            NetworkStatus::Degraded => {
                // degraded: show P95 and breakdown (wrap if long)
                let p95_display = if metrics.p95_latency_ms == 0 {
                    "P95:N/A".to_string()
                } else {
                    format!("P95:{}ms", metrics.p95_latency_ms)
                };
                let base = format!("🟡 {}", p95_display);
                self.format_with_breakdown(base, &metrics.breakdown)
            },
            NetworkStatus::Error => {
                // error: show breakdown (wrap if long)
                self.format_with_breakdown("🔴".to_string(), &metrics.breakdown)
            },
            NetworkStatus::Unknown => {
                "⚪ Env varis NOT Found".to_string()
            }
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
}

impl Default for StatusRenderer {
    fn default() -> Self {
        Self::new()
    }
}