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
/// - Detects `isApiErrorMessage: true` entries in transcript tail
/// - Returns boolean detection status + most recent error details
/// - Optimized for large files via configurable tail reading
/// - Memory-only operation with no persistence (complies with module boundaries)
///
/// **Debug Mode:** Set `CCSTATUS_DEBUG=true/1/yes/on` (flexible boolean) for enhanced logging
/// **Performance:** Configurable via `CCSTATUS_JSONL_TAIL_KB` environment variable (default: 64KB, max: 10MB)
/// **Security:** Input validation, structured logging, bounded memory usage
pub struct JsonlMonitor {
    debug_logger: Option<Arc<EnhancedDebugLogger>>,
}

impl JsonlMonitor {
    /// Create a new JsonlMonitor with optional debug logging
    ///
    /// **Security:** Never fails - graceful degradation without HOME directory
    /// **Debug Mode:** Uses flexible boolean parsing (true/1/yes/on case-insensitive)
    pub fn new() -> Self {
        Self::with_debug_logger(None)
    }

    /// Create JsonlMonitor with custom debug logger (for testing)
    pub fn with_debug_logger(custom_logger: Option<Arc<EnhancedDebugLogger>>) -> Self {
        let debug_logger = custom_logger.or_else(|| {
            // Use DebugLogger's flexible boolean parsing
            if Self::parse_debug_enabled() {
                Some(Arc::new(get_debug_logger()))
            } else {
                None
            }
        });

        Self { debug_logger }
    }

    /// Parse debug enabled status using flexible boolean parsing
    /// Supports: true/false, 1/0, yes/no, on/off (case insensitive)
    fn parse_debug_enabled() -> bool {
        std::env::var("CCSTATUS_DEBUG")
            .map(|v| match v.trim().to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                "" => false,
                _ => false,
            })
            .unwrap_or(false)
    }

    /// Scan transcript tail for API error detection - optimized for RED gate control
    ///
    /// **Return Semantics for RED Gate Control:**
    /// - Returns `(error_detected: bool, last_error_event: Option<JsonlError>)`
    /// - `error_detected`: true if ANY API errors found in tail content → triggers RED state
    /// - `last_error_event`: Most recent error details for display (timestamp from transcript)
    ///
    /// **Usage Pattern:**
    /// ```rust
    /// let (error_detected, last_error_event) = monitor.scan_tail(path).await?;
    /// if error_detected {
    ///     // Trigger RED gate state
    ///     if let Some(error) = last_error_event {
    ///         // Display error details: error.message, error.code, error.timestamp
    ///     }
    /// }
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

                // Debug logging only (no persistence)
                if let Some(logger) = &self.debug_logger {
                    let extracted_message = self.extract_message_from_details(&error_entry.details);
                    logger.jsonl_error_summary(
                        &error_entry.http_code.to_string(),
                        &extracted_message,
                        &error_entry.timestamp,
                    );
                }

                // Keep track of most recent error for return value (always needed for RED gate)
                last_error = Some(JsonlError {
                    timestamp: error_entry.timestamp.clone(),
                    code: error_entry.http_code,
                    message: self.extract_message_from_details(&error_entry.details),
                });
            }
        }

        // Summary logging for debug mode
        if let Some(logger) = &self.debug_logger {
            if error_count > 0 {
                logger.debug_sync(
                    "JsonlMonitor",
                    "tail_scan_complete",
                    &format!("Scanned tail content: {} API errors found", error_count),
                );
            } else {
                logger.debug_sync(
                    "JsonlMonitor",
                    "tail_scan_complete",
                    "Scanned tail content: no API errors found",
                );
            }
        }

        Ok((error_detected, last_error))
    }

    /// Parse a single JSONL line for error information with robustness improvements
    fn parse_jsonl_line(&self, line: &str) -> Result<Option<TranscriptErrorEntry>, NetworkError> {
        const MAX_LINE_LENGTH: usize = 1024 * 1024; // Phase 2: 1MB per line limit (matches read_tail_content)

        // Skip oversized lines to prevent memory pressure
        if line.len() > MAX_LINE_LENGTH {
            // Phase 2: Use debug logger for oversized line warnings
            if let Some(logger) = &self.debug_logger {
                logger.debug_sync(
                    "JsonlMonitor",
                    "oversized_line_skipped",
                    &format!("Skipped oversized line: {} bytes", line.len()),
                );
            }
            return Ok(None);
        }

        // Skip malformed JSON lines instead of failing entire operation
        let json: Value = match serde_json::from_str(line) {
            Ok(json) => json,
            Err(e) => {
                // Phase 2: Use debug logger for malformed JSON warnings
                if let Some(logger) = &self.debug_logger {
                    let error_msg = e.to_string();
                    let truncated_msg = if error_msg.len() > 100 {
                        format!("{}...", &error_msg[..100])
                    } else {
                        error_msg
                    };
                    logger.debug_sync(
                        "JsonlMonitor",
                        "malformed_json_skipped",
                        &format!("Skipped malformed JSON: {}", truncated_msg),
                    );
                }
                return Ok(None);
            }
        };

        // Check for isApiErrorMessage flag
        if let Some(is_error) = json.get("isApiErrorMessage").and_then(|v| v.as_bool()) {
            if is_error {
                return Ok(Some(self.extract_transcript_error(&json)?));
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

    /// Parse error text to extract code and message
    fn parse_error_text(&self, text: &str) -> Option<(u16, String)> {
        if text.starts_with("API Error: ") {
            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.len() >= 3 {
                if let Ok(code) = parts[2].parse::<u16>() {
                    // Try to parse JSON for detailed error message
                    if let Some(json_start) = text.find('{') {
                        let json_part = &text[json_start..];
                        if let Ok(error_json) = serde_json::from_str::<Value>(json_part) {
                            if let Some(error_msg) = error_json
                                .get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(|m| m.as_str())
                            {
                                return Some((code, error_msg.to_string()));
                            }
                        }
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
