# @mauruppi/ccstatus

CCstatus - Claude Code Network Monitor and StatusLine tool

## Installation

```bash
npm install -g @mauruppi/ccstatus
```

## Features

- 🚀 **High Performance**: Written in Rust for maximum speed (<50ms startup, <10MB memory)
- 🌐 **Network Probing**: Driven by Claude code statusline stdin to initial endpoint status awareness ⚡
- 📊 **Git Integration**: Branch status, tracking info, and repository state display
- 🤖 **Model Display**: Simplified Claude model names (e.g., Sonnet 3.5, Sonnet 4)
- 📈 **Usage Tracking**: Context window analysis based on transcript files
- 🛡️ **Bot Fight Mitigation**: JS Challenge/Bot Fight detection and handling
- 📁 **Workspace Display**: Current working directory information
- 🌍 **Cross-platform**: Works on Windows, macOS, and Linux
- 📦 **Easy Installation**: One command via npm with automatic Claude Code setup
- ⚙️ **Smart Configuration**: Automatic credential detection and minimal setup

## Usage

After installation, ccstatus is automatically configured for Claude Code at `~/.claude/ccstatus/ccstatus`.

You can also use it directly:

```bash
ccstatus --help
ccstatus --version
```

## For Users in China

Use npm mirror for faster installation:

```bash
npm install -g @mauruppi/ccstatus --registry https://registry.npmmirror.com
```

## More Information

- GitHub: https://github.com/MaurUppi/CCstatus
- Issues: https://github.com/MaurUppi/CCstatus/issues
- License: MIT