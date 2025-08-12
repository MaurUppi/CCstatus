# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2025-08-12

### Added
- Support for `total_tokens` field in token calculation for better accuracy with GLM-4.5 and similar providers
- Proper Git repository detection using `git rev-parse --git-dir`
- Cross-platform compatibility improvements for Windows path handling
- Pre-commit hooks for automatic code formatting

### Changed
- **Token calculation priority**: `total_tokens` → Claude format → OpenAI format → fallback
- **Display formatting**: Removed redundant ".0" from integer percentages and token counts
  - `0.0%` → `0%`, `25.0%` → `25%`, `50.0k` → `50k`
- **CI/CD**: Updated GitHub Actions to use Ubuntu 20.04 for better glibc compatibility
- **Version management**: Unified version number using `env!("CARGO_PKG_VERSION")`
- **Author**: Updated package author to "Haleclipse"

### Fixed
- Git segment now properly hides for non-Git directories instead of showing misleading "detached" status
- Windows Git repository path handling issues by removing overly aggressive path sanitization
- glibc compatibility issues on older Linux systems (now supports Ubuntu 18.04+, CentOS 8+, Debian 10+)

### Removed
- Path sanitization function that could break Windows paths in Git operations

## [0.1.0] - 2025-08-11

### Added
- Initial release of CCometixLine
- High-performance Rust-based statusline tool for Claude Code
- Git integration with branch, status, and tracking info
- Model display with simplified Claude model names
- Usage tracking based on transcript analysis
- Directory display showing current workspace
- Minimal design using Nerd Font icons
- Cross-platform support (Linux, macOS, Windows)
- Command-line configuration options
- GitHub Actions CI/CD pipeline

### Technical Details
- Context limit: 200,000 tokens
- Startup time: < 50ms
- Memory usage: < 10MB
- Binary size: ~2MB optimized release build

