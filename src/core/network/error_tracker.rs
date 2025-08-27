// Error tracking and classification for network monitoring

use crate::core::network::types::{JsonlError, NetworkStatus};
use std::collections::VecDeque;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Error tracker for managing error history and monitoring decisions
///
/// # Memory-Only Operation
///
/// **IMPORTANT**: This struct operates as an in-memory queue only and does not persist data.
/// - Error history is maintained in a VecDeque with configurable max size (default: 50 events)
/// - Data is lost on process restart - no disk persistence or state recovery
/// - For persistent error storage, use external logging or database systems
///
/// # Thread Safety
///
/// This struct is **not thread-safe**. Use appropriate synchronization if accessed from multiple threads.
///
/// # Capacity Management
///
/// - Max history size prevents unbounded memory growth
/// - Oldest errors are automatically evicted when capacity is reached
/// - Time-based cleanup available via `cleanup_old_errors()` for retention policies
pub struct ErrorTracker {
    error_history: VecDeque<ErrorEvent>,
    max_history: usize,
}

/// Individual error event
#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub timestamp: u64, // Unix timestamp in milliseconds
    pub error_type: String,
    pub http_status: u16,
    pub message: String,
}

impl ErrorTracker {
    pub fn new() -> Self {
        Self {
            error_history: VecDeque::new(),
            max_history: 50, // Keep last 50 errors
        }
    }

    /// Record a new error event
    pub fn record_error(&mut self, http_status: u16, message: String) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64;

        // Use official classification based on HTTP status for consistency with stats filtering
        let classified_error_type = if http_status == 0 {
            // For connection-level errors, use message-based classification
            Self::classify_connection_error(&message)
        } else {
            self.classify_http_status(http_status)
        };

        let event = ErrorEvent {
            timestamp,
            error_type: classified_error_type,
            http_status,
            message,
        };

        self.error_history.push_back(event);

        // Maintain max history size
        while self.error_history.len() > self.max_history {
            self.error_history.pop_front();
        }
    }

    /// Record error from JSONL transcript using original timestamp
    pub fn record_jsonl_error(&mut self, error: &JsonlError) {
        let error_type = self.classify_error_code(error.code);
        let timestamp = Self::parse_jsonl_timestamp(&error.timestamp);

        let event = ErrorEvent {
            timestamp,
            error_type,
            http_status: error.code,
            message: error.message.clone(),
        };

        self.error_history.push_back(event);

        // Maintain max history size
        while self.error_history.len() > self.max_history {
            self.error_history.pop_front();
        }
    }

    /// Check if there are recent errors (DIAGNOSTICS ONLY - DO NOT USE FOR RED GATING)
    ///
    /// **WARNING**: This method uses a 60-second lookback window which conflicts with
    /// the RED monitoring specification. For RED gating decisions, use JsonlMonitor::scan_tail()
    /// exclusively, which implements the correct 10s/1s frequency window per specification.
    ///
    /// This method serves as an auxiliary diagnostic signal only. The primary RED window
    /// trigger should be immediate error detection from JsonlMonitor to avoid architectural conflicts.
    pub fn has_recent_errors(&self, current_time_ms: u64) -> bool {
        // Look for errors in the last 60 seconds that happened before or at current_time
        let cutoff_time = current_time_ms.saturating_sub(60_000);

        self.error_history
            .iter()
            .any(|event| event.timestamp >= cutoff_time && event.timestamp <= current_time_ms)
    }

    /// Get the most recent error
    pub fn get_latest_error(&self) -> Option<&ErrorEvent> {
        self.error_history.back()
    }

    /// Classify HTTP status codes using official Anthropic API error types
    pub fn classify_http_status(&self, status_code: u16) -> String {
        match status_code {
            200..=299 => "success".to_string(),
            400 => "invalid_request_error".to_string(),
            401 => "authentication_error".to_string(),
            403 => "permission_error".to_string(),
            404 => "not_found_error".to_string(),
            413 => "request_too_large".to_string(),
            429 => "rate_limit_error".to_string(),
            500 => "api_error".to_string(),
            502 => "server_error".to_string(),
            504 => "socket_hang_up".to_string(),
            529 => "overloaded_error".to_string(),
            400..=499 => "client_error".to_string(), // Fallback for other 4xx
            500..=599 => "server_error".to_string(), // Fallback for other 5xx
            0 => "connection_error".to_string(),     // Network failures
            _ => "unknown_error".to_string(),
        }
    }

    /// Classify error codes (for JSONL errors)
    fn classify_error_code(&self, code: u16) -> String {
        self.classify_http_status(code)
    }

    /// Classify connection/network level errors from error messages
    pub fn classify_connection_error(error_message: &str) -> String {
        let message_lower = error_message.to_lowercase();
        if message_lower.contains("connection error")
            || message_lower.contains("fetch failed")
            || message_lower.contains("network error")
            || message_lower.contains("timeout")
            || message_lower.contains("request timed out")
            || message_lower.contains("connection refused")
            || message_lower.contains("certificate verification")
            || message_lower.contains("unknown certificate")
            || message_lower.contains("tls")
            || message_lower.contains("ssl")
        {
            "connection_error".to_string()
        } else if message_lower.contains("usage policy")
            || message_lower.contains("violate our usage policy")
        {
            "invalid_request_error".to_string()
        } else {
            "unknown_error".to_string()
        }
    }

    /// Determine network status based on HTTP response
    ///
    /// **DEPRECATED**: This method duplicates logic that should be centralized in HttpMonitor/StatusRenderer.
    /// Use shared utilities from the canonical HttpMonitor implementation instead.
    #[deprecated(
        note = "Use HttpMonitor utilities for status determination to maintain single source of truth"
    )]
    pub fn determine_status(
        &self,
        status_code: u16,
        latency_ms: u32,
        p80_threshold: u32,
        p95_threshold: u32,
    ) -> NetworkStatus {
        match status_code {
            200..=299 => {
                // Success - determine by latency thresholds
                if latency_ms <= p80_threshold {
                    NetworkStatus::Healthy
                } else if latency_ms <= p95_threshold {
                    NetworkStatus::Degraded
                } else {
                    NetworkStatus::Error // Above P95 is considered error
                }
            }
            429 => NetworkStatus::Degraded,    // Rate limiting
            401 | 403 => NetworkStatus::Error, // Authentication errors
            400..=499 => NetworkStatus::Error, // Client errors
            500..=599 => NetworkStatus::Error, // Server errors
            0 => NetworkStatus::Error,         // Timeout/connection failure
            _ => NetworkStatus::Error,         // Unknown errors
        }
    }

    /// Calculate percentiles from rolling totals using nearest-rank method
    ///
    /// Uses the nearest-rank method: idx = max(0, ceil(p * n) - 1)
    /// This avoids off-by-one issues and provides better estimates for small samples.
    ///
    /// **DEPRECATED**: This method duplicates percentile calculation logic that should be
    /// centralized in HttpMonitor. Use shared utilities to avoid architectural drift.
    #[deprecated(
        note = "Use HttpMonitor utilities for percentile calculations to maintain single source of truth"
    )]
    pub fn calculate_percentiles(&self, rolling_totals: &[u32]) -> (u32, u32) {
        if rolling_totals.is_empty() {
            return (0, 0);
        }

        let mut sorted = rolling_totals.to_vec();
        sorted.sort_unstable();
        let n = sorted.len();

        // Use nearest-rank method: idx = max(0, ceil(p * n) - 1)
        let p80_index = (((n as f64) * 0.80).ceil() as usize).saturating_sub(1);
        let p95_index = (((n as f64) * 0.95).ceil() as usize).saturating_sub(1);

        let p80 = sorted.get(p80_index).copied().unwrap_or(0);
        let p95 = sorted.get(p95_index).copied().unwrap_or(0);

        (p80, p95)
    }

    /// Get error statistics for the last N minutes
    pub fn get_error_stats(&self, minutes: u32) -> ErrorStats {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64
            - (minutes as u64 * 60_000);

        let recent_errors: Vec<&ErrorEvent> = self
            .error_history
            .iter()
            .filter(|event| event.timestamp >= cutoff_time)
            .collect();

        let total_errors = recent_errors.len();
        let authentication_errors = recent_errors
            .iter()
            .filter(|e| {
                matches!(
                    e.error_type.as_str(),
                    "authentication_error" | "permission_error"
                )
            })
            .count();
        let rate_limit_errors = recent_errors
            .iter()
            .filter(|e| e.error_type == "rate_limit_error")
            .count();
        let server_errors = recent_errors
            .iter()
            .filter(|e| {
                matches!(
                    e.error_type.as_str(),
                    "api_error" | "overloaded_error" | "socket_hang_up" | "server_error"
                )
            })
            .count();

        ErrorStats {
            total_errors,
            authentication_errors,
            rate_limit_errors,
            server_errors,
            time_window_minutes: minutes,
        }
    }

    /// Clear old errors beyond retention period
    pub fn cleanup_old_errors(&mut self, retention_hours: u32) {
        let cutoff_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64
            - (retention_hours as u64 * 60 * 60 * 1000);

        while let Some(front) = self.error_history.front() {
            if front.timestamp < cutoff_time {
                self.error_history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Parse JSONL timestamp string to Unix milliseconds
    ///
    /// Expected format: "2025-08-20T00:15:04.502780+08:00" (ISO 8601 with timezone)
    /// Returns milliseconds since Unix epoch, fallback to current time on parse failure
    fn parse_jsonl_timestamp(timestamp_str: &str) -> u64 {
        use std::str::FromStr;

        // Try to parse ISO 8601 timestamp
        if let Ok(parsed) = chrono::DateTime::<chrono::FixedOffset>::from_str(timestamp_str) {
            return parsed.timestamp_millis() as u64;
        }

        // Try without timezone (assume UTC)
        if let Ok(parsed) = chrono::DateTime::<chrono::Utc>::from_str(&format!(
            "{}Z",
            timestamp_str.trim_end_matches('Z')
        )) {
            return parsed.timestamp_millis() as u64;
        }

        // Fallback to current time if parsing fails
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as u64
    }
}

/// Error statistics for a time window
#[derive(Debug, Clone)]
pub struct ErrorStats {
    pub total_errors: usize,
    pub authentication_errors: usize,
    pub rate_limit_errors: usize,
    pub server_errors: usize,
    pub time_window_minutes: u32,
}

impl Default for ErrorTracker {
    fn default() -> Self {
        Self::new()
    }
}
