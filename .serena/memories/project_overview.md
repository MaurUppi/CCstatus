# CCstatus Project Overview

## Purpose
CCstatus (ccline) is a high-performance Claude Code statusline tool written in Rust with Git integration and real-time usage tracking. It provides a customizable status bar for Claude Code IDE showing model information, directory, git status, and context window usage.

## Key Features
- High performance Rust implementation with <50ms startup time
- Git integration with branch, status, and tracking info
- Model display with simplified Claude model names
- Usage tracking based on transcript analysis
- Directory display showing current workspace  
- Minimal design using Nerd Font icons
- TUI configuration interface
- Cross-platform support (Linux, macOS, Windows)

## Architecture
The project is organized into several main modules:
- **core**: Core functionality including statusline generation and segments
- **ui**: TUI interface for configuration with ratatui
- **config**: Configuration management and types
- **themes**: Theme system and presets
- **cli**: Command-line interface handling

## Installation
- Available via npm: `npm install -g @cometix/ccline`
- Manual installation from GitHub releases
- Build from source with Cargo

## Integration
Integrates with Claude Code via statusLine configuration in settings.json, executing as a command that reads JSON input from stdin and outputs formatted statusline text.