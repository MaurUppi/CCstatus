// JSONL transcript monitoring with error capture and persistence
use crate::core::segments::network::types::{JsonlError, NetworkError};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tokio::fs;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncSeekExt, AsyncReadExt};
use std::io::SeekFrom;

/// Captured API error with aggregation tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedApiError {
    #[serde(rename = "isApiErrorMessage")]
    pub is_api_error_message: String,
    pub details: String,
    pub http_code: u16,
    pub first_occurrence: String,
    pub last_occurrence: String,
    pub count: u32,
    pub session_id: String,
    pub project_path: String,
}

/// Monitor for scanning JSONL transcript files and detecting API errors
/// 
/// **Primary Purpose:** RED gate control for monitoring system
/// - Detects `isApiErrorMessage: true` entries in transcript tail
/// - Returns boolean detection status + most recent error details
/// - Optimized for large files via configurable tail reading
/// 
/// **Debug Mode:** Set `CCSTATUS_DEBUG=true/1/yes` to enable error aggregation and persistence
/// **Performance:** Configurable via `CCSTATUS_JSONL_TAIL_KB` environment variable (default: 64KB)
pub struct JsonlMonitor {
    captured_errors_path: Option<PathBuf>,
    captured_errors: HashMap<String, CapturedApiError>,
    debug_mode: bool,
}

impl JsonlMonitor {
    pub fn new() -> Result<Self, NetworkError> {
        let debug_mode = std::env::var("CCSTATUS_DEBUG").unwrap_or_default() == "true";
        
        let captured_errors_path = if debug_mode {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .map_err(|_| NetworkError::HomeDirNotFound)?;
            
            Some(PathBuf::from(home)
                .join(".claude")
                .join("ccstatus")
                .join("ccstatus-captured-error.json"))
        } else {
            None
        };
        
        let mut monitor = Self {
            captured_errors_path,
            captured_errors: HashMap::new(),
            debug_mode,
        };
        
        // Load existing captured errors only in debug mode
        if debug_mode {
            if let Err(e) = monitor.load_captured_errors() {
                eprintln!("Warning: Failed to load captured errors: {}", e);
            }
        }
        
        Ok(monitor)
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
        &mut self,
        transcript_path: P,
    ) -> Result<(bool, Option<JsonlError>), NetworkError> {
        let path = transcript_path.as_ref();
        
        // Check if file exists
        if !path.exists() {
            return Ok((false, None));
        }

        // Read only the tail content for efficiency with large files
        let content = self.read_tail_content(path).await?;

        // Parse and capture errors from tail content only
        self.parse_and_capture_errors(&content).await
    }

    /// Read only the tail N KB from the file to avoid memory issues with large files
    /// Configurable via CCSTATUS_JSONL_TAIL_KB environment variable (default: 64KB)
    async fn read_tail_content<P: AsRef<Path>>(&self, path: P) -> Result<String, NetworkError> {
        // Get configurable tail size (default 64KB)
        let tail_kb = std::env::var("CCSTATUS_JSONL_TAIL_KB")
            .unwrap_or_else(|_| "64".to_string())
            .parse::<u64>()
            .unwrap_or(64);
        let tail_bytes = tail_kb * 1024;
        const MAX_LINE_LENGTH: usize = 64 * 1024; // 64KB per line limit

        // Open file and get metadata
        let mut file = tokio::fs::File::open(path).await
            .map_err(|e| NetworkError::ConfigReadError(format!("Failed to open transcript: {}", e)))?;
        
        let file_len = file.metadata().await
            .map_err(|e| NetworkError::ConfigReadError(format!("Failed to get file metadata: {}", e)))?
            .len();

        // If file is smaller than tail size, read entire file
        if file_len <= tail_bytes {
            let mut content = String::new();
            file.read_to_string(&mut content).await
                .map_err(|e| NetworkError::ConfigReadError(format!("Failed to read small file: {}", e)))?;
            return Ok(content);
        }

        // Seek to tail position
        let seek_pos = file_len - tail_bytes;
        file.seek(SeekFrom::Start(seek_pos)).await
            .map_err(|e| NetworkError::ConfigReadError(format!("Failed to seek to tail: {}", e)))?;

        // Read from seek position to find first complete line boundary
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await
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

    /// Parse transcript content and detect API errors for RED gate control
    /// Returns (error_detected: bool, last_error_event: Option<JsonlError>)
    async fn parse_and_capture_errors(&mut self, content: &str) -> Result<(bool, Option<JsonlError>), NetworkError> {
        let mut last_error: Option<JsonlError> = None;
        let mut error_detected = false;
        let mut captured_any_new = false;

        // Process each line to find errors
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(Some(error_entry)) = self.parse_jsonl_line(line) {
                error_detected = true;
                
                // Only capture errors for persistence in debug mode
                if self.debug_mode {
                    if self.capture_error(&error_entry).await? {
                        captured_any_new = true;
                    }
                }
                
                // Keep track of most recent error for return value (always needed for RED gate)
                last_error = Some(JsonlError {
                    timestamp: error_entry.timestamp.clone(),
                    code: error_entry.http_code,
                    message: self.extract_message_from_details(&error_entry.details),
                });
            }
        }

        // Save captured errors if we found new ones (debug mode only)
        if self.debug_mode && captured_any_new {
            self.save_captured_errors().await?;
        }

        Ok((error_detected, last_error))
    }

    /// Parse a single JSONL line for error information with robustness improvements
    fn parse_jsonl_line(&self, line: &str) -> Result<Option<TranscriptErrorEntry>, NetworkError> {
        const MAX_LINE_LENGTH: usize = 64 * 1024; // 64KB per line limit
        
        // Skip oversized lines to prevent memory pressure
        if line.len() > MAX_LINE_LENGTH {
            eprintln!("Warning: Skipping oversized JSONL line ({} bytes)", line.len());
            return Ok(None);
        }

        // Skip malformed JSON lines instead of failing entire operation
        let json: Value = match serde_json::from_str(line) {
            Ok(json) => json,
            Err(e) => {
                eprintln!("Warning: Skipping malformed JSON line: {}", e);
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
        let parent_uuid = json.get("parentUuid")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
            
        let timestamp = json.get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let session_id = json.get("sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let cwd = json.get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Extract message content and HTTP code
        let mut http_code = 0u16;
        let mut details = "[]".to_string();

        if let Some(content_array) = json.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array()) 
        {
            details = serde_json::to_string(content_array)
                .unwrap_or_else(|_| "[]".to_string());
            
            // Extract HTTP code from first text content
            if let Some(first_content) = content_array.get(0) {
                if let Some(text) = first_content.get("text").and_then(|t| t.as_str()) {
                    if let Some((code, _)) = self.parse_error_text(text) {
                        http_code = code;
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
                            if let Some(error_msg) = error_json.get("error")
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
                    }.to_string();
                    
                    return Some((code, message));
                }
            }
        }
        None
    }

    /// Capture error for debug logging (simplified for RED gate focus)
    async fn capture_error(&mut self, entry: &TranscriptErrorEntry) -> Result<bool, NetworkError> {
        if !self.debug_mode {
            return Ok(false); // Skip capture if not in debug mode
        }
        
        let key = format!("api_error_{}", entry.parent_uuid);
        
        // Simplified capture: just update or insert for debug logging
        match self.captured_errors.get_mut(&key) {
            Some(existing) => {
                // Update existing error (simplified tracking)
                existing.last_occurrence = entry.timestamp.clone();
                existing.count += 1;
                Ok(true) // Modified existing entry
            }
            None => {
                // Create new error entry (minimal fields for debug)
                let captured_error = CapturedApiError {
                    is_api_error_message: "true".to_string(),
                    details: entry.details.clone(),
                    http_code: entry.http_code,
                    first_occurrence: entry.timestamp.clone(),
                    last_occurrence: entry.timestamp.clone(),
                    count: 1,
                    session_id: entry.session_id.clone(),
                    project_path: entry.project_path.clone(),
                };
                
                self.captured_errors.insert(key, captured_error);
                Ok(true) // Added new entry
            }
        }
    }

    /// Load existing captured errors from disk (debug mode only)
    fn load_captured_errors(&mut self) -> Result<(), NetworkError> {
        let captured_errors_path = match &self.captured_errors_path {
            Some(path) => path,
            None => return Ok(()), // Not in debug mode
        };
        
        if !captured_errors_path.exists() {
            return Ok(()); // File doesn't exist yet
        }
        
        let content = std::fs::read_to_string(captured_errors_path)
            .map_err(|e| NetworkError::StateFileError(format!("Failed to read captured errors: {}", e)))?;
            
        if content.trim().is_empty() {
            return Ok(());
        }
        
        let data: HashMap<String, CapturedApiError> = serde_json::from_str(&content)
            .map_err(|e| NetworkError::StateFileError(format!("Failed to parse captured errors: {}", e)))?;
            
        self.captured_errors = data;
        Ok(())
    }

    /// Save captured errors to disk atomically (debug mode only)
    async fn save_captured_errors(&self) -> Result<(), NetworkError> {
        let captured_errors_path = match &self.captured_errors_path {
            Some(path) => path,
            None => return Ok(()), // Not in debug mode, skip saving
        };
        
        // Ensure directory exists
        if let Some(parent) = captured_errors_path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| NetworkError::StateFileError(format!("Failed to create directory: {}", e)))?;
        }
        
        // Serialize to JSON
        let json_content = serde_json::to_string_pretty(&self.captured_errors)
            .map_err(|e| NetworkError::StateFileError(format!("Failed to serialize errors: {}", e)))?;
        
        // Atomic write: temp file + rename
        let temp_path = captured_errors_path.with_extension("tmp");
        
        fs::write(&temp_path, &json_content).await
            .map_err(|e| NetworkError::StateFileError(format!("Failed to write temp file: {}", e)))?;
            
        fs::rename(&temp_path, captured_errors_path).await
            .map_err(|e| NetworkError::StateFileError(format!("Failed to rename temp file: {}", e)))?;
        
        Ok(())
    }
    
    /// Extract message from details JSON string
    fn extract_message_from_details(&self, details: &str) -> String {
        if let Ok(content_array) = serde_json::from_str::<Vec<Value>>(details) {
            if let Some(first_item) = content_array.get(0) {
                if let Some(text) = first_item.get("text").and_then(|t| t.as_str()) {
                    if let Some((_, message)) = self.parse_error_text(text) {
                        return message;
                    }
                }
            }
        }
        "Unknown error".to_string()
    }

    /// Get captured error statistics (debug mode only)
    /// Note: For production RED gate control, use scan_tail() return values instead
    pub fn get_error_stats(&self) -> ErrorStats {
        if !self.debug_mode {
            return ErrorStats {
                total_unique_errors: 0,
                total_occurrences: 0,
            };
        }
        
        let total_errors = self.captured_errors.len();
        let total_occurrences = self.captured_errors.values()
            .map(|e| e.count)
            .sum::<u32>();
            
        ErrorStats {
            total_unique_errors: total_errors,
            total_occurrences,
        }
    }
}

/// Internal struct for parsing transcript entries
#[derive(Debug)]
struct TranscriptErrorEntry {
    pub parent_uuid: String,
    pub timestamp: String,
    pub session_id: String,
    pub project_path: String,
    pub http_code: u16,
    pub details: String,
}

/// Error statistics for debug mode only
/// 
/// **Note:** For production RED gate control, use `scan_tail()` return values instead.
/// These stats are only available when `CCSTATUS_DEBUG=true` for development debugging.
#[derive(Debug)]
pub struct ErrorStats {
    pub total_unique_errors: usize,
    pub total_occurrences: u32,
}

impl Default for JsonlMonitor {
    fn default() -> Self {
        Self::new().expect("Failed to create JsonlMonitor")
    }
}