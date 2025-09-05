//! Unified credential management for network monitoring
//!
//! This module provides comprehensive credential resolution from multiple sources including
//! environment variables, OAuth keychain integration, shell configuration files, and
//! Claude Code configuration files. It implements a priority-based credential resolution
//! system with cross-platform support for extracting API credentials from various sources.
//!
//! ## Architecture & Priority Hierarchy
//!
//! The main entry point is `CredentialManager` which orchestrates credential resolution
//! using this strict priority hierarchy:
//!
//! 1. **Environment variables** (highest priority)
//!    - Base URL priority: `ANTHROPIC_BASE_URL` > `ANTHROPIC_BEDROCK_BASE_URL` > `ANTHROPIC_VERTEX_BASE_URL`
//!    - Token priority: `ANTHROPIC_AUTH_TOKEN` > `ANTHROPIC_API_KEY`
//!    - Requires both a base URL and token to be present (any combination)
//!    - Empty strings are treated as missing values
//!
//! 2. **OAuth (macOS only)** - macOS Keychain integration
//!    - Uses `security find-generic-password -s "Claude Code-credentials"`
//!    - Returns fixed credentials when keychain item exists:
//!      - Base URL: `https://api.anthropic.com`
//!      - Auth Token: `probe-invalid-key` (dummy key for testing)
//!    - Fails silently on errors (returns None)
//!    - Skipped entirely on non-macOS platforms
//!
//! 3. **Shell configuration files** (.zshrc, .bashrc, PowerShell profiles)
//!    - Cross-platform shell parsing with auto-detection
//!    - Supports export statements, function-based variable definitions, and non-exported assignments
//!
//! 4. **Claude Code configuration files** (lowest priority)
//!    - JSON-based configuration files in `.claude/` directories
//!
//! ## Environment Variable Combination Rules
//!
//! Environment variables are combined using priority chains to handle multiple API endpoints:
//!
//! - **Valid combinations**: Any base URL + any token (6 total combinations)
//! - **Base URL candidates** (in priority order):
//!   1. `ANTHROPIC_BASE_URL` (primary - official Anthropic API)
//!   2. `ANTHROPIC_BEDROCK_BASE_URL` (secondary - AWS Bedrock)
//!   3. `ANTHROPIC_VERTEX_BASE_URL` (tertiary - Google Vertex AI)
//! - **Token candidates** (in priority order):
//!   1. `ANTHROPIC_AUTH_TOKEN` (primary - session tokens)
//!   2. `ANTHROPIC_API_KEY` (secondary - API keys)
//!
//! ## OAuth Integration Details
//!
//! The OAuth integration provides cross-platform OAuth detection with fixed credentials:
//!
//! - **Detection Signals**: Test flag `CCSTATUS_TEST_OAUTH_PRESENT`, `CLAUDE_CODE_OAUTH_TOKEN`, and (macOS) Keychain presence
//! - **Behavior on Selection**: Fixed `base_url=https://api.anthropic.com`, `auth_token=probe-invalid-key`, source=`oauth`
//! - **Never Forward OAuth Tokens**: OAuth tokens are never sent to REST API
//! - **Cross-platform**: `CLAUDE_CODE_OAUTH_TOKEN` env var works on all platforms
//! - **macOS Keychain**: `security find-generic-password -s "Claude Code-credentials"`
//! - **Error Handling**: All OAuth errors fail silently, continuing to next source
//! - **Test Override**: Set `CCSTATUS_TEST_OAUTH_PRESENT=1` for test simulation
//!
//! ## Shell Parsing Support
//!
//! Multi-platform shell configuration parsing with automatic detection:
//!
//! - **Bash/Zsh**: Export statements, function-based variable definitions, and non-exported assignments
//! - **PowerShell**: $env: syntax and [Environment]::SetEnvironmentVariable calls
//! - **Cross-platform**: OS-specific defaults with manual override support
//!

use regex::Regex;
use serde_json::Value;
use std::env;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::core::network::types::{ApiCredentials, CredentialSource, NetworkError};

/// Shell types supported for configuration parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ShellType {
    /// Z shell (default on macOS)
    Zsh,
    /// Bourne Again Shell (default on Linux)
    Bash,
    /// Microsoft PowerShell (default on Windows)
    PowerShell,
    /// Unknown or unsupported shell
    Unknown,
}

/// Manages credential resolution from multiple sources with shell configuration parsing
pub struct CredentialManager {
    claude_config_paths: Vec<PathBuf>,
}

impl CredentialManager {
    // Environment variable constants
    const ENV_BASE_URL: &'static str = "ANTHROPIC_BASE_URL";
    const ENV_BEDROCK_BASE_URL: &'static str = "ANTHROPIC_BEDROCK_BASE_URL";
    const ENV_VERTEX_BASE_URL: &'static str = "ANTHROPIC_VERTEX_BASE_URL";
    const ENV_AUTH_TOKEN: &'static str = "ANTHROPIC_AUTH_TOKEN";
    const ENV_API_KEY: &'static str = "ANTHROPIC_API_KEY";

    // Test control constants
    const TEST_NO_CREDENTIALS: &'static str = "CCSTATUS_NO_CREDENTIALS";
    const TEST_OAUTH_PRESENT: &'static str = "CCSTATUS_TEST_OAUTH_PRESENT";

    // OAuth constants
    const OAUTH_KEYCHAIN_SERVICE: &'static str = "Claude Code-credentials";
    const OAUTH_FIXED_BASE_URL: &'static str = "https://api.anthropic.com";
    const OAUTH_FIXED_TOKEN: &'static str = "probe-invalid-key";
    const OAUTH_ENV_TOKEN: &'static str = "CLAUDE_CODE_OAUTH_TOKEN";

    /// Create new credential manager with auto-configured paths
    pub fn new() -> Result<Self, NetworkError> {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .map_err(|_| NetworkError::HomeDirNotFound)?;

        let home_path = PathBuf::from(home);

        let claude_config_paths = vec![
            home_path.join(".claude").join("settings.json"),
            PathBuf::from(".claude").join("settings.local.json"),
            PathBuf::from(".claude").join("settings.json"),
        ];

        Ok(Self {
            claude_config_paths,
        })
    }

    /// Logging helper for credential source start
    async fn log_source_start(
        &self,
        logger: &crate::core::network::debug_logger::EnhancedDebugLogger,
        source: &str,
    ) {
        logger
            .debug(
                "CredentialManager",
                &format!("Checking {} credentials", source),
            )
            .await;
    }

    /// Logging helper for found credentials
    async fn log_credentials_found(
        &self,
        logger: &crate::core::network::debug_logger::EnhancedDebugLogger,
        source: &str,
        creds: &ApiCredentials,
    ) {
        logger
            .debug(
                "CredentialManager",
                &format!(
                    "Found {} credentials: endpoint={}, token_len={}",
                    source,
                    creds.base_url,
                    creds.auth_token.len()
                ),
            )
            .await;
    }

    /// Logging helper for no credentials found
    async fn log_no_credentials(
        &self,
        logger: &crate::core::network::debug_logger::EnhancedDebugLogger,
        source: &str,
    ) {
        logger
            .debug(
                "CredentialManager",
                &format!("No {} credentials found", source),
            )
            .await;
    }

    /// Logging helper for credential source errors
    async fn log_source_error(
        &self,
        logger: &crate::core::network::debug_logger::EnhancedDebugLogger,
        source: &str,
        error: &NetworkError,
    ) {
        logger
            .debug(
                "CredentialManager",
                &format!("{} credential error: {}", source, error),
            )
            .await;
    }

    /// Get credentials from environment, OAuth, shell config, or Claude config files
    ///
    /// ## Error Handling Policy (IMPORTANT FOR MAINTAINERS)
    ///
    /// **Continue-on-error for all sources except Environment**
    /// - **Environment**: Returns error (highest priority, should not fail in normal operation)
    /// - **OAuth/Shell/Config**: Log error and continue to next source (graceful fallback)
    ///
    /// **When adding new credential sources:**
    /// - Follow the same pattern: try source, log result, continue on error (except for Environment)
    /// - Use the logging helpers: log_source_start(), log_credentials_found(), log_no_credentials(), log_source_error()
    /// - Maintain graceful degradation: errors should not prevent trying lower-priority sources
    /// - Document new sources in both this comment and the module-level documentation
    ///
    /// ## Priority Hierarchy
    /// 1. Environment variables (ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN)  
    /// 2. OAuth (macOS only) - uses macOS Keychain with fixed endpoint and dummy key
    /// 3. Shell configuration files (.zshrc, .bashrc, PowerShell profiles)
    /// 4. Claude Code config files
    /// 5. None (warn level - expected in some environments)
    ///
    /// ## Test-specific Behavior
    /// - Set CCSTATUS_NO_CREDENTIALS=1 to force return None (for testing unknown scenarios)
    /// - Set CCSTATUS_TEST_OAUTH_PRESENT=1 to simulate OAuth presence on macOS (deterministic testing)
    pub async fn get_credentials(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        use crate::core::network::debug_logger::get_debug_logger;
        let logger = get_debug_logger();

        // Test override: force no credentials
        if env::var(Self::TEST_NO_CREDENTIALS).unwrap_or_default() == "1" {
            logger
                .debug(
                    "CredentialManager",
                    "Test override: forcing no credentials for testing",
                )
                .await;
            return Ok(None);
        }

        logger
            .debug(
                "CredentialManager",
                "Starting credential lookup from all sources",
            )
            .await;

        // Priority 1: Environment variables (returns error on failure)
        self.log_source_start(&logger, "environment").await;
        match self.get_from_environment() {
            Ok(Some(creds)) => {
                self.log_credentials_found(&logger, "environment", &creds)
                    .await;
                return Ok(Some(creds));
            }
            Ok(None) => {
                self.log_no_credentials(&logger, "environment").await;
            }
            Err(e) => {
                self.log_source_error(&logger, "environment", &e).await;
                return Err(e); // Environment errors are not silently ignored
            }
        }

        // Priority 2: OAuth (macOS only) - continue on error
        self.log_source_start(&logger, "OAuth").await;
        match self.get_from_oauth_keychain().await {
            Ok(Some(creds)) => {
                self.log_credentials_found(&logger, "OAuth", &creds).await;
                return Ok(Some(creds));
            }
            Ok(None) => {
                self.log_no_credentials(&logger, "OAuth").await;
            }
            Err(e) => {
                self.log_source_error(&logger, "OAuth", &e).await;
                // Continue to next source (graceful fallback)
            }
        }

        // Priority 3: Shell configuration files - continue on error
        self.log_source_start(&logger, "shell config").await;
        match self.get_from_shell_config().await {
            Ok(Some(creds)) => {
                self.log_credentials_found(&logger, "shell config", &creds)
                    .await;
                return Ok(Some(creds));
            }
            Ok(None) => {
                self.log_no_credentials(&logger, "shell config").await;
            }
            Err(e) => {
                self.log_source_error(&logger, "shell config", &e).await;
                // Continue to next source (graceful fallback)
            }
        }

        // Priority 4: Claude Code config files - continue on error
        self.log_source_start(&logger, "Claude config").await;

        for (index, config_path) in self.claude_config_paths.iter().enumerate() {
            logger
                .debug(
                    "CredentialManager",
                    &format!(
                        "Checking config file #{}: {}",
                        index + 1,
                        config_path.display()
                    ),
                )
                .await;

            match self.get_from_claude_config(config_path).await {
                Ok(Some(creds)) => {
                    self.log_credentials_found(
                        &logger,
                        &format!("Claude config file #{}", index + 1),
                        &creds,
                    )
                    .await;
                    return Ok(Some(creds));
                }
                Ok(None) => {
                    logger
                        .debug(
                            "CredentialManager",
                            &format!(
                                "Config file #{} exists but has no credentials (file: {})",
                                index + 1,
                                config_path.display()
                            ),
                        )
                        .await;
                }
                Err(e) => {
                    self.log_source_error(
                        &logger,
                        &format!("Claude config file #{}", index + 1),
                        &e,
                    )
                    .await;
                }
            }
        }
        self.log_no_credentials(&logger, "Claude config").await;

        // No credentials found in any source - warn level for expected states in some environments
        logger
            .warn(
                "CredentialManager",
                "FINAL RESULT: No credentials found in any source (env, OAuth, shell, or config files)",
            )
            .await;
        Ok(None)
    }

    /// Try to get credentials from environment variables
    pub fn get_from_environment(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        // Helper function to get non-empty environment variable
        let get_non_empty_var = |name: &str| -> Result<String, env::VarError> {
            match env::var(name) {
                Ok(value) if !value.trim().is_empty() => Ok(value),
                _ => Err(env::VarError::NotPresent),
            }
        };

        // Base URL priority chain using constants
        let base_url = get_non_empty_var(Self::ENV_BASE_URL)
            .or_else(|_| get_non_empty_var(Self::ENV_BEDROCK_BASE_URL))
            .or_else(|_| get_non_empty_var(Self::ENV_VERTEX_BASE_URL));

        // Token priority chain using constants
        let auth_token = get_non_empty_var(Self::ENV_AUTH_TOKEN)
            .or_else(|_| get_non_empty_var(Self::ENV_API_KEY));

        // Return credentials when both base URL and token are present and non-empty
        if let (Ok(base_url), Ok(auth_token)) = (base_url, auth_token) {
            return Ok(Some(ApiCredentials {
                base_url,
                auth_token,
                source: CredentialSource::Environment,
                expires_at: None,
            }));
        }

        Ok(None)
    }

    /// Try to get credentials from macOS OAuth Keychain (macOS only)
    #[cfg(target_os = "macos")]
    async fn get_from_oauth_keychain(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        use crate::core::network::debug_logger::get_debug_logger;
        let logger = get_debug_logger();

        logger
            .debug(
                "CredentialManager",
                "Checking macOS Keychain for OAuth credentials",
            )
            .await;

        // Test override: simulate OAuth credentials present for deterministic testing
        if env::var(Self::TEST_OAUTH_PRESENT).unwrap_or_default() == "1" {
            logger
                .debug(
                    "CredentialManager",
                    "Test override: simulating OAuth credentials present",
                )
                .await;
            
            // Test override with configurable expiry
            let test_expires_at = env::var("CCSTATUS_TEST_OAUTH_EXPIRES_AT")
                .ok()
                .and_then(|s| s.parse::<i64>().ok());
            
            return Ok(Some(ApiCredentials {
                base_url: Self::OAUTH_FIXED_BASE_URL.to_string(),
                auth_token: Self::OAUTH_FIXED_TOKEN.to_string(),
                source: CredentialSource::OAuth,
                expires_at: test_expires_at,
            }));
        }

        // Check for OAuth env token (cross-platform signal)
        if let Ok(oauth_token) = env::var(Self::OAUTH_ENV_TOKEN) {
            if !oauth_token.is_empty() {
                logger
                    .debug(
                        "CredentialManager",
                        "OAuth env token present; selecting OAuth",
                    )
                    .await;
                
                // For env token testing, use configurable expiry
                let test_expires_at = env::var("CCSTATUS_TEST_OAUTH_EXPIRES_AT")
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok());
                    
                return Ok(Some(ApiCredentials {
                    base_url: Self::OAUTH_FIXED_BASE_URL.to_string(),
                    auth_token: Self::OAUTH_FIXED_TOKEN.to_string(),
                    source: CredentialSource::OAuth,
                    expires_at: test_expires_at,
                }));
            }
        }

        // Check if Claude Code credentials exist in Keychain
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new("security")
                .arg("find-generic-password")
                .arg("-s")
                .arg(Self::OAUTH_KEYCHAIN_SERVICE)
                .output()
        })
        .await;

        match output {
            Ok(Ok(result)) if result.status.success() => {
                logger
                    .debug(
                        "CredentialManager",
                        "Found OAuth credentials in macOS Keychain",
                    )
                    .await;

                // Parse actual OAuth credentials from Keychain
                self.parse_oauth_keychain_credentials(&logger).await
            }
            Ok(Ok(_)) => {
                logger
                    .debug(
                        "CredentialManager",
                        "No OAuth credentials found in macOS Keychain",
                    )
                    .await;
                Ok(None)
            }
            Ok(Err(e)) => {
                logger
                    .debug(
                        "CredentialManager",
                        &format!("Security command execution error: {}", e),
                    )
                    .await;
                Ok(None) // Fail silently and continue to next source
            }
            Err(e) => {
                logger
                    .debug(
                        "CredentialManager",
                        &format!("Keychain access error: {}", e),
                    )
                    .await;
                Ok(None) // Fail silently and continue to next source
            }
        }
    }

    /// Try to get credentials from OAuth (non-macOS: env token only)
    #[cfg(not(target_os = "macos"))]
    async fn get_from_oauth_keychain(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        use crate::core::network::debug_logger::get_debug_logger;
        let logger = get_debug_logger();

        // Test override: simulate OAuth credentials present for deterministic testing
        if env::var(Self::TEST_OAUTH_PRESENT).unwrap_or_default() == "1" {
            logger
                .debug(
                    "CredentialManager",
                    "Test override: simulating OAuth credentials present",
                )
                .await;
            
            // Test override with configurable expiry
            let test_expires_at = env::var("CCSTATUS_TEST_OAUTH_EXPIRES_AT")
                .ok()
                .and_then(|s| s.parse::<i64>().ok());
            
            return Ok(Some(ApiCredentials {
                base_url: Self::OAUTH_FIXED_BASE_URL.to_string(),
                auth_token: Self::OAUTH_FIXED_TOKEN.to_string(),
                source: CredentialSource::OAuth,
                expires_at: test_expires_at,
            }));
        }

        // Check for OAuth env token (cross-platform signal)
        if let Ok(oauth_token) = env::var(Self::OAUTH_ENV_TOKEN) {
            if !oauth_token.is_empty() {
                logger
                    .debug(
                        "CredentialManager",
                        "OAuth env token present; selecting OAuth",
                    )
                    .await;
                
                // For env token testing, use configurable expiry
                let test_expires_at = env::var("CCSTATUS_TEST_OAUTH_EXPIRES_AT")
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok());
                    
                return Ok(Some(ApiCredentials {
                    base_url: Self::OAUTH_FIXED_BASE_URL.to_string(),
                    auth_token: Self::OAUTH_FIXED_TOKEN.to_string(),
                    source: CredentialSource::OAuth,
                    expires_at: test_expires_at,
                }));
            }
        }

        // Non-macOS builds: no Keychain support
        Ok(None)
    }

    /// Try to get credentials from shell configuration files
    async fn get_from_shell_config(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        let shell_type = detect_shell();
        let config_paths = get_shell_config_paths(&shell_type)?;

        for config_path in &config_paths {
            if let Some(creds) = self
                .read_shell_credentials_from_file(&shell_type, config_path)
                .await?
            {
                return Ok(Some(creds));
            }
        }

        Ok(None)
    }

    /// Read credentials from a specific shell config file
    async fn read_shell_credentials_from_file(
        &self,
        shell_type: &ShellType,
        path: &PathBuf,
    ) -> Result<Option<ApiCredentials>, NetworkError> {
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path).await?;

        match shell_type {
            ShellType::Zsh | ShellType::Bash => self.parse_unix_shell_config(&content, path),
            ShellType::PowerShell => self.parse_powershell_config(&content, path),
            ShellType::Unknown => Ok(None),
        }
    }

    /// Parse bash/zsh config file for exported environment variables and function-based definitions
    fn parse_unix_shell_config(
        &self,
        content: &str,
        source_path: &Path,
    ) -> Result<Option<ApiCredentials>, NetworkError> {
        // Phase 1: Look for traditional export statements
        if let Some(creds) = self.parse_export_statements(content)? {
            return Ok(Some(ApiCredentials {
                base_url: creds.0,
                auth_token: creds.1,
                source: CredentialSource::ShellConfig(source_path.to_path_buf()),
                expires_at: None,
            }));
        }

        // Phase 2: Look for function-based variable definitions (like cc-env)
        if let Some(creds) = self.parse_function_variables(content)? {
            return Ok(Some(ApiCredentials {
                base_url: creds.0,
                auth_token: creds.1,
                source: CredentialSource::ShellConfig(source_path.to_path_buf()),
                expires_at: None,
            }));
        }

        // Phase 3: Look for non-exported assignments as fallback (VAR=value without export)
        if let Some(creds) = self.parse_variable_assignments(content)? {
            return Ok(Some(ApiCredentials {
                base_url: creds.0,
                auth_token: creds.1,
                source: CredentialSource::ShellConfig(source_path.to_path_buf()),
                expires_at: None,
            }));
        }

        Ok(None)
    }

    /// Parse traditional export statements with improved comment handling
    pub fn parse_export_statements(
        &self,
        content: &str,
    ) -> Result<Option<(String, String)>, NetworkError> {
        // Enhanced regex to match export statements with better value extraction
        // Matches: export VAR="value" or export VAR='value' or export VAR=value
        let export_regex = Regex::new(r#"^\s*export\s+([A-Z_]+)=(.*)"#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;

        // Use the common helper method (skip_export = false for export statements)
        self.process_shell_variables_with_regex(content, &export_regex, false)
    }

    /// Process shell variables using a regex pattern with common logic
    /// Returns (base_url, auth_token) if both are found, None otherwise
    fn process_shell_variables_with_regex(
        &self,
        content: &str,
        regex: &Regex,
        skip_export: bool,
    ) -> Result<Option<(String, String)>, NetworkError> {
        let mut base_url: Option<String> = None;
        let mut auth_token: Option<String> = None;

        for line in content.lines() {
            let trimmed_line = line.trim_start();

            // Skip comments
            if trimmed_line.starts_with('#') {
                continue;
            }

            // Skip export statements if requested (for assignment parsing)
            if skip_export && trimmed_line.starts_with("export ") {
                continue;
            }

            // Process regex matches
            if let Some(captures) = regex.captures(line) {
                let var_name = captures.get(1).map(|m| m.as_str());
                let raw_value = captures.get(2).map(|m| m.as_str()).unwrap_or("");

                // Extract the actual value, handling quotes and comments
                let var_value = self.extract_shell_value(raw_value);

                if !var_value.is_empty() {
                    process_anthropic_variable(var_name, var_value, &mut base_url, &mut auth_token);
                }
            }
        }

        // Check if we have complete credentials
        if let (Some(url), Some(token)) = (base_url, auth_token) {
            return Ok(Some((url, token)));
        }

        Ok(None)
    }

    /// Extract shell variable value handling quotes and trailing comments
    fn extract_shell_value(&self, raw_value: &str) -> String {
        let value = raw_value.trim();

        // Handle quoted values
        if value.starts_with('"') {
            // Double quoted - find the closing quote, handling escaped quotes
            if let Some(end_pos) = self.find_closing_quote(value, '"', 1) {
                return value[1..end_pos].to_string();
            }
        } else if let Some(stripped) = value.strip_prefix('\'') {
            // Single quoted - find the closing quote (no escaping in single quotes)
            if let Some(end_pos) = stripped.find('\'') {
                return stripped[..end_pos].to_string();
            }
        } else {
            // Unquoted value - take everything up to first unescaped whitespace or comment
            for (i, ch) in value.char_indices() {
                match ch {
                    ' ' | '\t' | '#' => {
                        // Check if this character is escaped
                        if i > 0 && value.chars().nth(i - 1) == Some('\\') {
                            continue;
                        }
                        return value[..i].trim().to_string();
                    }
                    _ => continue,
                }
            }
            return value.to_string();
        }

        // Fallback: return the whole value trimmed
        value.to_string()
    }

    /// Find the closing quote position, handling escaped quotes  
    fn find_closing_quote(&self, value: &str, quote_char: char, start: usize) -> Option<usize> {
        let chars: Vec<char> = value.chars().collect();
        let mut i = start;

        while i < chars.len() {
            if chars[i] == quote_char {
                // Check if this quote is escaped
                let mut escape_count = 0;
                let mut j = i;
                while j > 0 && chars[j - 1] == '\\' {
                    escape_count += 1;
                    j -= 1;
                }
                // If even number of escapes (including 0), the quote is not escaped
                if escape_count % 2 == 0 {
                    return Some(i);
                }
            }
            i += 1;
        }

        None
    }

    /// Parse function-based variable definitions (like cc-env pattern)
    pub fn parse_function_variables(
        &self,
        content: &str,
    ) -> Result<Option<(String, String)>, NetworkError> {
        // Regex to detect function definitions
        let function_regex =
            Regex::new(r#"^\s*(function\s+)?([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(\s*\)\s*\{"#)
                .map_err(|e| NetworkError::RegexError(e.to_string()))?;

        // Regex to detect array assignments within functions
        let array_start_regex = Regex::new(r#"^\s*local\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=\s*\("#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;

        // Regex to extract ANTHROPIC variables from array elements
        // Matches: "ANTHROPIC_BASE_URL=value" or 'ANTHROPIC_BASE_URL=value' or ANTHROPIC_BASE_URL=value
        let var_regex = Regex::new(r#"^\s*(["']?)(ANTHROPIC_(?:BASE_URL|BEDROCK_BASE_URL|VERTEX_BASE_URL|AUTH_TOKEN|API_KEY))=([^\n\r]+)"#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;

        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];

            // Skip comments
            if line.trim_start().starts_with('#') {
                i += 1;
                continue;
            }

            // Check if this line starts a function definition
            if function_regex.is_match(line) {
                i += 1;

                // Look for array assignments within this function
                while i < lines.len() {
                    let func_line = lines[i];

                    // Stop if we hit the end of function (closing brace at start of line)
                    if func_line.trim_start().starts_with('}') {
                        break;
                    }

                    // Check if this line starts an array assignment
                    if array_start_regex.is_match(func_line) {
                        i += 1;

                        // Parse array elements until we find closing parenthesis
                        let mut base_url: Option<String> = None;
                        let mut auth_token: Option<String> = None;

                        while i < lines.len() {
                            let array_line = lines[i];

                            // Stop if we hit closing parenthesis
                            if array_line.trim().starts_with(')') {
                                break;
                            }

                            // Check for ANTHROPIC variables in this array element
                            if let Some(captures) = var_regex.captures(array_line) {
                                let quote_char = captures.get(1).map(|m| m.as_str()).unwrap_or("");
                                let var_name = captures.get(2).map(|m| m.as_str());
                                let raw_value = captures.get(3).map(|m| m.as_str()).unwrap_or("");

                                // Remove matching quotes if present
                                let var_value =
                                    if !quote_char.is_empty() && raw_value.ends_with(quote_char) {
                                        raw_value.trim_end_matches(quote_char).to_string()
                                    } else {
                                        raw_value.to_string()
                                    };

                                process_anthropic_variable(
                                    var_name,
                                    var_value,
                                    &mut base_url,
                                    &mut auth_token,
                                );
                            }

                            i += 1;
                        }

                        // If we found both credentials in this array, return them
                        if let (Some(url), Some(token)) = (base_url, auth_token) {
                            return Ok(Some((url, token)));
                        }
                    }

                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        Ok(None)
    }

    /// Parse non-exported variable assignments (VAR=value without export) as fallback
    pub fn parse_variable_assignments(
        &self,
        content: &str,
    ) -> Result<Option<(String, String)>, NetworkError> {
        // Regex to match variable assignments without export
        // Matches: VAR="value" or VAR='value' or VAR=value (at start of line, not within export)
        let assignment_regex = Regex::new(r#"^\s*([A-Z_]+)=(.*)"#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;

        // Use the common helper method (skip_export = true to avoid processing export statements)
        self.process_shell_variables_with_regex(content, &assignment_regex, true)
    }

    /// Parse PowerShell config file for environment variables
    pub fn parse_powershell_config(
        &self,
        content: &str,
        source_path: &Path,
    ) -> Result<Option<ApiCredentials>, NetworkError> {
        // Regex for PowerShell environment variable setting
        // Matches: $env:VAR = "value" or [Environment]::SetEnvironmentVariable("VAR", "value", ...)
        let env_regex = Regex::new(r#"\$env:([A-Z_]+)\s*=\s*["']([^"']+)["']"#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;
        let setenv_regex = Regex::new(r#"\[.*Environment.*]::SetEnvironmentVariable\s*\(\s*["']([A-Z_]+)["']\s*,\s*["']([^"']+)["']"#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;

        let mut base_url: Option<String> = None;
        let mut auth_token: Option<String> = None;

        for line in content.lines() {
            // Skip comments
            if line.trim_start().starts_with('#') {
                continue;
            }

            // Check $env: syntax
            process_powershell_regex_captures(&env_regex, line, &mut base_url, &mut auth_token);

            // Check SetEnvironmentVariable syntax
            process_powershell_regex_captures(&setenv_regex, line, &mut base_url, &mut auth_token);
        }

        // Check if we have complete credentials
        if let (Some(url), Some(token)) = (base_url, auth_token) {
            return Ok(Some(ApiCredentials {
                base_url: url,
                auth_token: token,
                source: CredentialSource::ShellConfig(source_path.to_path_buf()),
                expires_at: None,
            }));
        }

        Ok(None)
    }

    /// Try to get credentials from Claude Code config file
    pub async fn get_from_claude_config(
        &self,
        config_path: &PathBuf,
    ) -> Result<Option<ApiCredentials>, NetworkError> {
        if !config_path.exists() {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(config_path).await?;

        let config: Value = serde_json::from_str(&content)?;

        // Try to extract credentials from Claude Code config
        // This matches the actual Claude Code config format
        if let (Some(base_url), Some(auth_token)) = (
            config.get("api_base_url").and_then(|v| v.as_str()),
            config.get("auth_token").and_then(|v| v.as_str()),
        ) {
            return Ok(Some(ApiCredentials {
                base_url: base_url.to_string(),
                auth_token: auth_token.to_string(),
                source: CredentialSource::ClaudeConfig(config_path.clone()),
                expires_at: None,
            }));
        }

        // Alternative config format - try different field names
        if let (Some(base_url), Some(auth_token)) = (
            config.get("base_url").and_then(|v| v.as_str()),
            config.get("auth_token").and_then(|v| v.as_str()),
        ) {
            return Ok(Some(ApiCredentials {
                base_url: base_url.to_string(),
                auth_token: auth_token.to_string(),
                source: CredentialSource::ClaudeConfig(config_path.clone()),
                expires_at: None,
            }));
        }

        Ok(None)
    }

    /// Parse OAuth credentials from macOS Keychain JSON
    #[cfg(target_os = "macos")]
    async fn parse_oauth_keychain_credentials(
        &self, 
        logger: &crate::core::network::debug_logger::EnhancedDebugLogger
    ) -> Result<Option<ApiCredentials>, NetworkError> {
        // Get the actual credentials from keychain with -w flag
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new("security")
                .arg("find-generic-password")
                .arg("-s")
                .arg(Self::OAUTH_KEYCHAIN_SERVICE)
                .arg("-w")  // Output password only
                .output()
        })
        .await;

        match output {
            Ok(Ok(result)) if result.status.success() => {
                let keychain_data = String::from_utf8(result.stdout)
                    .map_err(|e| NetworkError::CredentialError(format!("Keychain data not UTF-8: {}", e)))?
                    .trim()
                    .to_string();

                if keychain_data.is_empty() {
                    logger
                        .debug("CredentialManager", "Empty keychain data for OAuth credentials")
                        .await;
                    return Ok(None);
                }

                // Parse JSON from keychain
                let keychain_json: serde_json::Value = serde_json::from_str(&keychain_data)
                    .map_err(|e| NetworkError::CredentialError(format!("Invalid JSON in keychain: {}", e)))?;

                // Extract OAuth credentials
                let access_token = keychain_json
                    .get("claudeAiOauth")
                    .and_then(|oauth| oauth.get("accessToken"))
                    .and_then(|token| token.as_str())
                    .ok_or_else(|| NetworkError::CredentialError("Missing claudeAiOauth.accessToken in keychain".to_string()))?;

                // Extract expiry (optional)
                let expires_at = keychain_json
                    .get("claudeAiOauth")
                    .and_then(|oauth| oauth.get("expiresAt"))
                    .and_then(|exp| exp.as_i64());

                logger
                    .debug(
                        "CredentialManager",
                        &format!(
                            "Parsed OAuth credentials: token_length={} expires_at={}",
                            access_token.len(),
                            expires_at.map_or("none".to_string(), |exp| exp.to_string())
                        )
                    )
                    .await;

                Ok(Some(ApiCredentials {
                    base_url: Self::OAUTH_FIXED_BASE_URL.to_string(),
                    auth_token: access_token.to_string(),
                    source: CredentialSource::OAuth,
                    expires_at,
                }))
            }
            Ok(Ok(_)) => {
                logger
                    .debug("CredentialManager", "Keychain command failed for OAuth credentials")
                    .await;
                Ok(None)
            }
            Ok(Err(e)) => {
                logger
                    .debug(
                        "CredentialManager",
                        &format!("Security command execution error: {}", e)
                    )
                    .await;
                Ok(None)
            }
            Err(e) => {
                logger
                    .debug(
                        "CredentialManager",
                        &format!("Keychain access error: {}", e)
                    )
                    .await;
                Ok(None)
            }
        }
    }
}

// Private helper functions

/// Detect the current shell type based on environment and platform
pub fn detect_shell() -> ShellType {
    // Check SHELL environment variable first
    if let Ok(shell) = env::var("SHELL") {
        if shell.contains("zsh") {
            return ShellType::Zsh;
        } else if shell.contains("bash") {
            return ShellType::Bash;
        }
    }

    // Check for Windows
    if cfg!(target_os = "windows") {
        return ShellType::PowerShell;
    }

    // Default based on OS
    if cfg!(target_os = "macos") {
        ShellType::Zsh // macOS defaults to zsh
    } else if cfg!(target_os = "linux") {
        ShellType::Bash // Linux commonly uses bash
    } else {
        ShellType::Unknown
    }
}

/// Get configuration file paths based on shell type
pub fn get_shell_config_paths(shell_type: &ShellType) -> Result<Vec<PathBuf>, NetworkError> {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map_err(|_| NetworkError::HomeDirNotFound)?;

    let home_path = PathBuf::from(home);

    let paths = match shell_type {
        ShellType::Zsh => vec![
            home_path.join(".zshrc"),
            home_path.join(".zshenv"),
            home_path.join(".zprofile"),
        ],
        ShellType::Bash => vec![
            home_path.join(".bashrc"),
            home_path.join(".bash_profile"),
            home_path.join(".profile"),
        ],
        ShellType::PowerShell => {
            // PowerShell profiles on Windows
            let mut ps_paths = vec![];

            // User profile
            if let Ok(ps_home) = env::var("USERPROFILE") {
                let ps_home_path = PathBuf::from(ps_home);
                ps_paths.push(
                    ps_home_path
                        .join("Documents")
                        .join("WindowsPowerShell")
                        .join("Microsoft.PowerShell_profile.ps1"),
                );
                ps_paths.push(
                    ps_home_path
                        .join("Documents")
                        .join("PowerShell")
                        .join("Microsoft.PowerShell_profile.ps1"),
                );
            }

            ps_paths
        }
        ShellType::Unknown => vec![],
    };

    Ok(paths)
}

/// Helper function to process ANTHROPIC environment variables with priority handling
pub fn process_anthropic_variable(
    var_name: Option<&str>,
    var_value: String,
    base_url: &mut Option<String>,
    auth_token: &mut Option<String>,
) {
    match var_name {
        // Base URL priority: ANTHROPIC_BASE_URL > ANTHROPIC_BEDROCK_BASE_URL > ANTHROPIC_VERTEX_BASE_URL
        Some("ANTHROPIC_BASE_URL") => {
            *base_url = Some(var_value); // Highest priority, always set
        }
        Some("ANTHROPIC_BEDROCK_BASE_URL") => {
            if base_url.is_none() {
                // Only set if higher priority not already set
                *base_url = Some(var_value);
            }
        }
        Some("ANTHROPIC_VERTEX_BASE_URL") => {
            if base_url.is_none() {
                // Only set if higher priority not already set
                *base_url = Some(var_value);
            }
        }
        // Token priority: ANTHROPIC_AUTH_TOKEN > ANTHROPIC_API_KEY
        Some("ANTHROPIC_AUTH_TOKEN") => {
            *auth_token = Some(var_value); // Highest priority, always set
        }
        Some("ANTHROPIC_API_KEY") => {
            if auth_token.is_none() {
                // Only set if higher priority not already set
                *auth_token = Some(var_value);
            }
        }
        _ => {}
    }
}

/// Helper function to process regex captures for PowerShell environment variables
pub fn process_powershell_regex_captures(
    regex: &Regex,
    line: &str,
    base_url: &mut Option<String>,
    auth_token: &mut Option<String>,
) {
    if let Some(captures) = regex.captures(line) {
        let var_name = captures.get(1).map(|m| m.as_str());
        let var_value = captures.get(2).map(|m| m.as_str().to_string());

        if let Some(value) = var_value {
            process_anthropic_variable(var_name, value, base_url, auth_token);
        }
    }
}
