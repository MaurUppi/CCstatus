# Tech Stack and Dependencies

## Language
- **Rust 2021 Edition** - Main programming language for performance and safety

## Core Dependencies
- **serde** (1.0) with derive features - JSON serialization/deserialization
- **serde_json** (1.0) - JSON processing for Claude Code input data
- **clap** (4.0) with derive features - Command-line argument parsing
- **toml** (0.8) - Configuration file format

## TUI Dependencies (optional feature)
- **ratatui** (0.29) - Terminal user interface framework
- **crossterm** (0.28) - Cross-platform terminal manipulation
- **ansi_term** (0.12) - ANSI color and style handling
- **ansi-to-tui** (7.0) - Convert ANSI sequences to TUI styles

## Self-Update Dependencies (optional feature)
- **ureq** (2.10) with json features - HTTP client for updates
- **semver** (1.0) - Semantic versioning
- **chrono** (0.4) with serde features - Date/time handling
- **dirs** (5.0) - Platform-specific directories

## Features
- **default**: ["tui", "self-update"]
- **tui**: Enables terminal UI configuration interface
- **self-update**: Enables automatic update checking

## Build System
- **Cargo** - Rust's build system and package manager
- **GitHub Actions** - CI/CD pipeline for testing and releases
- Cross-compilation for multiple platforms (Linux, macOS, Windows)