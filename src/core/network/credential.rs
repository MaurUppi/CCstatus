//! Unified credential management for network monitoring
//! 
//! This module provides comprehensive credential resolution from multiple sources including
//! environment variables, shell configuration files, and Claude Code configuration files.
//! It implements a priority-based credential resolution system with cross-platform support
//! for extracting API credentials from various shell types (Bash, Zsh, PowerShell).
//!
//! ## Architecture
//! 
//! The module combines functionality from CredentialManager and ShellConfigReader into a 
//! single cohesive unit. The main entry point is `CredentialManager` which orchestrates
//! credential resolution using this priority hierarchy:
//!
//! 1. Environment variables (highest priority)
//! 2. Shell configuration files (.zshrc, .bashrc, PowerShell profiles)
//! 3. Claude Code configuration files (lowest priority)
//!
//! ## Shell Parsing Support
//!
//! - **Bash/Zsh**: Export statements and function-based variable definitions
//! - **PowerShell**: $env: syntax and [Environment]::SetEnvironmentVariable calls
//! - **Cross-platform**: Auto-detection with OS-specific defaults
//!
//! ## Usage
//!
//! ```rust
//! use crate::core::segments::network::credential::CredentialManager;
//!
//! let credential_manager = CredentialManager::new()?;
//! if let Some(creds) = credential_manager.get_credentials().await? {
//!     println!("Found credentials from: {:?}", creds.source);
//! }
//! ```

use std::env;
use std::path::{Path, PathBuf};
use serde_json::Value;
use regex::Regex;
use tokio::fs;

use crate::core::segments::network::types::{ApiCredentials, CredentialSource, NetworkError};

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
        
        Ok(Self { claude_config_paths })
    }
    
    /// Get credentials from environment, shell config, or Claude config files
    /// 
    /// Implements priority hierarchy:
    /// 1. Environment variables (ANTHROPIC_BASE_URL + ANTHROPIC_AUTH_TOKEN)
    /// 2. Shell configuration files (.zshrc, .bashrc, PowerShell profiles)
    /// 3. Claude Code config files
    /// 4. None (fail silent)
    pub async fn get_credentials(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        use crate::core::segments::network::debug_logger::get_debug_logger;
        let logger = get_debug_logger();
        
        logger.debug("CredentialManager", "Starting credential lookup from all sources").await;
        
        // Priority 1: Environment variables
        logger.debug("CredentialManager", "Checking environment variables...").await;
        let env_result = self.get_from_environment();
        logger.debug("CredentialManager", &format!("Environment check completed: {}", env_result.is_ok())).await;
        
        match env_result {
            Ok(Some(creds)) => {
                logger.debug("CredentialManager", &format!("Found env credentials: endpoint={}, token_len={}", creds.base_url, creds.auth_token.len())).await;
                return Ok(Some(creds));
            },
            Ok(None) => {
                logger.debug("CredentialManager", "Environment variables not set (ANTHROPIC_BASE_URL or ANTHROPIC_AUTH_TOKEN missing)").await;
            },
            Err(e) => {
                logger.debug("CredentialManager", &format!("Environment variable error: {}", e)).await;
                return Err(e);
            }
        }
        
        // Priority 2: Shell configuration files
        logger.debug("CredentialManager", "About to check shell config files...").await;
        
        match self.get_from_shell_config().await {
            Ok(Some(creds)) => {
                logger.debug("CredentialManager", &format!("Found shell credentials: endpoint={}, token_len={}", creds.base_url, creds.auth_token.len())).await;
                return Ok(Some(creds));
            },
            Ok(None) => {
                logger.debug("CredentialManager", "No credentials found in shell configs").await;
            },
            Err(e) => {
                logger.debug("CredentialManager", &format!("Shell config credential error: {}", e)).await;
            }
        }
        
        logger.debug("CredentialManager", "Shell config check completed, no credentials found").await;
        
        // Priority 3: Claude Code config files
        logger.debug("CredentialManager", "About to check Claude config files...").await;
        
        for (index, config_path) in self.claude_config_paths.iter().enumerate() {
            logger.debug("CredentialManager", &format!("Checking config file #{}: {}", index + 1, config_path.display())).await;
            
            match self.get_from_claude_config(config_path).await {
                Ok(Some(creds)) => {
                    logger.debug("CredentialManager", &format!("Found config credentials in file #{}: endpoint={}, token_len={}", index + 1, creds.base_url, creds.auth_token.len())).await;
                    return Ok(Some(creds));
                },
                Ok(None) => {
                    logger.debug("CredentialManager", &format!("Config file #{} exists but has no credentials (file: {})", index + 1, config_path.display())).await;
                },
                Err(e) => {
                    logger.debug("CredentialManager", &format!("Config file #{} error: {} (file: {})", index + 1, e, config_path.display())).await;
                }
            }
        }
        logger.debug("CredentialManager", "All Claude config files checked, no credentials found").await;
        
        // No credentials found
        logger.error("CredentialManager", "FINAL RESULT: No credentials found in any source (env, shell, or config files)").await;
        Ok(None)
    }
    
    /// Try to get credentials from environment variables
    pub fn get_from_environment(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        // Check for base URL + token combination
        if let (Ok(base_url), Ok(auth_token)) = (
            env::var("ANTHROPIC_BASE_URL"),
            env::var("ANTHROPIC_AUTH_TOKEN")
        ) {
            return Ok(Some(ApiCredentials {
                base_url,
                auth_token,
                source: CredentialSource::Environment,
            }));
        }
        
        Ok(None)
    }
    
    /// Try to get credentials from shell configuration files
    async fn get_from_shell_config(&self) -> Result<Option<ApiCredentials>, NetworkError> {
        let shell_type = detect_shell();
        let config_paths = get_shell_config_paths(&shell_type)?;
        
        for config_path in &config_paths {
            if let Some(creds) = self.read_shell_credentials_from_file(&shell_type, config_path).await? {
                return Ok(Some(creds));
            }
        }
        
        Ok(None)
    }
    
    /// Read credentials from a specific shell config file
    async fn read_shell_credentials_from_file(
        &self, 
        shell_type: &ShellType, 
        path: &PathBuf
    ) -> Result<Option<ApiCredentials>, NetworkError> {
        if !path.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(path).await
?;
        
        match shell_type {
            ShellType::Zsh | ShellType::Bash => {
                self.parse_unix_shell_config(&content, path)
            },
            ShellType::PowerShell => {
                self.parse_powershell_config(&content, path)
            },
            ShellType::Unknown => Ok(None),
        }
    }
    
    /// Parse bash/zsh config file for exported environment variables and function-based definitions
    fn parse_unix_shell_config(&self, content: &str, source_path: &Path) -> Result<Option<ApiCredentials>, NetworkError> {
        // Phase 1: Look for traditional export statements
        if let Some(creds) = self.parse_export_statements(content)? {
            return Ok(Some(ApiCredentials {
                base_url: creds.0,
                auth_token: creds.1,
                source: CredentialSource::ShellConfig(source_path.to_path_buf()),
            }));
        }
        
        // Phase 2: Look for function-based variable definitions (like cc-env)
        if let Some(creds) = self.parse_function_variables(content)? {
            return Ok(Some(ApiCredentials {
                base_url: creds.0,
                auth_token: creds.1,
                source: CredentialSource::ShellConfig(source_path.to_path_buf()),
            }));
        }
        
        Ok(None)
    }
    
    /// Parse traditional export statements
    pub fn parse_export_statements(&self, content: &str) -> Result<Option<(String, String)>, NetworkError> {
        // Enhanced regex to match export statements
        // Matches: export VAR="value" or export VAR='value' or export VAR=value
        let export_regex = Regex::new(r#"^\s*export\s+([A-Z_]+)=(["']?)([^\n\r]+)"#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;
        
        let mut base_url: Option<String> = None;
        let mut auth_token: Option<String> = None;
        
        for line in content.lines() {
            // Skip comments
            if line.trim_start().starts_with('#') {
                continue;
            }
            
            if let Some(captures) = export_regex.captures(line) {
                let var_name = captures.get(1).map(|m| m.as_str());
                let quote_char = captures.get(2).map(|m| m.as_str()).unwrap_or("");
                let raw_value = captures.get(3).map(|m| m.as_str()).unwrap_or("");
                
                // Remove matching quotes if present
                let var_value = if !quote_char.is_empty() && raw_value.ends_with(quote_char) {
                    raw_value.trim_end_matches(quote_char).to_string()
                } else {
                    raw_value.to_string()
                };
                
                process_anthropic_variable(var_name, var_value, &mut base_url, &mut auth_token);
            }
        }
        
        // Check if we have complete credentials
        if let (Some(url), Some(token)) = (base_url, auth_token) {
            return Ok(Some((url, token)));
        }
        
        Ok(None)
    }
    
    /// Parse function-based variable definitions (like cc-env pattern)
    pub fn parse_function_variables(&self, content: &str) -> Result<Option<(String, String)>, NetworkError> {
        // Regex to detect function definitions
        let function_regex = Regex::new(r#"^\s*(function\s+)?([a-zA-Z_][a-zA-Z0-9_-]*)\s*\(\s*\)\s*\{"#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;
        
        // Regex to detect array assignments within functions
        let array_start_regex = Regex::new(r#"^\s*local\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=\s*\("#)
            .map_err(|e| NetworkError::RegexError(e.to_string()))?;
        
        // Regex to extract ANTHROPIC variables from array elements
        // Matches: "ANTHROPIC_BASE_URL=value" or 'ANTHROPIC_BASE_URL=value' or ANTHROPIC_BASE_URL=value
        let var_regex = Regex::new(r#"^\s*(["']?)(ANTHROPIC_(?:BASE_URL|AUTH_TOKEN))=([^\n\r]+)"#)
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
                                let var_value = if !quote_char.is_empty() && raw_value.ends_with(quote_char) {
                                    raw_value.trim_end_matches(quote_char).to_string()
                                } else {
                                    raw_value.to_string()
                                };
                                
                                process_anthropic_variable(var_name, var_value, &mut base_url, &mut auth_token);
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
    
    /// Parse PowerShell config file for environment variables
    pub fn parse_powershell_config(&self, content: &str, source_path: &Path) -> Result<Option<ApiCredentials>, NetworkError> {
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
            }));
        }
        
        Ok(None)
    }
    
    /// Try to get credentials from Claude Code config file
    pub async fn get_from_claude_config(
        &self, 
        config_path: &PathBuf
    ) -> Result<Option<ApiCredentials>, NetworkError> {
        if !config_path.exists() {
            return Ok(None);
        }
        
        let content = tokio::fs::read_to_string(config_path).await
?;
        
        let config: Value = serde_json::from_str(&content)?;
        
        // Try to extract credentials from Claude Code config
        // This matches the actual Claude Code config format
        if let (Some(base_url), Some(auth_token)) = (
            config.get("api_base_url").and_then(|v| v.as_str()),
            config.get("auth_token").and_then(|v| v.as_str())
        ) {
            return Ok(Some(ApiCredentials {
                base_url: base_url.to_string(),
                auth_token: auth_token.to_string(),
                source: CredentialSource::ClaudeConfig(config_path.clone()),
            }));
        }
        
        // Alternative config format - try different field names
        if let (Some(base_url), Some(auth_token)) = (
            config.get("base_url").and_then(|v| v.as_str()),
            config.get("auth_token").and_then(|v| v.as_str())
        ) {
            return Ok(Some(ApiCredentials {
                base_url: base_url.to_string(),
                auth_token: auth_token.to_string(),
                source: CredentialSource::ClaudeConfig(config_path.clone()),
            }));
        }
        
        Ok(None)
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
        ShellType::Zsh  // macOS defaults to zsh
    } else if cfg!(target_os = "linux") {
        ShellType::Bash  // Linux commonly uses bash
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
                ps_paths.push(ps_home_path.join("Documents")
                    .join("WindowsPowerShell")
                    .join("Microsoft.PowerShell_profile.ps1"));
                ps_paths.push(ps_home_path.join("Documents")
                    .join("PowerShell")
                    .join("Microsoft.PowerShell_profile.ps1"));
            }
            
            ps_paths
        },
        ShellType::Unknown => vec![],
    };
    
    Ok(paths)
}

/// Helper function to process ANTHROPIC environment variables
pub fn process_anthropic_variable(
    var_name: Option<&str>,
    var_value: String,
    base_url: &mut Option<String>,
    auth_token: &mut Option<String>,
) {
    match var_name {
        Some("ANTHROPIC_BASE_URL") => {
            *base_url = Some(var_value);
        },
        Some("ANTHROPIC_AUTH_TOKEN") => {
            *auth_token = Some(var_value);
        },
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
