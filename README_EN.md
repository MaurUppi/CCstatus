# CCstatus

![Language:Rust](https://img.shields.io/static/v1?label=Language&message=Rust&color=orange&style=flat-square)
![License:MIT](https://img.shields.io/static/v1?label=License&message=MIT&color=blue&style=flat-square)
![Changelog](https://img.shields.io/badge/Changelog-Keep%20a%20Changelog-brightgreen?style=flat-square)

[English](README_EN.md) | [中文](README.md)

## Motivation

- After reviewing many statusline projects, most focus on UI aesthetics rather than practical functionality.
- Anthropic Claude Code is arguably the best in its class, but it doesn't support mainland China, leading to projects like [CCR](https://github.com/musistudio/claude-code-router) and especially [Claude Relay Service](https://github.com/Wei-Shaw/claude-relay-service).
- This brings various issues, particularly network-related ones. The network path "User -> Anthropic" is complex, and it's often unclear what problems occur.
- Therefore, this project was developed to understand the health status of CRS and communication latency to Anthropic API along the path `User -> ··· -> CRS -> ··· -> Anthropic`.

- Known error scenarios:
    - [API Error](assets/API-error.png)
    - [Error Code](assets/CC-ErrorCode-0.png)

## Overview

- A high-performance Claude Code statusline tool written in Rust, integrating network probing, Git information, and real-time usage tracking.
- No background monitoring processes, driven solely by statusline `stdin` information, featuring `time window`-based `network probing` functionality.
- Aggregates JSONL logs from work projects, consolidating `error` information to clearly understand Claude Code error conditions.
- For more important information about `stdin, time windows, JSONL`, please refer to: [Q & A](qna-stdin-windows-jsonl.md)

## Important Notes

### Network Probing is **NOT** Monitoring

- The design relies on background processes for timed probing, so if Claude Code is open but idle, network data won't refresh during the designed window periods.
- Network conditions are dynamic (🟢/🟡/🔴), using P95 statistics calculated from aggregated 12 Total (end-to-end) data samples. For more details, check `ccstatus-monitoring.json`.
- When degraded/error occurs, detailed timing data is displayed (DNS|TCP|TLS|TTFB). TTFB is particularly important - it's the time from `sending model service request <--> Anthropic returns` the first byte.
- This tool can only provide information about which stage has issues; you need to investigate which component has the highest latency yourself.

### Areas for Improvement

- If you use Claude subscription instead of API key, and have **unofficial** BASE_URL/AUTH_TOKEN in `.zshrc, .bashrc`, the CRS health status icon will also display.
  - Reason: Credential priority design: `System environment [sys env] --> Environment files [.zshrc] --> JSON`, and I haven't found a way to identify `subscription` users yet.

## User Interface

- Standard display: ![CCstatus](assets/CCstatus.png)
- Degraded detailed info: ![CCstatus](assets/degraded.png)

```
Model | Working Directory | Git Branch Status | Context Window | Network Status
```

## Features

- **High performance** with Rust native speed
- **Git integration** with branch, status, and tracking info  
- **Model display** with simplified Claude model names
- **Usage tracking** based on transcript analysis
- **Network Probing**: Driven by Claude code statusline stdin to initial endpoint status awareness ⚡
- **Trying to resolve** JS Challenge/Bot Fight detection and countermeasures (LOW EXPECTATION) 🛡️
- **Directory display** showing current workspace
- **Minimal design** using Nerd Font icons
- **Simple configuration** via command line options
- **Modular features** with configurable build options

## Installation & Setup

### NPM Installation (Recommended)

- The easiest way to install CCstatus is via npm:

```bash
npm install -g @mauruppi/ccstatus
```

- Use mirror registry acceleration
```bash
npm install -g @mauruppi/ccstatus --registry https://registry.npmmirror.com
```

**Features:**
- ✅ **One-command installation** across all platforms
- ✅ **Automatic platform detection** (macOS Intel/ARM64, Linux x64, Windows x64)
- ✅ **Auto-setup for Claude Code** (installs to `~/.claude/ccstatus/`)
- ✅ **Static binaries** with zero dependencies
- ✅ **Easy updates** via `npm update -g @mauruppi/ccstatus`

After installation, the binary is automatically configured for Claude Code and ready to use.

### Manual Installation (Alternative)
#### [GitHub Releases](https://github.com/MaurUppi/CCstatus/releases)

<details><summary>Platform Deployment</summary>
<p>

#### Linux

#### Option 1: Dynamic Binary (Recommended)
```bash
mkdir -p ~/.claude/ccstatus
wget https://github.com/MaurUppi/CCstatus/releases/latest/download/ccstatus-linux-x64-static.tar.gz
tar -xzf ccstatus-linux-x64-static.tar.gz
cp ccstatus ~/.claude/ccstatus/CCstatus
chmod +x ~/.claude/ccstatus/CCstatus
```
*Requires: Ubuntu 22.04+, CentOS 9+, Debian 11+, RHEL 9+ (glibc 2.35+)*

#### Option 2: Static Binary (Universal Compatibility)
```bash
mkdir -p ~/.claude/ccstatus
wget https://github.com/MaurUppi/CCstatus/releases/latest/download/ccstatus-linux-x64-static.tar.gz
tar -xzf ccstatus-linux-x64-static.tar.gz
cp ccstatus ~/.claude/ccstatus/CCstatus
chmod +x ~/.claude/ccstatus/CCstatus
```
*Works on any Linux distribution (static, no dependencies)*

### macOS (Intel)

```bash  
mkdir -p ~/.claude/ccstatus
wget https://github.com/MaurUppi/CCstatus/releases/latest/download/ccstatus-macos-x64-static.tar.gz
tar -xzf ccstatus-macos-x64-static.tar.gz
cp ccstatus ~/.claude/ccstatus/CCstatus
chmod +x ~/.claude/ccstatus/CCstatus
```

### macOS (Apple Silicon)

```bash
mkdir -p ~/.claude/ccstatus  
wget https://github.com/MaurUppi/CCstatus/releases/latest/download/ccstatus-macos-arm64-static.tar.gz
tar -xzf ccstatus-macos-arm64-static.tar.gz
cp ccstatus ~/.claude/ccstatus/CCstatus
chmod +x ~/.claude/ccstatus/CCstatus
```

### Windows

```powershell
# Create directory and download
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.claude\ccstatus"
Invoke-WebRequest -Uri "https://github.com/MaurUppi/CCstatus/releases/latest/download/ccstatus-windows-x64-static.zip" -OutFile "ccstatus-windows-x64-static.zip"
Expand-Archive -Path "ccstatus-windows-x64-static.zip" -DestinationPath "."
Move-Item "ccstatus.exe" "$env:USERPROFILE\.claude\ccstatus\CCstatus.exe"
```

</p>
</details> 

### Claude Code Activation

**Linux/macOS:** `~/.claude/settings.json`
```json
{
  "statusLine": {
    "type": "command", 
    "command": "~/.claude/ccstatus/CCstatus",
    "padding": 0
  }
}
```

**Windows:** `C:\ProgramData\ClaudeCode\settings.json`
```json
{
  "statusLine": {
    "type": "command", 
    "command": "%USERPROFILE%\\.claude\\ccstatus\\CCstatus.exe",
    "padding": 0
  }
}
```

## Default Display

### Current Model

Shows simplified Claude model names:
- `claude-3-5-sonnet` → `Sonnet 3.5`
- `claude-4-sonnet` → `Sonnet 4`

### Working Directory
- Current project directory name

### Git Status Indicators

- Branch name with Nerd Font icon
- Status: `✓` Clean, `●` Dirty, `⚠` Conflicts  
- Remote tracking: `↑n` Ahead, `↓n` Behind

### Context Window Display

Token usage percentage based on transcript analysis with context limit tracking.

### Network Probing ⚡

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
- **Proxy Health Check**: Dedicated proxy health status monitoring module
  - Intelligent health status assessment: Healthy/Degraded/Bad/Unknown
  - Multi-URL probe strategy: primary endpoint + fallback endpoint
  - IF detected Official endpoint then skip proxy check to avoid redundancy
- **Bot Fight Intelligent Detection**: Bot challenge identification and mitigation 🛡️
  - **Multi-dimensional Detection**: HTTP status codes (403/429/503) + Cloudflare header analysis
  - **Shield Status Display**: Shows 🛡️ icon and total response time during bot challenges
  - **P95 Contamination Protection**: Bot challenge responses automatically excluded from performance statistics
  - **Secure Timing Suppression**: POST bot challenges don't display detailed timing breakdown
  - **HTTP Version Tracking**: Records HTTP/1.1 vs HTTP/2.0 protocol usage
- **Enhanced JSONL Logging**: Improved error information aggregation and analysis
- P95 latency tracking with rolling 12-sample window
- Frequency-gated probing to minimize API usage
- Debug logging with `CCSTATUS_DEBUG=true`
- State persistence across sessions

## Performance

- **Startup time**: < 50ms
- **Memory usage**: < 10MB 
- **Binary size**: 3.1 MB static build (network probing included)
- **Network overhead**: < 1 API call per 5 minutes (frequency-gated)
- **Monitoring latency**: Smart windowing minimizes impact on Claude API usage

## System Requirements

- **Claude Code**: For statusline integration

## Changelog

See changelog: [`CHANGELOG.md`](CHANGELOG.md)

<details><summary>Build from Source</summary>
<p>

- For detailed build configuration options, refer to [BUILD.md Build from Source section](README.md#build-from-source)
- Modify `Cargo.toml` as needed

```bash
git clone https://github.com/MaurUppi/CCstatus.git
cd CCstatus

# Default build (foundation + network probing **without timing display**)
cargo build --release

# Build (foundation + network probing **with timing display**)
cargo build --release --features timings-curl

# Optional: Add self-update feature
cargo build --release --features "self-update"
```

**Build Options:**
- **Default**: Core functionality + network probing (with timing display) (~3MB)
- **+ self-update**: Auto-update notifications (~4.1MB)

</p>
</details>

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Acknowledgments

- This project is based on comprehensive refactoring of Haleclipse's [CCometixLine](https://github.com/Haleclipse/CCometixLine)

## License

This project is licensed under the [MIT License](LICENSE).

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=MaurUppi/CCstatus&type=Date)](https://star-history.com/#MaurUppi/CCstatus&Date)