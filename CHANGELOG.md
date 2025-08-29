# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.2.1] - 2025-08-29

### 🛡️ Bot Fight 反击系统

#### 🚨 威胁检测与防护
- **智能 Bot 挑战识别**: 多维度检测 Bot 防护系统触发
  - **HTTP 状态码检测**: 精确识别 `403/429/503` 防护响应
  - **Cloudflare 头部分析**: 检测 `cf-ray`, `cf-cache-status`, `server: cloudflare` 等标识
  - **POST 429 增强检测**: 特别优化 POST 请求的 CF 防护识别逻辑
  - **多层验证**: 状态码 + 头部组合验证，减少误报率

#### 🛡️ 盾牌渲染系统
- **直观状态显示**: 使用 🛡️ emoji 直接展示 Bot 防护状态
- **智能信息呈现**: Bot 挑战时显示总响应时间而非详细分解
- **安全优先设计**: POST Bot 挑战自动抑制详细计时分解（防止时序攻击分析）
- **8 种边缘情况覆盖**: 全面测试各种 Bot 挑战场景的盾牌渲染逻辑

#### ⚡ P95 污染防护
- **性能指标保护**: Bot 挑战响应自动从 P95 延迟统计中排除
- **监控数据纯净性**: 确保网络健康指标不被防护系统影响
- **智能分离**: 区分正常 API 延迟和防护触发延迟
- **统计准确性**: 维护真实网络性能基线数据

#### 🔍 HTTP 版本持久化
- **协议版本追踪**: 记录每次请求使用的 HTTP 协议版本
- **诊断信息增强**: HTTP/1.1 vs HTTP/2.0 使用情况统计
- **兼容性监控**: 协助识别协议相关的连接问题
- **性能分析**: 支持基于 HTTP 版本的性能对比分析

### 🧪 全面测试覆盖

#### 🛡️ Bot Fight 测试套件
- **盾牌渲染测试**: 8 个边缘情况验证 Bot 挑战显示逻辑
- **POST 抑制验证**: 确认 POST Bot 挑战正确抑制详细时间分解
- **状态组合测试**: 验证 Bot 挑战与代理健康状态的交互
- **时间格式测试**: 确认盾牌状态下的时间显示格式正确性

#### 🏗️ Mock 架构重构
- **URL 路由系统**: 从 LIFO 栈模式升级为基于 URL 的路由分发
- **方法区分**: `MockHttpMethod` 枚举支持 GET/POST 请求区分
- **响应队列**: 每个路由维护独立的响应队列，防止串扰
- **测试隔离**: 彻底解决 mock 响应被意外消费的问题

### 🔧 代码质量提升

#### 📦 重复代码消除
- **JSON 验证重构**: 提取 `determine_bad_health_reason()` 辅助函数
- **代码维护性**: 替换 `proxy_health/checker.rs` 中的重复 JSON 解析逻辑
- **一致性改进**: 统一错误原因判定逻辑 (`invalid_json_200` vs `unknown_schema_200`)

#### 🏛️ 架构优化
- **5 元组返回**: `execute_http_probe` 返回包含 HTTP 版本的完整探测信息
- **类型系统增强**: `ProbeMetrics` 和 `NetworkMetrics` 添加 `http_version` 字段
- **向后兼容**: 保持现有 API 接口稳定，新增字段为可选类型

### 🛡️ 安全性考量

#### 🔒 防时序攻击
- **POST 挑战保护**: POST Bot 挑战时不显示详细时间分解
- **信息泄露防护**: 避免通过计时信息推断系统内部状态
- **渐进式披露**: 仅在必要时显示详细时间信息

#### 🎯 精准识别
- **误报控制**: 多层验证机制减少 Bot 挑战误判
- **真阳性保证**: 确保真实 Bot 防护触发被正确识别
- **边界情况处理**: 处理各种异常响应格式和头部组合

---

## [2.1.0] - 2025-08-28

### 🏗️ 架构重大改进

#### 🔄 代理健康检查模块化分离
- **独立代理健康监控模块**: 从 HttpMonitor 中分离出专门的 `proxy_health` 模块
  - **专业化架构**: 独立的健康检查客户端和评估逻辑
  - **智能状态评估**: 支持 健康/降级/故障/未知 四种状态
  - **多 URL 探测策略**: 主要端点 (`/api/health`) + 备用端点 (`/health`)
  - **官方端点检测**: 自动识别 Anthropic 官方端点，跳过代理检查避免冗余
  - **容错设计**: 支持响应解析失败时的降级处理
- **模块化结构**: 清晰的关注点分离
  - `checker.rs`: 核心评估逻辑和状态映射
  - `client.rs`: HTTP 客户端抽象和 Mock 支持  
  - `parsing.rs`: JSON 响应解析和模式检测
  - `url.rs`: URL 构建和验证逻辑
  - `config.rs`: 配置选项和默认值

#### 🧹 代码质量提升
- **消除重复代码**: 重构 `ProxyHealthOutcome` 构建逻辑
  - 创建 `build_outcome_with_response()` 和 `build_outcome_no_response()` 辅助函数
  - 替换 6 处重复的结构体构建模式
  - 提高代码可维护性和一致性
- **改进 COLD 窗口逻辑**: 恢复为原始的纯时间设计
  - 修复测试失败：从混合状态/时间触发逻辑回归为 `total_duration_ms < COLD_WINDOW_MS`
  - 保持原始设计意图的时间窗口概念
  - 提高时间窗口行为的可预测性

#### 🧪 全面测试覆盖
- **25 个代理健康检查测试**: 涵盖所有组件和场景
  - **单元测试**: checker, parsing, url 模块独立测试  
  - **集成测试**: 端到端工作流验证
  - **边缘情况**: 网络失败、无效响应、超时处理
  - **状态映射**: 验证所有健康状态的正确转换
- **网络监控测试修复**: 修复所有失败的测试用例
  - 修复 3 个 status_renderer_tests (typo 修正)
  - 修复 3 个 jsonl_monitor_tests (NBSP 字符清理)  
  - 修复 10 个 network_segment_tests (COLD 窗口逻辑)
- **测试覆盖完整性**: 确保新功能的可靠性和回归防护

### 🔧 开发工具增强
- **改进 .gitignore 配置**: 确保本地开发文件不被推送
  - 添加 `project/` 目录排除（文档和开发文件）
  - 添加 `.serena/` 目录排除（MCP 服务器内存文件）
  - 保持代码库整洁，仅跟踪必要文件

### 📊 向后兼容性
- **无破坏性变更**: 所有现有功能保持不变
- **API 兼容性**: HttpMonitor 接口保持稳定
- **配置兼容性**: 用户无需修改现有配置
- **性能优化**: 模块化设计提高了代码组织但不影响运行时性能

### 🛡️ 质量保证
- **架构清晰性**: 关注点分离提高代码可读性
- **可测试性**: 独立模块便于单元测试和集成测试  
- **可维护性**: 减少重复代码，提高修改效率
- **稳定性**: 全面的测试覆盖确保系统稳定性

---

## [2.0.2] - 2025-08-28

### 🔧 构建系统重大改进

#### 🍎 macOS ARM64 静态链接修复
- **解决 dyld OpenSSL 路径问题**: 修复 CI 构建的 macOS ARM64 二进制文件运行时 OpenSSL 动态库路径错误
  - 问题：CI 构建查找 `/usr/local/opt/openssl` (Intel 路径)，ARM64 实际在 `/opt/homebrew/opt/openssl`
  - 解决方案：启用静态链接 `OPENSSL_STATIC=1` 和 `OPENSSL_NO_VENDOR=1`
  - 更新 `isahc` 依赖使用 `static-curl` 特性消除运行时依赖
- **双版本构建策略**: 实现静态版本和轻量版本并行构建
  - **Static**: `timings-curl-static` (~7MB) - 零依赖，通用兼容性
  - **Slim**: `network-monitoring` (~3MB) - 需要系统 OpenSSL 3.x
  - macOS 平台同时提供两种版本供用户选择

#### 📦 发布流程优化
- **清晰的版本标识**: 所有发布包名称包含 `-static` 或 `-slim` 后缀
- **详细发布说明**: 每个 release 包含构建差异说明和使用场景指导
- **跨平台兼容性**: Linux/Windows 保持静态构建，macOS 提供双选择
- **安装指南更新**: README 中所有平台安装命令更新为 `-static` 版本

#### 🧹 代码库清理
- **修复 .gitignore 违规**: 移除 `.idea/`, `.serena/`, `project/`, `claude*.md` 等被误提交文件
- **防止未来违规**: 加强 `.gitignore` 规则，添加系统文件排除模式
- **保持测试文件跟踪**: 确认 `tests/` 目录应被版本控制追踪

### 📚 文档完善
- **BUILD.md 全面更新**: 反映新的双版本构建策略和平台特定说明
- **CI 配置文档化**: 提供实际 CI 矩阵配置而非示例代码
- **平台说明优化**: 明确各平台推荐构建变体和依赖要求

### 🛡️ 质量保证
- **构建验证**: 所有平台构建通过 `cargo check` 验证
- **零破坏性变更**: 现有用户工作流程无需修改
- **向后兼容**: 保持所有现有功能和配置格式

---

## [2.0.0] - 2025-08-27

### ⚡ 重大架构升级

#### 🏗️ 双日志架构实现
- **始终开启的 JSONL 运营日志**: 不再依赖 `CCSTATUS_DEBUG` 设置
  - 独立的运营数据记录：`~/.claude/ccstatus/ccstatus-jsonl-error.json`
  - 自动敏感信息脱敏，防止 API 密钥泄露
  - 内置日志轮转和 gzip 压缩，防止磁盘占用
- **可选调试日志**: `CCSTATUS_DEBUG=true` 时启用详细调试信息
  - 平文本格式，便于人工排查问题
  - 独立的文件路径和轮转策略
- **修复归档清理逻辑**: 解决文件命名不匹配导致的磁盘空间问题

#### 🚀 功能完善
- **全面集成测试**: 验证 JSONL 始终开启行为
- **代码清理**: 移除已废弃的 `jsonl_error_summary` 方法
- **文档更新**: 反映严格的布尔值解析规则（仅支持 true/false）

### 🛡️ 安全性提升
- **防御性脱敏**: JSONL 消息在写入前自动过滤敏感信息
- **原子文件操作**: 临时文件 + 重命名模式确保数据完整性

### 📊 向后兼容性
- 现有用户配置无需修改
- 新增 JSONL 日志文件，但不影响现有功能
- 平滑升级路径，无破坏性变更

---

## [1.3.0] - 2025-08-26

### 🔧 模块化构建系统
- **特性标志扩展**: 支持 `timings-curl-static` 构建选项
  - 静态 curl 库集成，消除运行时依赖
  - 专为 Windows/Linux 可移植部署优化
  - 自动降级：curl 失败时回退到 isahc 启发式计时
- **CI/CD 优化**: 更新发布流程支持静态库构建
- **BUILD.md 文档**: 完整的构建选项和平台说明

---

## [1.2.0] - 2025-08-25

### 📊 高精度性能监控
- **HTTP 阶段计时**: 基于 libcurl 的详细网络性能测量
  - DNS 解析时间独立统计
  - TCP 连接建立时间测量
  - TLS 握手时间分析
  - TTFB（首字节时间）精确计算
  - 总响应时间完整追踪
- **性能指标优化**: 改进延迟统计和 P95 计算准确性
- **依赖项精简**: 优化网络监控相关依赖包大小

---

## [1.1.0] - 2025-08-24

### 🚀 网络健康检查系统
- **HealthCheckClient 架构**: 专业级代理健康状态监控
  - 支持多种探测模式：COLD（启动）、GREEN（定期）、RED（错误触发）
  - 四种状态指示：🟢 健康、🟡 降级、🔴 错误、⚪ 未知
  - 集成测试覆盖，确保监控准确性
- **HttpMonitor 增强**: 全面的 HTTP 探测能力
  - 原子状态持久化，防止数据竞争
  - 窗口去重机制，避免重复探测
  - 会话跟踪，支持 COLD 探测去重
- **错误恢复**: 增强的故障处理和自动恢复机制

---

## [1.0.6] - 2025-08-23

### 📋 监控组件完善
- **JsonlMonitor 增强**: UTF-8 安全处理和模式匹配改进
  - 全面支持 Unicode 内容解析
  - 改进的 API 错误检测算法
  - 结构化日志记录和全面测试覆盖
- **ErrorTracker 升级**: 智能错误分类和安全性改进
  - 基于 HTTP 状态码的精确错误映射
  - 支持 `isApiErrorMessage` 标志检测
  - 模式匹配备用检测，提高容错能力
- **DebugLogger 优化**: 结构化日志记录和轮转机制

---

## [1.0.5] - 2025-08-22

### ⚡ 网络监控基础框架
- **NetworkSegment 协调器**: stdin 触发的窗口化探测系统
  - 智能监控窗口：COLD（启动检测）、GREEN（5分钟定期）、RED（错误触发）
  - 频率控制探测，最小化对 Claude API 的影响
  - P95 延迟追踪，12样本滚动窗口性能分析
- **凭证管理**: 自动检测环境变量、shell 和 Claude 配置
- **状态线集成**: 修复缺失组件，实现双行布局支持
- **基础测试框架**: 网络监控组件单元测试

---

## [1.0.4] - 2025-08-21

### Added
- **Network Monitoring Feature ⚡**: Real-time Claude API connectivity status monitoring
  - Smart monitoring windows: COLD (startup), GREEN (regular 5min), RED (error-triggered)
  - Four status indicators: 🟢 Healthy, 🟡 Degraded, 🔴 Error, ⚪ Unknown
  - P95 latency tracking with rolling 12-sample window for performance analysis
  - Automatic credential detection from environment, shell, and Claude config
  - Frequency-gated probing to minimize API usage impact
  - State persistence across sessions with atomic file operations
  - Debug logging support with `CCSTATUS_DEBUG=true`
- **Modular Build System**: Configurable feature flags for optimized builds
  - Default build: Foundation + network monitoring (~1.8MB)
  - Optional features: `tui` (configuration interface), `self-update` (auto-updates)
  - Full feature matrix with size optimization for different use cases
  - BUILD-CONFIG.md documentation for detailed build options

### Changed
- **Default Features**: Updated from `["tui", "network-monitoring"]` to `["network-monitoring"]`
  - Reduced default binary size from 2.6MB to 1.8MB (30% smaller)
  - TUI and self-update features now optional, configurable at build time
  - Maintains backward compatibility for users wanting full features
- **Build Architecture**: Enhanced conditional compilation support
  - Added feature gates throughout codebase for clean modular builds  
  - Improved error handling for missing features
  - Updated documentation to reflect new build options
- **Binary Name**: Updated from `ccometixline` to `ccstatus` for consistency
  - Reflects the project's focus on statusline and monitoring functionality
  - Updated all build scripts and documentation accordingly

### Fixed
- **ureq v3.1.0 Compatibility**: Updated HTTP client API usage
  - Fixed breaking changes in ureq v3.1.0 JSON response handling
  - Changed from deprecated `.json()` to `.body_mut().read_json()` pattern
  - Ensured self-update feature works correctly with new API
- **NetworkSegment Enhancements**: Implemented priority-based monitoring windows
  - Eliminated JSONL double-scan in RED path for better performance
  - Added per-window deduplication with monotonic window ID tracking
  - Enhanced COLD probe semantics with proper session tracking
  - Implemented atomic state persistence with temp file + rename pattern

### Technical Details
- **Monitoring Windows**: 
  - COLD: `startup_duration < 5000ms` OR new session_id
  - GREEN: `(total_duration_ms % 300_000) < 3_000` (5min intervals)
  - RED: `(total_duration_ms % 10_000) < 1_000` AND transcript has API errors
- **API Endpoints**: `/v1/messages` for health checking with minimal payload
- **Error Classification**: Comprehensive HTTP status code mapping to user-friendly states
- **Performance Optimizations**: Single-probe guarantee per stdin event
- **State Management**: JSON state file with versioning and migration support

## [1.0.3] - 2025-08-17

### Fixed
- **TUI Preview Display**: Complete redesign of preview system for cross-platform reliability
  - Replaced environment-dependent segment collection with pure mock data generation
  - Fixed Git segment not showing in preview on Windows and Linux systems
  - Ensures consistent preview display across all supported platforms
- **Documentation Accuracy**: Corrected CLI parameter reference from `--interactive` to `--config`
  - Fixed changelog and documentation to reflect actual CLI parameters
- **Preview Data Quality**: Enhanced mock data to better represent actual usage
  - Usage segment now displays proper format: "78.2% · 156.4k"
  - Update segment displays dynamic version number from Cargo.toml
  - All segments show realistic and informative preview data

### Changed
- **Preview Architecture**: Complete rewrite of preview component for better maintainability
  - Removed dependency on real file system and Git repository detection
  - Implemented `generate_mock_segments_data()` for environment-independent previews
  - Simplified code structure and improved performance
  - Preview now works reliably in any environment without external dependencies

### Technical Details
- Environment-independent mock data generation for all segment types
- Dynamic version display using `env!("CARGO_PKG_VERSION")`
- Optimized preview rendering without file system calls or Git operations
- Consistent cross-platform display: "Sonnet 4 | CCstatus | main ✓ | 78.2% · 156.4k"

## [1.0.2] - 2025-08-17

### Fixed
- **Windows PowerShell Compatibility**: Fixed double key event triggering in TUI interface
  - Resolved issue #18 where keystrokes were registered twice on Windows PowerShell
  - Added proper KeyEventKind filtering to only process key press events
  - Maintained cross-platform compatibility with Unix/Linux/macOS systems

### Technical Details
- Import KeyEventKind from crossterm::event module  
- Filter out KeyUp events to prevent double triggering on Windows Console API
- Uses efficient continue statement to skip non-press events
- No impact on existing behavior on Unix-based systems

## [1.0.1] - 2025-08-17

### Fixed
- NPM package publishing workflow compatibility issues
- Cargo.lock version synchronization with package version
- GitHub Actions release pipeline for NPM distribution

### Changed
- Enhanced npm postinstall script with improved binary lookup for different package managers
- Better error handling and user feedback in installation process
- Improved cross-platform compatibility for npm package installation

### Technical
- Updated dependency versions (bitflags, proc-macro2)
- Resolved NPM version conflict preventing 1.0.0 re-publication
- Ensured proper version alignment across all distribution channels

## [1.0.0] - 2025-08-16

### Added
- **Interactive TUI Mode**: Full-featured terminal user interface with ratatui
  - Real-time statusline preview while editing configuration
  - Live theme switching with instant visual feedback
  - Intuitive keyboard navigation (Tab, Escape, Enter, Arrow keys)
  - Comprehensive help system with context-sensitive guidance
- **Comprehensive Theme System**: Modular theme architecture with multiple presets
  - Default, Minimal, Powerline, Compact themes included
  - Custom color schemes and icon sets
  - Theme validation and error reporting
  - Powerline theme importer for external theme compatibility
- **Enhanced Configuration System**: Robust config management with validation
  - TOML-based configuration with schema validation
  - Dynamic config loading with intelligent defaults
  - Interactive mode support and theme selection
  - Configuration error handling and user feedback
- **Advanced Segment System**: Modular statusline segments with improved functionality
  - Enhanced Git segment with stash detection and conflict status
  - Model segment with simplified display names for Claude models
  - Directory segment with customizable display options
  - Usage segment with better token calculation accuracy
  - Update segment for version management and notifications
- **CLI Interface Enhancements**: Improved command-line experience
  - `--config` flag for launching TUI configuration mode
  - Enhanced argument parsing with better error messages
  - Theme selection via command line options
  - Comprehensive help and version information

### Changed
- **Architecture**: Complete modularization of codebase for better maintainability
  - Separated core logic from presentation layer
  - Improved error handling throughout all modules
  - Better separation of concerns between data and UI
- **Dependencies**: Added TUI and terminal handling capabilities
  - ratatui for terminal user interface components
  - crossterm for cross-platform terminal manipulation
  - ansi_term and ansi-to-tui for color processing
- **Configuration**: Enhanced config structure for theme and TUI mode support
  - Expanded config types to support new features
  - Improved validation and default value handling
  - Better error messages for configuration issues

### Technical Improvements
- **Performance**: Optimized statusline generation and rendering
- **Code Quality**: Comprehensive refactoring with improved error handling
- **User Experience**: Intuitive interface design with immediate visual feedback
- **Extensibility**: Modular architecture allows easy addition of new themes and segments

### Breaking Changes
- Configuration file format has been extended (backward compatible for basic usage)
- Some internal APIs have been restructured for better modularity
- Minimum supported features now include optional TUI dependencies

## [0.1.1] - 2025-08-12

### Added
- Support for `total_tokens` field in token calculation for better accuracy with GLM-4.5 and similar providers
- Proper Git repository detection using `git rev-parse --git-dir`
- Cross-platform compatibility improvements for Windows path handling
- Pre-commit hooks for automatic code formatting
- **Static Linux binary**: Added musl-based static binary for universal Linux compatibility without glibc dependencies

### Changed
- **Token calculation priority**: `total_tokens` → Claude format → OpenAI format → fallback
- **Display formatting**: Removed redundant ".0" from integer percentages and token counts
  - `0.0%` → `0%`, `25.0%` → `25%`, `50.0k` → `50k`
- **CI/CD**: Updated GitHub Actions to use Ubuntu 22.04 for Linux builds and ubuntu-latest for Windows cross-compilation
- **Binary distribution**: Now provides two Linux options - dynamic (glibc) and static (musl) binaries
- **Version management**: Unified version number using `env!("CARGO_PKG_VERSION")`

### Fixed
- Git segment now properly hides for non-Git directories instead of showing misleading "detached" status
- Windows Git repository path handling issues by removing overly aggressive path sanitization
- GitHub Actions runner compatibility issues (updated to supported versions: ubuntu-22.04 for Linux, ubuntu-latest for Windows)
- **Git version compatibility**: Added fallback to `git symbolic-ref` for Git versions < 2.22 when `--show-current` is not available

### Removed
- Path sanitization function that could break Windows paths in Git operations

## [0.1.0] - 2025-08-11

### Added
- Initial release of CCstatus
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

