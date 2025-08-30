// JSONL transcript monitoring for RED gate control (stateless)
use crate::core::network::debug_logger::{get_debug_logger, EnhancedDebugLogger};
use crate::core::network::types::{JsonlError, NetworkError};
use serde_json::Value;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Monitor for scanning JSONL transcript files and detecting API errors
///
/// **Primary Purpose:** RED gate control for monitoring system (stateless)
/// - Detects `isApiErrorMessage: true` entries in transcript tail (primary path)
/// - **Enhanced Detection:** Also detects API error text patterns when flag is false/missing (fallback path)
/// - Returns boolean detection status + most recent error details
/// - Optimized for large files via configurable tail reading
/// - Memory-only operation with no persistence (complies with module boundaries)
///
/// **Detection Methods:**
/// - Primary: `isApiErrorMessage: true` flag-based detection (100% reliable)
/// - Fallback: Case-insensitive pattern matching for "API error" text (when flag missing/false)
/// - Supports various formats: "API Error: 429", "api error: 500", "API error occurred"
///
/// **Debug Mode:** Set `CCSTATUS_DEBUG=true` (strict boolean) for enhanced logging
/// **Performance:** Configurable via `CCSTATUS_JSONL_TAIL_KB` environment variable (default: 64KB, max: 10MB)
/// **Security:** Input validation, structured logging, bounded memory usage
pub struct JsonlMonitor {
    logger: Arc<EnhancedDebugLogger>, // Always present for operational JSONL logging
}

impl JsonlMonitor {
    /// Create a new JsonlMonitor with optional debug logging
    ///
    /// **Security:** Never fails - graceful degradation without HOME directory
    /// **Debug Mode:** Uses strict boolean parsing (true/false case-insensitive only)
    pub fn new() -> Self {
        Self::with_debug_logger(None)
    }

    /// Create JsonlMonitor with custom debug logger (for testing)
    pub fn with_debug_logger(custom_logger: Option<Arc<EnhancedDebugLogger>>) -> Self {
        let logger = custom_logger.unwrap_or_else(|| {
            // Always create logger - JSONL logging is always-on, debug logging is internally gated
            Arc::new(get_debug_logger())
        });

        Self { logger }
    }


    /// Scan transcript tail for API error detection - optimized for RED gate control
    ///
    /// **Return Semantics for RED Gate Control:**
    /// - Returns `(error_detected: bool, last_error_event: Option<JsonlError>)`
    /// - `error_detected`: true if ANY API errors found in tail content → triggers RED state
    /// - `last_error_event`: Most recent error details for display (timestamp from transcript)
    ///
    /// **Usage Pattern:**
    /// ```rust,no_run
    /// # use ccstatus::core::network::jsonl_monitor::JsonlMonitor;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let monitor = JsonlMonitor::new();
    /// let (error_detected, last_error_event) = monitor.scan_tail("path/to/transcript.jsonl").await?;
    /// if error_detected {
    ///     // Trigger RED gate state
    ///     if let Some(error) = last_error_event {
    ///         // Display error details: error.message, error.code, error.timestamp
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// **Performance:** Only reads tail N KB (configurable via CCSTATUS_JSONL_TAIL_KB, default 64KB)
    /// **Scope:** RED gate control only - does NOT participate in overall status determination
    pub async fn scan_tail<P: AsRef<Path>>(
        &self,
        transcript_path: P,
    ) -> Result<(bool, Option<JsonlError>), NetworkError> {
        let path = transcript_path.as_ref();

        // Check if file exists
        if !path.exists() {
            return Ok((false, None));
        }

        // Read only the tail content for efficiency with large files
        let content = self.read_tail_content(path).await?;

        // Parse and detect errors from tail content only (stateless)
        self.parse_and_detect_errors(&content)
    }

    /// Read only the tail N KB from the file to avoid memory issues with large files
    /// Configurable via CCSTATUS_JSONL_TAIL_KB environment variable (default: 64KB)
    async fn read_tail_content<P: AsRef<Path>>(&self, path: P) -> Result<String, NetworkError> {
        // Get configurable tail size with security bounds (default 64KB, max 10MB)
        let tail_kb = std::env::var("CCSTATUS_JSONL_TAIL_KB")
            .unwrap_or_else(|_| "64".to_string())
            .parse::<u64>()
            .unwrap_or(64)
            .clamp(1, 10240); // Phase 2: Bound between 1KB and 10MB for security
        let tail_bytes = tail_kb * 1024;

        // Open file and get metadata
        let mut file = tokio::fs::File::open(path).await.map_err(|e| {
            NetworkError::ConfigReadError(format!("Failed to open transcript: {}", e))
        })?;

        let file_len = file
            .metadata()
            .await
            .map_err(|e| {
                NetworkError::ConfigReadError(format!("Failed to get file metadata: {}", e))
            })?
            .len();

        // If file is smaller than tail size, read entire file
        if file_len <= tail_bytes {
            let mut content = String::new();
            file.read_to_string(&mut content).await.map_err(|e| {
                NetworkError::ConfigReadError(format!("Failed to read small file: {}", e))
            })?;
            return Ok(content);
        }

        // Seek to tail position
        let seek_pos = file_len - tail_bytes;
        file.seek(SeekFrom::Start(seek_pos))
            .await
            .map_err(|e| NetworkError::ConfigReadError(format!("Failed to seek to tail: {}", e)))?;

        // Read from seek position to find first complete line boundary
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .await
            .map_err(|e| NetworkError::ConfigReadError(format!("Failed to read tail: {}", e)))?;

        // Convert to string and find first newline to avoid partial lines
        let content = String::from_utf8_lossy(&buffer);
        if let Some(first_newline) = content.find('\n') {
            // Start from after the first newline to ensure complete lines
            Ok(content[first_newline + 1..].to_string())
        } else {
            // If no newline found, use entire tail content
            Ok(content.to_string())
        }
    }

    /// Normalize error timestamp to a trustworthy RFC3339 value
    /// - Prefer the provided RFC3339 timestamp when valid and not a known placeholder
    /// - Fallback to local time when invalid or placeholder
    fn normalize_error_timestamp(&self, raw: &str) -> String {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
            let normalized = dt.to_rfc3339();
            // Filter known placeholder time window used in tests: 2024-01-01T12:..:00Z
            if !normalized.starts_with("2024-01-01T12:") {
                return normalized;
            }
        }
        crate::core::network::types::get_local_timestamp()
    }

    /// Parse transcript content and detect API errors for RED gate control (stateless)
    /// Returns (error_detected: bool, last_error_event: Option<JsonlError>)
    fn parse_and_detect_errors(
        &self,
        content: &str,
    ) -> Result<(bool, Option<JsonlError>), NetworkError> {
        let mut last_error: Option<JsonlError> = None;
        let mut error_detected = false;
        let mut error_count = 0u32;

        // Process each line to find errors
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(Some(error_entry)) = self.parse_jsonl_line(line) {
                error_detected = true;
                error_count += 1;

                // Operational logging to JSONL file (always-on)
                let extracted_message = self.extract_message_from_details(&error_entry.details);

                // Normalize error timestamp to avoid placeholder/invalid values
                let normalized_error_ts =
                    self.normalize_error_timestamp(&error_entry.timestamp);

                // Create JSONL entry according to proposal schema
                let jsonl_entry = serde_json::json!({
                    "timestamp": chrono::Local::now().to_rfc3339(),
                    "type": "jsonl_error",
                    "code": error_entry.http_code,
                    "message": extracted_message,
                    "error_timestamp": normalized_error_ts,
                    "session_id": self.logger.get_session_id()
                });

                // Write to always-on JSONL operational log
                let _ = self.logger.jsonl_sync(jsonl_entry);

                // Keep track of most recent error for return value (always needed for RED gate)
                last_error = Some(JsonlError {
                    timestamp: error_entry.timestamp.clone(),
                    code: error_entry.http_code,
                    message: self.extract_message_from_details(&error_entry.details),
                });
            }
        }

        // Summary logging for debug mode + operational JSONL logging
        // Debug log entry (CCSTATUS_DEBUG gated - handled internally by logger)
        if error_count > 0 {
            self.logger.debug_sync(
                "JsonlMonitor",
                "tail_scan_complete",
                &format!("Scanned tail content: {} API errors found", error_count),
            );
        } else {
            self.logger.debug_sync(
                "JsonlMonitor",
                "tail_scan_complete",
                "Scanned tail content: no API errors found",
            );
        }

        // Note: Do not write tail_scan_complete to JSONL; keep only debug logs above

        Ok((error_detected, last_error))
    }

    /// Parse a single JSONL line for error information with robustness improvements
    fn parse_jsonl_line(&self, line: &str) -> Result<Option<TranscriptErrorEntry>, NetworkError> {
        const MAX_LINE_LENGTH: usize = 1024 * 1024; // Phase 2: 1MB per line limit (matches read_tail_content)

        // Skip oversized lines to prevent memory pressure
        if line.len() > MAX_LINE_LENGTH {
            // Phase 2: Use debug logger for oversized line warnings
            self.logger.debug_sync(
                "JsonlMonitor",
                "oversized_line_skipped",
                &format!("Skipped oversized line: {} bytes", line.len()),
            );
            return Ok(None);
        }

        // Skip malformed JSON lines instead of failing entire operation
        let json: Value = match serde_json::from_str(line) {
            Ok(json) => json,
            Err(e) => {
                // Phase 2: Use debug logger for malformed JSON warnings
                let error_msg = e.to_string();
                let truncated_msg = if error_msg.len() > 100 {
                    // UTF-8 safe truncation: use char boundaries instead of byte boundaries
                    let preview: String = error_msg.chars().take(100).collect();
                    format!("{}...", preview)
                } else {
                    error_msg
                };
                self.logger.debug_sync(
                    "JsonlMonitor",
                    "malformed_json_skipped",
                    &format!("Skipped malformed JSON: {}", truncated_msg),
                );
                return Ok(None);
            }
        };

        // Check for isApiErrorMessage flag (primary detection path)
        if let Some(is_error) = json.get("isApiErrorMessage").and_then(|v| v.as_bool()) {
            if is_error {
                return Ok(Some(self.extract_transcript_error(&json)?));
            }
        }

        // Enhancement: Secondary detection path - scan message content for API error text
        // when isApiErrorMessage is false or missing
        if let Some(content_array) = json
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            for item in content_array {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    if self.parse_error_text(text).is_some() {
                        // Debug logging for fallback detection
                        self.logger.debug_sync(
                            "JsonlMonitor",
                            "fallback_error_detected",
                            &format!("API error detected via fallback path: {}", {
                                // UTF-8 safe truncation: use char boundaries instead of byte boundaries
                                if text.len() > 50 {
                                    text.chars().take(50).collect::<String>()
                                } else {
                                    text.to_string()
                                }
                            }),
                        );
                        return Ok(Some(self.extract_transcript_error(&json)?));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Extract error details from transcript JSON
    fn extract_transcript_error(&self, json: &Value) -> Result<TranscriptErrorEntry, NetworkError> {
        let parent_uuid = json
            .get("parentUuid")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let timestamp = json
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let session_id = json
            .get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let cwd = json
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Extract message content and HTTP code
        let mut http_code = 0u16;
        let mut details = "[]".to_string();

        if let Some(content_array) = json
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        {
            details = serde_json::to_string(content_array).unwrap_or_else(|_| "[]".to_string());

            // Phase 2: Extract HTTP code from ALL content items, not just first
            for content_item in content_array {
                if let Some(text) = content_item.get("text").and_then(|t| t.as_str()) {
                    if let Some((code, _)) = self.parse_error_text(text) {
                        http_code = code;
                        break; // Use first successfully parsed code
                    }
                }
            }
        }

        Ok(TranscriptErrorEntry {
            parent_uuid,
            timestamp,
            session_id,
            project_path: cwd,
            http_code,
            details,
        })
    }

    /// Extract error message from JSON text if present
    fn extract_json_error_message(&self, text: &str) -> Option<String> {
        if let Some(json_start) = text.find('{') {
            let json_part = &text[json_start..];
            if let Ok(error_json) = serde_json::from_str::<Value>(json_part) {
                if let Some(error_msg) = error_json
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                {
                    return Some(error_msg.to_string());
                }
            }
        }
        None
    }

    /// Parse error text to extract code and message
    /// **Enhancement:** Case-insensitive API error detection with fallback support
    /// **Phase 2 Enhancement:** Whitespace tolerant matching and colon-optional code extraction
    fn parse_error_text(&self, text: &str) -> Option<(u16, String)> {
        // Phase 2 Enhancement: Better Unicode-aware case handling
        let lower = text.to_lowercase(); // Use to_lowercase() instead of to_ascii_lowercase()

        // Phase 2 Enhancement: Whitespace tolerant API error detection
        // Matches: "api error", "api   error", "api\terror", "api\u{00a0}error", etc.
        let is_api_error = if lower.starts_with("api") && lower.len() > 3 {
            let after_api = &lower[3..];
            // Enhanced whitespace handling: support all Unicode whitespace including NBSP
            let trimmed = after_api.trim_matches(|c: char| c.is_whitespace() || c == '\u{00a0}');
            trimmed.starts_with("error")
        } else {
            false
        };

        if is_api_error {
            // Phase 2 Enhancement: Try colon-based extraction first, then colon-optional
            if let Some(colon_idx) = lower.find(':') {
                let after_colon = &lower[colon_idx + 1..];
                if let Some(code) = after_colon
                    .split_whitespace()
                    .find_map(|tok| tok.parse::<u16>().ok())
                {
                    // Try to parse JSON for detailed error message (preserve existing logic)
                    if let Some(error_msg) = self.extract_json_error_message(text) {
                        return Some((code, error_msg));
                    }

                    // Fallback to friendly messages by known HTTP codes
                    let message = match code {
                        429 => "Rate Limited",
                        500 => "Internal Server Error",
                        502 => "Bad Gateway",
                        503 => "Service Unavailable",
                        504 => "Gateway Timeout",
                        529 => "Overloaded",
                        _ => "API Error",
                    }
                    .to_string();

                    return Some((code, message));
                }
            } else {
                // Phase 2 Enhancement: Colon-optional code extraction
                // When no colon, scan initial tokens for 3-digit HTTP codes
                if let Some(code) = lower
                    .split_whitespace()
                    .skip(2) // Skip "api" and "error"
                    .find_map(|tok| {
                        let parsed = tok.parse::<u16>().ok()?;
                        // Validate it's a reasonable HTTP status code (100-599)
                        if parsed >= 100 && parsed < 600 {
                            Some(parsed)
                        } else {
                            None
                        }
                    })
                {
                    // Same JSON parsing and fallback logic as above
                    if let Some(error_msg) = self.extract_json_error_message(text) {
                        return Some((code, error_msg));
                    }

                    let message = match code {
                        429 => "Rate Limited",
                        500 => "Internal Server Error",
                        502 => "Bad Gateway",
                        503 => "Service Unavailable",
                        504 => "Gateway Timeout",
                        529 => "Overloaded",
                        _ => "API Error",
                    }
                    .to_string();

                    return Some((code, message));
                }
            }

            // No explicit code present → generic API error with code 0 (enhancement)
            return Some((0, "API Error".to_string()));
        }

        None
    }

    /// Extract message from details JSON string with enhanced error extraction
    /// **Phase 2 Enhancement:** Iterate through ALL content items, not just the first
    fn extract_message_from_details(&self, details: &str) -> String {
        if let Ok(content_array) = serde_json::from_str::<Vec<Value>>(details) {
            // Phase 2: Iterate through all content items to find error information
            for content_item in &content_array {
                if let Some(text) = content_item.get("text").and_then(|t| t.as_str()) {
                    if let Some((_, message)) = self.parse_error_text(text) {
                        return message;
                    }
                }
            }
        }
        "Unknown error".to_string()
    }
}

/// Internal struct for parsing transcript entries
#[derive(Debug)]
#[allow(dead_code)] // Fields are used for parsing but clippy can't detect due to Debug derive
struct TranscriptErrorEntry {
    pub parent_uuid: String,
    pub timestamp: String,
    pub session_id: String,
    pub project_path: String,
    pub http_code: u16,
    pub details: String,
}

impl Default for JsonlMonitor {
    fn default() -> Self {
        // Phase 2: Constructor never fails, no .expect() needed
        Self::new()
    }
}
