use std::sync::Once;
use std::io::Write;
use std::path::PathBuf;
use std::env;
use std::fs::OpenOptions;
use chrono::Local;

static LOG_INIT: Once = Once::new();

pub struct DebugLogger {
    enabled: bool,
    log_path: PathBuf,
}

impl DebugLogger {
    pub fn new() -> Self {
        let enabled = Self::parse_debug_enabled();

        let mut log_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        log_path.push(".claude");
        log_path.push("ccstatus");
        log_path.push("ccstatus-debug.log");

        // Ensure directory exists
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Session refresh - clear log file at start of each session
        LOG_INIT.call_once(|| {
            if enabled {
                let _ = std::fs::write(&log_path, "");
            }
        });

        Self { enabled, log_path }
    }

    /// Parse debug enabled status from environment variables with flexible boolean parsing
    /// 
    /// Priority order:
    /// 1. `ccstatus_debug` (lowercase) - parsed as flexible boolean
    /// 2. `CCSTATUS_DEBUG` (uppercase, legacy) - existence enables, value parsed as flexible boolean if present
    /// 3. Default: false
    fn parse_debug_enabled() -> bool {
        // Priority 1: lowercase ccstatus_debug
        if let Ok(val) = env::var("ccstatus_debug") {
            return Self::parse_flexible_bool(&val).unwrap_or(false);
        }
        
        // Priority 2: uppercase CCSTATUS_DEBUG (legacy behavior)
        if let Ok(val) = env::var("CCSTATUS_DEBUG") {
            if val.is_empty() {
                return true; // existence enables (legacy behavior)
            }
            return Self::parse_flexible_bool(&val).unwrap_or(false);
        }
        
        false
    }

    /// Parse flexible boolean values from strings
    /// 
    /// Supports: true/false, 1/0, yes/no, on/off (case insensitive)
    /// Returns None for unrecognized values
    fn parse_flexible_bool(s: &str) -> Option<bool> {
        match s.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    }

    pub async fn debug(&self, component: &str, message: &str) {
        if !self.enabled {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] [DEBUG] [{}] {}\n", timestamp, component, message);

        // Use blocking task for file I/O
        let log_path = self.log_path.clone();
        tokio::task::spawn_blocking(move || {
            let _ = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut file| file.write_all(log_entry.as_bytes()));
        }).await.ok();
    }

    pub async fn error(&self, component: &str, message: &str) {
        if !self.enabled {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] [ERROR] [{}] {}\n", timestamp, component, message);

        // Use blocking task for file I/O
        let log_path = self.log_path.clone();
        tokio::task::spawn_blocking(move || {
            let _ = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut file| file.write_all(log_entry.as_bytes()));
        }).await.ok();
    }

    pub async fn performance(&self, component: &str, operation: &str, duration_ms: u64) {
        if !self.enabled {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] [PERF] [{}] {} took {}ms\n", timestamp, component, operation, duration_ms);

        let log_path = self.log_path.clone();
        tokio::task::spawn_blocking(move || {
            let _ = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut file| file.write_all(log_entry.as_bytes()));
        }).await.ok();
    }

    pub async fn credential_info(&self, component: &str, source: &str, token_length: usize) {
        if !self.enabled {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_entry = format!("[{}] [CRED] [{}] Using credentials from {} (token length: {} chars)\n", 
                              timestamp, component, source, token_length);

        let log_path = self.log_path.clone();
        tokio::task::spawn_blocking(move || {
            let _ = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut file| file.write_all(log_entry.as_bytes()));
        }).await.ok();
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

pub fn get_debug_logger() -> DebugLogger {
    DebugLogger::new()
}