# Credential Manager Module

## Overview

The Credential Manager is a comprehensive, cross-platform credential resolution system that intelligently retrieves API authentication credentials from multiple sources. It implements a priority-based resolution hierarchy designed for real-world development environments and production deployments.

## Core Functionality

### Priority-Based Resolution System

The credential manager implements a 3-tier priority hierarchy:

1. **Environment Variables** (Highest Priority)
   - `ANTHROPIC_BASE_URL` + `ANTHROPIC_AUTH_TOKEN`
   - Immediate availability, ideal for CI/CD and containerized environments

2. **Shell Configuration Files** (Medium Priority)  
   - Cross-platform shell parsing (Bash, Zsh, PowerShell)
   - Advanced parsing of export statements and function-based variables
   - Developer workflow integration

3. **Claude Code Configuration Files** (Lowest Priority)
   - JSON-based configuration files
   - Support for global and project-local settings
   - Fallback configuration management

### Cross-Platform Shell Support

#### Unix Shells (Bash/Zsh)
- **Export Statements**: `export ANTHROPIC_BASE_URL="https://api.anthropic.com"`
- **Function-Based Variables**: Parses `cc-env()` functions with local variable arrays
- **Configuration Files**: `.zshrc`, `.bashrc`, `.zshenv`, `.bash_profile`, `.profile`

#### PowerShell (Windows)
- **Environment Syntax**: `$env:ANTHROPIC_BASE_URL = "https://api.anthropic.com"`
- **Method Syntax**: `[Environment]::SetEnvironmentVariable("VAR", "value")`
- **Profile Locations**: User and system PowerShell profiles

## Integration Interface

### Primary Integration Pattern

```rust
use crate::core::segments::network::credential::CredentialManager;

// Standard usage
let credential_manager = CredentialManager::new()?;
if let Some(creds) = credential_manager.get_credentials().await? {
    println!("API Endpoint: {}", creds.base_url);
    println!("Credential Source: {:?}", creds.source);
    // Use creds.auth_token for authentication
}
```

### Public API Methods

#### Core Methods

- **`CredentialManager::new()`** → `Result<Self, NetworkError>`
  - Auto-configures Claude config file paths
  - Platform-independent initialization
  - No external dependencies required

- **`get_credentials()`** → `Result<Option<ApiCredentials>, NetworkError>`
  - Primary entry point implementing full priority hierarchy
  - Async operation with comprehensive debug logging
  - Returns `None` if no credentials found (fail-silent behavior)

#### Granular Control Methods

- **`get_from_environment()`** → `Result<Option<ApiCredentials>, NetworkError>`
  - Direct environment variable resolution
  - Immediate execution (not async)
  - Useful for testing or custom logic

- **`parse_export_statements(content: &str)`** → `Result<Option<(String, String)>, NetworkError>`
  - Parses traditional shell export statements
  - Handles multiple quote types and whitespace variations
  - Returns (base_url, auth_token) tuple

- **`parse_function_variables(content: &str)`** → `Result<Option<(String, String)>, NetworkError>`
  - Advanced parsing of function-based variable definitions
  - Supports complex shell patterns like `cc-env()` functions
  - Multi-phase parsing with array detection

- **`parse_powershell_config(content: &str, source_path: &Path)`** → `Result<Option<ApiCredentials>, NetworkError>`
  - PowerShell-specific configuration parsing
  - Supports both `$env:` and `SetEnvironmentVariable` syntax
  - Returns full `ApiCredentials` with source tracking

- **`get_from_claude_config(config_path: &PathBuf)`** → `Result<Option<ApiCredentials>, NetworkError>`
  - JSON configuration file parsing
  - Supports multiple field name conventions
  - Async file I/O with proper error handling

#### Utility Functions

- **`detect_shell()`** → `ShellType`
  - Automatic shell type detection
  - OS-specific defaults with environment override
  - Cross-platform compatibility

- **`get_shell_config_paths(shell_type: &ShellType)`** → `Result<Vec<PathBuf>, NetworkError>`
  - Shell-specific configuration file path resolution
  - Platform-aware path construction
  - Multiple file location support

## Data Structures

### ApiCredentials
```rust
pub struct ApiCredentials {
    pub base_url: String,      // API endpoint URL
    pub auth_token: String,    // Authentication token
    pub source: CredentialSource, // Source tracking for audit/debug
}
```

### CredentialSource
```rust
pub enum CredentialSource {
    Environment,                    // Environment variables
    ShellConfig(PathBuf),          // Shell config file path
    ClaudeConfig(PathBuf),         // Claude config file path
}
```

### ShellType
```rust
pub enum ShellType {
    Zsh,           // Z shell (default on macOS)
    Bash,          // Bourne Again Shell (default on Linux)
    PowerShell,    // Microsoft PowerShell (default on Windows)
    Unknown,       // Unsupported or undetected shell
}
```

## Configuration Examples

### Environment Variables
```bash
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
export ANTHROPIC_AUTH_TOKEN="sk-your-token-here"
```

### Shell Configuration (.zshrc/.bashrc)
```bash
# Traditional export statements
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
export ANTHROPIC_AUTH_TOKEN="sk-your-token-here"

# Function-based configuration
function cc-env() {
    local env_vars=(
        "ANTHROPIC_BASE_URL=https://api.anthropic.com"
        "ANTHROPIC_AUTH_TOKEN=sk-your-token-here"
    )
    for var in "${env_vars[@]}"; do
        export "$var"
    done
}
```

### PowerShell Profile
```powershell
# Environment variable syntax
$env:ANTHROPIC_BASE_URL = "https://api.anthropic.com"
$env:ANTHROPIC_AUTH_TOKEN = "sk-your-token-here"

# Method syntax
[Environment]::SetEnvironmentVariable("ANTHROPIC_BASE_URL", "https://api.anthropic.com", "User")
[Environment]::SetEnvironmentVariable("ANTHROPIC_AUTH_TOKEN", "sk-your-token-here", "User")
```

### Claude Code Configuration (JSON)
```json
{
  "ANTHROPIC_BASE_URL": "https://api.anthropic.com",
  "ANTHROPIC_AUTH_TOKEN": "sk-your-token-here"
}
```

## Error Handling

The module uses a comprehensive error handling strategy:

- **`NetworkError`** enum for specific error categorization
- **Graceful degradation**: Failed sources don't prevent checking other sources  
- **Option-based returns**: `None` indicates "no credentials found" vs error conditions
- **Async error propagation**: Proper error context preservation across async boundaries

### Common Error Scenarios
- **`HomeDirNotFound`**: Cannot locate user home directory
- **`ConfigReadError`**: File I/O failures during config reading
- **`ConfigParseError`**: JSON parsing failures in Claude config files
- **`RegexError`**: Regex compilation failures (should not occur in production)

## Security Considerations

### Built-in Security Features
- **Fail-Silent Behavior**: Missing credentials return `None` rather than exposing system information
- **No Credential Logging**: Debug logging shows credential lengths, never actual values
- **Input Sanitization**: Regex patterns prevent injection attacks
- **Priority Override**: Environment variables allow secure credential override

### Best Practices for Integration
1. **Environment Priority**: Use environment variables for production deployments
2. **Source Tracking**: Leverage `CredentialSource` for audit logging
3. **Error Handling**: Always handle `NetworkError` cases appropriately
4. **Async Patterns**: Use proper async/await for non-blocking credential resolution

## Integration Patterns

### Standard Web Service Integration
```rust
let credential_manager = CredentialManager::new()?;
if let Some(creds) = credential_manager.get_credentials().await? {
    let client = HttpClient::new(&creds.base_url, &creds.auth_token)?;
    // Proceed with API calls
} else {
    return Err("No API credentials configured".into());
}
```

### Testing and Development
```rust
// Test environment variable resolution specifically
let creds = credential_manager.get_from_environment()?;

// Test shell config parsing with custom content  
let shell_creds = credential_manager.parse_export_statements(&config_content)?;

// Custom configuration path testing
let config_creds = credential_manager.get_from_claude_config(&custom_path).await?;
```

### CI/CD Integration
```yaml
# Environment variables take highest priority
env:
  ANTHROPIC_BASE_URL: "https://api.anthropic.com"
  ANTHROPIC_AUTH_TOKEN: ${{ secrets.ANTHROPIC_TOKEN }}
```

## Performance Characteristics

- **Lazy Loading**: Configuration files read only when needed
- **Async I/O**: Non-blocking file operations for shell and Claude configs
- **Minimal Overhead**: Environment variable checking is immediate
- **Caching**: No caching implemented - credential resolution per call
- **Memory Efficient**: Minimal memory footprint with string-based credentials

## Thread Safety

The credential manager is thread-safe with the following considerations:
- **Immutable Configuration**: No shared mutable state
- **Environment Variables**: Read-only access to process environment
- **File I/O**: Independent file operations per instance
- **Async Safe**: Compatible with multi-threaded async runtimes

## Dependencies

### Core Dependencies
- **`std::env`**: Environment variable access
- **`std::path`**: Path manipulation and validation
- **`serde_json`**: JSON parsing for Claude config files
- **`regex`**: Pattern matching for shell configuration parsing
- **`tokio::fs`**: Async file I/O operations

### Internal Dependencies
- **`crate::core::segments::network::types`**: Core data structures
- **`crate::core::segments::network::debug_logger`**: Debug logging integration

## Module History

This module represents a unified credential management system that combines:
- **CredentialManager**: Priority-based credential resolution
- **ShellConfigReader**: Advanced shell configuration parsing
- **Cross-platform support**: Native shell integration across operating systems

The design emphasizes real-world usability, security, and integration flexibility while maintaining a clean, documented API surface.