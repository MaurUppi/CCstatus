# CCometixLine

[English](README.md) | [中文](README.zh.md)

A high-performance Claude Code statusline tool written in Rust with Git integration and real-time usage tracking.

![Language:Rust](https://img.shields.io/static/v1?label=Language&message=Rust&color=orange&style=flat-square)
![License:MIT](https://img.shields.io/static/v1?label=License&message=MIT&color=blue&style=flat-square)

## Screenshots

![CCometixLine](assets/img1.png)

The statusline shows: Model | Directory | Git Branch Status | Context Window | Network Status

## Features

- **High performance** with Rust native speed
- **Git integration** with branch, status, and tracking info  
- **Model display** with simplified Claude model names
- **Usage tracking** based on transcript analysis
- **Network monitoring** with real-time Claude API connectivity status ⚡
- **Directory display** showing current workspace
- **Minimal design** using Nerd Font icons
- **Simple configuration** via command line options
- **Modular features** with configurable build options

## Installation

### Quick Install (Recommended)

Install via npm (works on all platforms):

```bash
# Install globally
npm install -g @cometix/ccline

# Or using yarn
yarn global add @cometix/ccline

# Or using pnpm
pnpm add -g @cometix/ccline
```

Use npm mirror for faster download:
```bash
npm install -g @cometix/ccline --registry https://registry.npmmirror.com
```

After installation:
- ✅ Global command `ccline` is available everywhere  
- ✅ Automatically configured for Claude Code at `~/.claude/ccline/ccline`
- ✅ Ready to use immediately!

### Update

```bash
npm update -g @cometix/ccline
```

### Manual Installation

Alternatively, download from [Releases](https://github.com/Haleclipse/CCometixLine/releases):

#### Linux

#### Option 1: Dynamic Binary (Recommended)
```bash
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-linux-x64.tar.gz
tar -xzf ccline-linux-x64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```
*Requires: Ubuntu 22.04+, CentOS 9+, Debian 11+, RHEL 9+ (glibc 2.35+)*

#### Option 2: Static Binary (Universal Compatibility)
```bash
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-linux-x64-static.tar.gz
tar -xzf ccline-linux-x64-static.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```
*Works on any Linux distribution (static, no dependencies)*

### macOS (Intel)

```bash  
mkdir -p ~/.claude/ccline
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-x64.tar.gz
tar -xzf ccline-macos-x64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

### macOS (Apple Silicon)

```bash
mkdir -p ~/.claude/ccline  
wget https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-macos-arm64.tar.gz
tar -xzf ccline-macos-arm64.tar.gz
cp ccline ~/.claude/ccline/
chmod +x ~/.claude/ccline/ccline
```

### Windows

```powershell
# Create directory and download
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\ccline"
Invoke-WebRequest -Uri "https://github.com/Haleclipse/CCometixLine/releases/latest/download/ccline-windows-x64.zip" -OutFile "ccline-windows-x64.zip"
Expand-Archive -Path "ccline-windows-x64.zip" -DestinationPath "."
Move-Item "ccline.exe" "$env:USERPROFILE\.claude\ccline\"
```

### Claude Code Configuration

Add to your Claude Code `settings.json`:

**Linux/macOS:**
```json
{
  "statusLine": {
    "type": "command", 
    "command": "~/.claude/ccline/ccline",
    "padding": 0
  }
}
```

**Windows:**
```json
{
  "statusLine": {
    "type": "command", 
    "command": "%USERPROFILE%\\.claude\\ccline\\ccline.exe",
    "padding": 0
  }
}
```

### Build from Source

```bash
git clone https://github.com/Haleclipse/CCometixLine.git
cd CCometixLine

# Default build (foundation + network monitoring)
cargo build --release

# Optional: Add self-update feature
cargo build --release --features "self-update"

# Optional: Add TUI configuration interface  
cargo build --release --features "tui"

# Full build (all features)
cargo build --release --features "tui,self-update"

# Linux/macOS
mkdir -p ~/.claude/ccline
cp target/release/ccstatus ~/.claude/ccline/ccline
chmod +x ~/.claude/ccline/ccline

# Windows (PowerShell)
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\ccline"
copy target\release\ccstatus.exe "$env:USERPROFILE\.claude\ccline\ccline.exe"
```

**Build Options:**
- **Default**: Core functionality + network monitoring (~1.8MB)
- **+ self-update**: Auto-update notifications (~4.1MB) 
- **+ tui**: Configuration interface (~2.5MB)
- **Full**: All features (~4.8MB)

See [BUILD-CONFIG.md](BUILD-CONFIG.md) for detailed build configuration options.

## Usage

```bash
# Basic usage (displays all enabled segments)
ccline

# Show help
ccline --help

# Print default configuration  
ccline --print-config

# TUI configuration mode (planned)
ccline --configure
```

## Default Segments

Displays: `Directory | Git Branch Status | Model | Context Window | Network Status`

### Git Status Indicators

- Branch name with Nerd Font icon
- Status: `✓` Clean, `●` Dirty, `⚠` Conflicts  
- Remote tracking: `↑n` Ahead, `↓n` Behind

### Model Display

Shows simplified Claude model names:
- `claude-3-5-sonnet` → `Sonnet 3.5`
- `claude-4-sonnet` → `Sonnet 4`

### Context Window Display

Token usage percentage based on transcript analysis with context limit tracking.

### Network Monitoring ⚡

**Real-time Claude API connectivity monitoring:**
- 🟢 **Healthy**: API responding normally (P95 < 4s)
- 🟡 **Degraded**: Slower responses or rate limits (P95 4-8s) 
- 🔴 **Error**: Connection issues or API failures
- ⚪ **Unknown**: Monitoring disabled or no credentials

**Smart monitoring windows:**
- **COLD**: Immediate check on startup or session changes
- **GREEN**: Regular health checks every 5 minutes during active use
- **RED**: Error-triggered checks when transcript shows API errors

**Features:**
- Automatic credential detection (environment, shell, Claude config)
- P95 latency tracking with rolling 12-sample window
- Frequency-gated probing to minimize API usage
- Debug logging with `CCSTATUS_DEBUG=true`
- State persistence across sessions

## Configuration

Configuration support is planned for future releases. Currently uses sensible defaults for all segments.

## Performance

- **Startup time**: < 50ms (vs ~200ms for TypeScript equivalents)
- **Memory usage**: < 10MB (vs ~25MB for Node.js tools)
- **Binary size**: 1.8MB default build (network monitoring included)
- **Network overhead**: < 1 API call per 5 minutes (frequency-gated)
- **Monitoring latency**: Smart windowing minimizes impact on Claude API usage

## Requirements

- **Git**: Version 1.5+ (Git 2.22+ recommended for better branch detection)
- **Terminal**: Must support Nerd Fonts for proper icon display
  - Install a [Nerd Font](https://www.nerdfonts.com/) (e.g., FiraCode Nerd Font, JetBrains Mono Nerd Font)
  - Configure your terminal to use the Nerd Font
- **Claude Code**: For statusline integration

## Development

```bash
# Build development version
cargo build

# Run tests
cargo test

# Build optimized release
cargo build --release
```

## Roadmap

- [ ] TOML configuration file support
- [ ] TUI configuration interface
- [ ] Custom themes
- [ ] Plugin system
- [ ] Cross-platform binaries

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

This project is licensed under the [MIT License](LICENSE).

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=Haleclipse/CCometixLine&type=Date)](https://star-history.com/#Haleclipse/CCometixLine&Date)