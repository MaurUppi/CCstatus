use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use std::io::{Write, BufReader};
use std::path::PathBuf;
use std::env;
use std::fs::{File, OpenOptions};
use std::collections::HashMap;

use serde::{Serialize, Deserialize};
use chrono::Local;
use uuid::Uuid;
use flate2::{write::GzEncoder, Compression};
use fs2::FileExt;
use regex::Regex;

// Hardcoded configuration - no environment variables needed
const LOG_ROTATION_SIZE_MB: u64 = 8;
const MAX_ARCHIVES: u32 = 5;
const ROTATION_CHECK_INTERVAL: u32 = 200;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LogEntry {
    timestamp: String,                                      // ISO-8601 with timezone
    level: String,                                          // DEBUG, ERROR, PERF, CRED, NETWORK
    component: String,                                      // Component name
    event: String,                                          // Event type
    message: String,                                        // Human readable message (redacted)
    correlation_id: Option<String>,                         // For tracking multi-step operations
    fields: HashMap<String, serde_json::Value>,            // Structured data
}

struct RotatingLogger {
    log_path: PathBuf,
    write_count: AtomicU32,
}

impl RotatingLogger {
    pub fn new(log_path: PathBuf) -> Self {
        // Ensure parent directory exists
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        
        Self {
            log_path,
            write_count: AtomicU32::new(0),
        }
    }
    
    pub fn write_with_rotation(&self, json_line: &str) -> Result<(), std::io::Error> {
        // Check for rotation every ROTATION_CHECK_INTERVAL writes
        if self.write_count.fetch_add(1, Ordering::Relaxed) % ROTATION_CHECK_INTERVAL == 0 {
            let _ = self.rotate_if_needed(); // Don't let rotation errors stop logging
        }
        
        // Append JSON line to current log
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
            
        writeln!(file, "{}", json_line)?;
        Ok(())
    }
    
    fn rotate_if_needed(&self) -> Result<(), std::io::Error> {
        if !self.needs_rotation()? {
            return Ok(());
        }
        
        // File locking to prevent concurrent rotation
        let lock_path = self.log_path.with_extension("lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)?;
        
        match lock_file.try_lock_exclusive() {
            Ok(()) => {
                // Double-check if rotation is still needed after acquiring lock
                if self.needs_rotation()? {
                    self.perform_rotation()?;
                }
                let _ = std::fs::remove_file(&lock_path);
                Ok(())
            }
            Err(_) => {
                // Another process is rotating, skip this time
                Ok(())
            }
        }
    }
    
    fn needs_rotation(&self) -> Result<bool, std::io::Error> {
        if !self.log_path.exists() {
            return Ok(false);
        }
        
        let metadata = std::fs::metadata(&self.log_path)?;
        Ok(metadata.len() >= LOG_ROTATION_SIZE_MB * 1024 * 1024)
    }
    
    fn perform_rotation(&self) -> Result<(), std::io::Error> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let base_name = self.log_path.file_stem().unwrap().to_str().unwrap();
        let archive_name = format!("{}.{}.gz", base_name, timestamp);
        let archive_path = self.log_path.parent().unwrap().join(archive_name);
        
        // Atomic rotation: move current log to temp, compress, cleanup
        let temp_path = self.log_path.with_extension("rotating");
        std::fs::rename(&self.log_path, &temp_path)?;
        
        // Compress the rotated file
        let source_file = File::open(&temp_path)?;
        let target_file = File::create(&archive_path)?;
        let mut encoder = GzEncoder::new(target_file, Compression::default());
        std::io::copy(&mut BufReader::new(source_file), &mut encoder)?;
        encoder.finish()?;
        
        // Remove temporary file
        std::fs::remove_file(&temp_path)?;
        
        // Cleanup old archives (keep last MAX_ARCHIVES)
        let _ = self.cleanup_old_archives(); // Don't let cleanup errors stop rotation
        
        Ok(())
    }
    
    fn cleanup_old_archives(&self) -> Result<(), std::io::Error> {
        let log_dir = self.log_path.parent().unwrap();
        let base_name = self.log_path.file_stem().unwrap().to_str().unwrap();
        
        let mut archives = Vec::new();
        for entry in std::fs::read_dir(log_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            
            if name.starts_with(&format!("{}.", base_name)) && name.ends_with(".gz") {
                archives.push((entry.path(), entry.metadata()?.modified()?));
            }
        }
        
        // Keep only the most recent MAX_ARCHIVES
        archives.sort_by_key(|(_, modified)| *modified);
        if archives.len() > MAX_ARCHIVES as usize {
            let to_remove = archives.len() - MAX_ARCHIVES as usize;
            for (path, _) in archives.iter().take(to_remove) {
                let _ = std::fs::remove_file(path); // Ignore individual cleanup errors
            }
        }
        
        Ok(())
    }
}

pub struct EnhancedDebugLogger {
    enabled: bool,
    rotating_logger: Option<Arc<Mutex<RotatingLogger>>>,
    session_id: String, // Correlation ID for this session
    redaction_patterns: Vec<Regex>,
}

impl EnhancedDebugLogger {
    pub fn new() -> Self {
        let enabled = Self::parse_debug_enabled();
        let session_id = Uuid::new_v4().to_string()[..8].to_string();
        
        let rotating_logger = if enabled {
            let log_path = Self::get_log_path();
            Some(Arc::new(Mutex::new(RotatingLogger::new(log_path))))
        } else {
            None
        };
        
        // Compile redaction patterns once at startup
        let redaction_patterns = Self::compile_redaction_patterns();
        
        Self { 
            enabled, 
            rotating_logger, 
            session_id,
            redaction_patterns,
        }
    }
    
    /// Parse debug enabled status from CCSTATUS_DEBUG environment variable only
    /// Supports: true/false, 1/0, yes/no, on/off (case insensitive)
    fn parse_debug_enabled() -> bool {
        env::var("CCSTATUS_DEBUG")
            .map(|v| match v.trim().to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                "" => false, // Empty value is disabled (no legacy behavior)
                _ => false,
            })
            .unwrap_or(false)
    }
    
    fn get_log_path() -> PathBuf {
        let mut log_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        log_path.push(".claude");
        log_path.push("ccstatus");
        log_path.push("ccstatus-debug.log");
        log_path
    }
    
    fn compile_redaction_patterns() -> Vec<Regex> {
        let patterns = [
            r"(?i)authorization[:\s]+[^\s\n]+",
            r"(?i)bearer[:\s]+[^\s\n]+",
            r"(?i)token[:\s]+[^\s\n]+",
            r"(?i)password[:\s]+[^\s\n]+",
            r"(?i)api[_-]?key[:\s]+[^\s\n]+",
            r"(?i)secret[:\s]+[^\s\n]+",
        ];
        
        patterns
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect()
    }
    
    /// Redaction guardrails for sensitive data
    fn redact_sensitive_data(&self, text: &str) -> String {
        let mut redacted = text.to_string();
        
        // Apply redaction patterns
        for regex in &self.redaction_patterns {
            redacted = regex.replace_all(&redacted, "[REDACTED]").to_string();
        }
        
        // Redact suspiciously long strings (potential tokens)
        if redacted.len() > 100 && !redacted.contains(' ') && redacted.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)) {
            redacted = format!("[REDACTED_LONG_STRING_{}chars]", redacted.len());
        }
        
        redacted
    }
    
    /// Core synchronous logging method with JSON Lines format
    fn log_sync(
        &self, 
        level: &str, 
        component: &str, 
        event: &str, 
        message: &str,
        correlation_id: Option<String>,
        fields: HashMap<String, serde_json::Value>
    ) {
        if !self.enabled {
            return;
        }
        
        let entry = LogEntry {
            timestamp: Local::now().to_rfc3339(),
            level: level.to_string(),
            component: component.to_string(),
            event: event.to_string(),
            message: self.redact_sensitive_data(message),
            correlation_id: correlation_id.or_else(|| Some(self.session_id.clone())),
            fields,
        };
        
        if let Some(logger) = &self.rotating_logger {
            if let Ok(logger) = logger.lock() {
                if let Ok(json_line) = serde_json::to_string(&entry) {
                    let _ = logger.write_with_rotation(&json_line); // Don't crash on logging errors
                }
            }
        }
    }
    
    // Synchronous methods for short-lived processes
    
    pub fn debug_sync(&self, component: &str, event: &str, message: &str) {
        self.log_sync("DEBUG", component, event, message, None, HashMap::new());
    }
    
    pub fn error_sync(&self, component: &str, event: &str, message: &str) {
        self.log_sync("ERROR", component, event, message, None, HashMap::new());
    }
    
    pub fn performance_sync(&self, component: &str, operation: &str, duration_ms: u64) {
        let mut fields = HashMap::new();
        fields.insert("duration_ms".to_string(), serde_json::Value::Number(duration_ms.into()));
        
        self.log_sync("PERF", component, "operation_complete", operation, None, fields);
    }
    
    // Typed methods for network monitoring events
    
    pub fn network_probe_start(&self, mode: &str, timeout_ms: u64, correlation_id: String) {
        let mut fields = HashMap::new();
        fields.insert("mode".to_string(), serde_json::Value::String(mode.to_string()));
        fields.insert("timeout_ms".to_string(), serde_json::Value::Number(timeout_ms.into()));
        
        self.log_sync("NETWORK", "HttpMonitor", "probe_start", 
                     &format!("Starting probe in {} mode", mode), 
                     Some(correlation_id), fields);
    }
    
    pub fn network_probe_end(&self, status: &str, http_status: Option<u16>, duration_ms: u64, correlation_id: String) {
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), serde_json::Value::String(status.to_string()));
        fields.insert("duration_ms".to_string(), serde_json::Value::Number(duration_ms.into()));
        
        if let Some(code) = http_status {
            fields.insert("http_status".to_string(), serde_json::Value::Number(code.into()));
        }
        
        self.log_sync("NETWORK", "HttpMonitor", "probe_end",
                     &format!("Probe completed: {} ({}ms)", status, duration_ms),
                     Some(correlation_id), fields);
    }
    
    pub fn credential_info_safe(&self, source: &str, token_length: usize) {
        let mut fields = HashMap::new();
        fields.insert("source".to_string(), serde_json::Value::String(source.to_string()));
        fields.insert("token_length".to_string(), serde_json::Value::Number(token_length.into()));
        
        self.log_sync("CRED", "CredentialManager", "token_loaded",
                     &format!("Using credentials from {} ({} chars)", source, token_length),
                     None, fields);
    }
    
    pub fn state_write_summary(&self, status: &str, p95_ms: u64, rolling_window_size: u32) {
        let mut fields = HashMap::new();
        fields.insert("status".to_string(), serde_json::Value::String(status.to_string()));
        fields.insert("p95_latency_ms".to_string(), serde_json::Value::Number(p95_ms.into()));
        fields.insert("rolling_window_size".to_string(), serde_json::Value::Number(rolling_window_size.into()));
        
        self.log_sync("NETWORK", "StateManager", "state_update",
                     &format!("State updated: {} (p95: {}ms)", status, p95_ms),
                     None, fields);
    }
    
    pub fn jsonl_error_summary(&self, error_code: &str, _error_message: &str, timestamp: &str) {
        let mut fields = HashMap::new();
        fields.insert("error_code".to_string(), serde_json::Value::String(error_code.to_string()));
        fields.insert("error_timestamp".to_string(), serde_json::Value::String(timestamp.to_string()));
        
        self.log_sync("NETWORK", "JsonlMonitor", "jsonl_error",
                     &format!("JSONL processing error: {}", error_code),
                     None, fields);
    }
    
    pub fn render_summary(&self, emoji: &str, status: &str) {
        let mut fields = HashMap::new();
        fields.insert("emoji".to_string(), serde_json::Value::String(emoji.to_string()));
        fields.insert("render_status".to_string(), serde_json::Value::String(status.to_string()));
        
        self.log_sync("NETWORK", "StatusRenderer", "render_complete",
                     &format!("Status rendered: {} {}", emoji, status),
                     None, fields);
    }
    
    // Compatibility with existing async methods (deprecated but maintained for transition)
    
    pub async fn debug(&self, component: &str, message: &str) {
        self.debug_sync(component, "legacy_debug", message);
    }
    
    pub async fn error(&self, component: &str, message: &str) {
        self.error_sync(component, "legacy_error", message);
    }
    
    pub async fn performance(&self, component: &str, operation: &str, duration_ms: u64) {
        self.performance_sync(component, operation, duration_ms);
    }
    
    pub async fn credential_info(&self, _component: &str, source: &str, token_length: usize) {
        self.credential_info_safe(source, token_length);
    }
    
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    pub fn get_session_id(&self) -> &str {
        &self.session_id
    }
}

// Factory function for backward compatibility
pub fn get_debug_logger() -> EnhancedDebugLogger {
    EnhancedDebugLogger::new()
}

