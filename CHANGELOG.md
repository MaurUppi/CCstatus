# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.2.6] - 2025-09-06

### 🔧 OAuth Masquerade System Implementation

#### ✨ OAuth Masquerade Core Features
- **OAuth Masquerade Mode**: Complete OAuth environment support with first-party request simulation
  - **First-party Request Masquerade**: OAuth tokens sent as Authorization headers to `https://api.anthropic.com`, masquerading as direct client requests
  - **Token Expiry Validation**: Automatic token expiration checking with `expires_at` millisecond precision to prevent invalid requests
  - **Cross-platform Support**: Via `CLAUDE_CODE_OAUTH_TOKEN` environment variable and macOS Keychain integration
  - **Secure Debug Logging**: Logs token length rather than full token to protect sensitive information
  - **Green Status Indicator**: Displays green emoji (🟢) with timing metrics, omits proxy health prefix in OAuth mode
  - **Smart Rendering**: Format `🟢 P95:...ms DNS:...|TCP:...|TLS:...|Total:... HTTP/x`

#### ⚡ Advanced Timing Features  
- **curl Phase Timings Support**: Enhanced OAuth masquerade with detailed timing breakdown when `timings-curl` feature is enabled
  - **DNS|TCP|TLS|TTFB|Total Breakdown**: Provides comprehensive timing analysis for OAuth requests
  - **Feature-gated Implementation**: Dual function signatures for timings-curl enabled/disabled configurations
  - **isahc Fallback**: Automatic fallback to isahc for resilience when curl is unavailable
  - **Header Conversion**: HashMap to Vec format conversion for curl compatibility

#### 🛡️ Security & Quality Enhancements
- **Duplicate Code Elimination**: Comprehensive refactoring to remove 132+ lines of duplicate code
  - **Token Expiration Helper**: Extracted `check_token_expiry_with_logging()` to eliminate 60 lines of duplicate token validation
  - **Debug Logging Helpers**: Created `log_entry_decision()` and `log_request_construction()` to eliminate 72 lines of duplicate logging
  - **Code Quality**: Improved maintainability through centralized helper functions
- **Linter Warning Fixes**: Resolved Rust code style warnings for improved code quality
  - **Control Flow Optimization**: Extracted `return` keyword from if-else and match expressions
  - **Path Prefix Cleanup**: Removed unnecessary `serde_json::` prefixes where `Value` is already imported
  - **Idiomatic Rust**: Improved code readability following Rust best practices

#### 🏗️ Implementation Details
- **HttpMonitor Enhancement**: Modified `process_probe_results()` to conditionally skip proxy health assessment based on credential source
- **StatusRenderer Upgrade**: Added `render_oauth_metrics()` method for OAuth-specific display with green status indicator
- **API Compatibility**: Updated all `render_status()` callers to pass `api_config` parameter for source detection
- **Type System**: Added `PartialEq` derive to `CredentialSource` enum for OAuth comparison support

#### 🧪 Comprehensive Test Coverage
- **OAuth Proxy Skip Tests**: Validates proxy health bypass with panic client (ensures no calls in OAuth mode)
- **OAuth Rendering Tests**: Confirms green status indicator display and proxy health omission in OAuth mode
- **Non-OAuth Compatibility**: Verifies existing behavior unchanged for environment/shell/config sources
- **Edge Case Coverage**: Tests OAuth mode with minimal metrics and fallback scenarios

#### 🛡️ Quality Assurance
- **Compilation Verified**: All code compiles without errors after signature updates
- **Test Suite**: New OAuth-specific tests pass while maintaining existing test compatibility
- **Code Quality**: Formatted with `cargo fmt` and linted with `cargo clippy`
- **Backward Compatibility**: Non-OAuth modes function identically to previous versions

---

## [2.2.5] - 2025-09-01

### 🔧 Network Monitoring Breakdown Format Enhancement

#### ✨ Degraded Performance Detection
- **P80 Latency Check**: Enhanced breakdown format condition to include network performance degradation
  - **Smart Format Switching**: Shows `ServerTTFB/TotalTTFB` when network latency exceeds P80 threshold
  - **Timings-Curl Compliance**: Fixed `timings-curl` feature to comply with degraded status logic  
  - **Better Diagnostics**: Detailed timing breakdowns now appear for degraded network performance (🟡), not just HTTP errors
  - **Performance Aware**: Breakdown format now responds to both HTTP status codes AND network performance metrics

#### ⚙️ Monitoring System Improvements  
- **GREEN Window Optimization**: Widened capture window from 3s to 10s for better monitoring coverage
- **Update Check Enhancement**: Reduced threshold from 12 to 6 GREEN ticks (~30min cycle)
- **Time-Based Fallback**: Added 30-minute maximum age for update checks to ensure timely notifications

#### 🛡️ Quality Assurance
- **Enhanced Testing**: Verified breakdown format shows detailed timings when network degrades
- **Fix Validation**: Confirmed P80 latency condition properly triggers enhanced breakdown display
- **Performance Monitoring**: Ensured timings-curl feature works correctly with degraded status detection

## [2.2.4] - 2025-08-31

### 📊 JSONL Monitor Phase 2 Enhancements

#### ✨ Advanced Error Deduplication System
- **SHA256-based Deduplication**: Intelligent deduplication using session_id, timestamp, and HTTP status code
  - **60-second Sliding Window**: Prevents duplicate error entries within 60 seconds
  - **Smart Key Generation**: `sha256(session_id|occurred_at|status_code)` for precise deduplication
  - **Memory Efficient**: LRU-style cache with automatic cleanup of expired entries
  - **High Performance**: Deduplication check averages <1ms per entry

#### 🏗️ Enhanced Field Schema & Naming
- **Improved Field Naming**: More descriptive field names for better log analysis
  - `logged_at`: When the error was written to JSONL (RFC3339 format)
  - `occurred_at`: When the error actually happened (from transcript)
  - `type`: Error classification (`isApiErrorMessage` vs `fallback`)
  - `code_source`: Detection method (`explicit`, `parsed`, or `none`)
- **RFC3339 Timestamp Normalization**: Automatic timestamp validation and correction
  - **Placeholder Detection**: Identifies test placeholders like `2024-01-01T12:*:00Z`
  - **Fallback to Local Time**: Invalid timestamps replaced with current local time
  - **Consistent Format**: All timestamps guaranteed to be valid RFC3339

#### 🧪 Test Infrastructure Improvements  
- **Serial Test Execution**: Added `serial_test` crate to prevent environment variable race conditions
  - **Environment Isolation**: Tests no longer interfere with each other via CCSTATUS_DEBUG
  - **Deterministic Results**: Eliminates flaky test failures from concurrent execution
  - **Comprehensive Coverage**: All JSONL monitor tests now use `#[serial]` annotation

#### ⚙️ Configuration-Based Dependency Injection
- **JsonlLoggerConfig Structure**: Explicit configuration for improved testability
  - **Configurable Paths**: Custom JSONL and debug log file locations
  - **Debug Control**: Programmatic debug logging enable/disable
  - **Test-Friendly**: Supports temporary directories and mock configurations
- **Enhanced Logger Creation**: `from_config()` method for dependency injection pattern
  - **Explicit Configuration**: No more reliance on implicit environment detection
  - **Better Testing**: Allows precise control over logger behavior in tests
  - **Maintainability**: Clear separation between configuration and implementation

#### 🔧 Code Quality & Architecture
- **SHA256 Dependency**: Added `sha2 = "0.10"` for cryptographic hashing
- **Enhanced Error Parsing**: Improved `parse_jsonl_line_enhanced()` method
  - **Better Detection**: More accurate API error message identification
  - **Structured Output**: Returns tuple with error entry, dedup key, and detection type
  - **Error Handling**: Comprehensive error propagation for parsing failures
- **Memory Management**: Efficient deduplication cache with automatic cleanup
  - **Time-based Expiry**: Removes cache entries older than 60 seconds
  - **Bounded Memory**: Prevents unbounded cache growth over time

### 🛡️ Quality Assurance
- **Enhanced Test Suite**: Comprehensive testing of new deduplication logic
- **Serialized Execution**: Eliminates race conditions in test environment
- **Performance Validation**: Deduplication performance benchmarked and optimized
- **Schema Compatibility**: New field schema maintains backward compatibility

## [2.2.3] - 2025-08-31

### 🔄 Self-Update System V1

#### ✨ 核心自动更新功能
- **内置版本检查系统**: 默认启用的自动更新能力
  - **手动检查模式**: `ccstatus --check-update` 命令行工具立即检查更新
  - **后台集成检查**: 状态栏正常使用时自动进行版本检测
  - **智能更新提醒**: 发现新版本时状态栏显示闪烁文本提醒用户
  - **跨会话状态持久化**: 更新检查历史和节流状态跨会话保持
  
#### 🌍 地理路由优化
- **中国大陆网络优化**: 自动检测地理位置并优化下载路径
  - **代理加速**: 中国用户自动使用 `hk.gh-proxy.com` 代理加速访问 GitHub
  - **智能降级**: 代理失败时自动回退到官方 `raw.githubusercontent.com`
  - **双路径策略**: 主要路径 + 备用路径确保更新检查可靠性
  - **24小时缓存**: 地理检测结果缓存24小时，减少重复检测开销

#### ⚡ 高性能缓存系统
- **HTTP 缓存优化**: 基于 ETag 和 Last-Modified 的智能缓存
  - **条件请求**: 支持 `If-None-Match` 和 `If-Modified-Since` 头部
  - **304 Not Modified**: 无变化时服务器返回 304，节省带宽和时间
  - **主机级缓存**: 按主机名分离缓存，支持代理和官方地址独立缓存
  - **持久化状态**: 缓存信息持久化到 `~/.claude/ccstatus/update-state.json`

#### 🏗️ 架构设计与模块化
- **专业模块分离**: 清晰的关注点分离设计
  - `updater/state.rs`: 状态文件管理和更新逻辑协调
  - `updater/manifest.rs`: 清单文件获取和版本解析客户端
  - `updater/url_resolver.rs`: URL解析和地理路由策略
  - `updater/geo.rs`: 地理位置检测和TTL缓存管理
  - `updater/github.rs`: GitHub API 集成和资产查找
- **状态栏集成**: 与现有 `update.rs` 段无缝集成，支持闪烁提醒
- **错误恢复**: 全面的错误处理和降级机制，确保更新检查不影响主要功能

#### 🛠️ 开发与构建系统改进
- **默认特性更新**: `self-update` 现已纳入默认构建特性
  - **新默认**: `["network-monitoring", "self-update"]`
  - **向后兼容**: 现有用户无需修改，自动获得更新功能
  - **大小优化**: 默认构建从 3MB 增至 4.1MB，增加更新能力
- **CI/CD 修复**: GitHub Actions 工作流修复，确保所有发布包含自动更新
  - **Slim 构建**: `"network-monitoring,self-update"` 
  - **Static 构建**: `"timings-curl-static,self-update"`
  - **发布一致性**: 所有 GitHub Release 二进制文件均支持 `--check-update`

#### 🧪 全面测试覆盖
- **单元测试完整性**: 新增 26 个更新系统相关测试
  - **状态管理测试**: ETag/Last-Modified 缓存逻辑验证
  - **URL 解析测试**: 地理路由策略和代理降级测试  
  - **清单解析测试**: JSON 解析和版本比较逻辑测试
  - **GitHub API 测试**: 版本解析和资产查找功能测试
- **测试重组优化**: 更新系统测试统一组织
  - **测试目录重构**: `tests/update/` → `tests/updater/` 统一命名
  - **内联测试提取**: 所有 `#[cfg(test)]` 模块提取为独立测试文件
  - **测试命名规范**: 统一 `{module}_test.rs` 命名约定
  - **完整覆盖验证**: 136 个测试全部通过，无回归问题

### 🔧 质量与兼容性改进

#### 🛡️ API 兼容性修复  
- **ureq 3.1.0 兼容**: 修复 HTTP 客户端 API 变更导致的编译问题
  - **头部提取修复**: 从 `.header()` 更新为 `.headers().get()` API
  - **响应体处理**: 确保 `response` 可变性用于 `.body_mut().read_to_string()`
  - **类型安全**: 移除未使用的 HashMap 导入和辅助函数
  
#### 📋 文档全面更新
- **README 双语更新**: 中英文档同步更新 Self-Update V1 能力描述
- **构建说明更新**: 反映新的默认特性和构建选项
- **CHANGELOG 详细记录**: 完整记录 Self-Update V1 实现细节
- **BUILD.md 修订**: 更新默认特性说明和构建矩阵

#### 🎯 用户体验提升
- **即开即用**: 默认构建即包含自动更新，无需额外配置
- **智能提醒**: 状态栏闪烁提醒平衡了可见性和干扰性
- **网络友好**: 缓存和地理优化减少网络开销和延迟
- **透明操作**: 更新检查不影响正常状态栏功能和性能

### 🛡️ 安全与隐私
- **无自动下载**: 仅检查版本，不自动下载或安装
- **用户控制**: 用户完全控制何时检查和如何更新
- **网络最小化**: 通过缓存和条件请求最小化网络活动
- **状态透明**: 所有更新检查活动对用户可见

---

## [2.2.2] - 2025-08-30

### 📦 NPM Package Distribution

#### 🚀 One-Command Installation
- **NPM 包发布**: 发布到 `@mauruppi/ccstatus` 命名空间，支持一键全平台安装
  - **简化安装**: `npm install -g @mauruppi/ccstatus` 即可完成安装配置
  - **平台感知**: 自动检测并安装对应平台的静态二进制文件
  - **自动配置**: 安装后自动部署到 `~/.claude/ccstatus/ccstatus` 供 Claude Code 使用
  - **零依赖部署**: 所有平台包均为静态构建，无需系统依赖库
- **多平台支持**: 涵盖主要开发平台
  - **macOS Intel**: `@mauruppi/ccstatus-darwin-x64` (静态构建)
  - **macOS Apple Silicon**: `@mauruppi/ccstatus-darwin-arm64` (静态构建) 
  - **Linux x64**: `@mauruppi/ccstatus-linux-x64` (静态构建)
  - **Windows x64**: `@mauruppi/ccstatus-win32-x64` (静态构建)
- **CI/CD 自动化**: GitHub Actions 集成 NPM 发布流程
  - **版本同步**: 基于 Git 标签自动发布到 NPM 仓库
  - **发布顺序**: 先发布平台包，后发布主包，确保依赖完整性
  - **质量保证**: 发布前自动验证包结构和二进制文件

#### 🌍 开发者体验提升
- **Node.js 生态集成**: 为前端开发者和 VS Code/Claude 插件生态提供便利
- **包管理器兼容**: 支持 npm、yarn、pnpm 等主流 Node.js 包管理器
- **中国大陆优化**: 支持 `--registry https://registry.npmmirror.com` 镜像加速
- **版本管理**: 通过 `npm update -g @mauruppi/ccstatus` 轻松更新到最新版本

### 🔧 Critical Bug Fixes

#### 📝 JSONL Monitor Operational Log Cleanup  
- **消除日志噪音**: 完全移除 `"type":"tail_scan_complete"` 记录污染
  - **问题**: v2.2.1 中 JSONL 错误日志被大量无用的扫描完成记录污染 (7,000+ 条记录)
  - **解决**: 移除 tail_scan_complete 的 JSONL 写入，仅保留调试日志
  - **影响**: 运营日志现在只包含真实的 API 错误事件，显著减少日志噪音
- **时间戳规范化**: 新增 `normalize_error_timestamp()` 函数处理无效时间戳
  - **RFC3339 验证**: 严格解析 RFC3339 格式时间戳
  - **占位符过滤**: 自动识别并替换测试用占位符时间 (`2024-01-01T12:*:00Z`)
  - **本地时间后备**: 无效时间戳自动使用本地时间替代，确保日志完整性

#### ⚡ 网络诊断 TTFB 增强
- **双重 TTFB 测量**: 区分服务器处理时间和端到端响应时间
  - **ServerTTFB**: `starttransfer_time - appconnect_time` (隔离服务器处理时间)
  - **TotalTTFB**: `starttransfer_time` (包含 DNS+TCP+TLS+服务器的完整时间)
  - **状态感知格式**: 降级/错误状态显示增强格式便于故障诊断
- **curl 路径增强**: 高精度计时显示完整网络栈分解
  - **健康状态**: `P95:200ms` (保持现有显示)
  - **降级状态**: `P95:200ms DNS:25ms|TCP:30ms|TLS:35ms|ServerTTFB:100ms/TotalTTFB:190ms|Total:250ms`
  - **错误状态**: `DNS:25ms|TCP:30ms|TLS:35ms|ServerTTFB:100ms/TotalTTFB:190ms|Total:250ms`
- **isahc 路径简化**: 后备路径使用简洁的总时间显示
  - **健康状态**: `P95:200ms` (保持一致)
  - **降级状态**: `P95:200ms Total:250ms`
  - **错误状态**: `Total:250ms`

### 🛠️ 架构改进

#### 📊 PhaseTimings 结构扩展
- **新增字段**: `total_ttfb_ms: u32` 用于端到端 TTFB 测量
- **向后兼容**: 所有现有代码继续正常工作
- **测试更新**: 完整的测试套件覆盖新字段和格式

#### 🏗️ 代码质量提升
- **重构合并策略**: 正确合并 feat/jsol-improve 和 feat/ttfb-enhancement 分支
- **版本控制修复**: 删除有缺陷的 v2.2.1 标签，发布完整的 v2.2.2
- **构建验证**: 确保所有 curl 和 isahc 路径测试通过

### 🔍 诊断能力提升

#### 🎯 网络故障排查
- **精确定位**: ServerTTFB 帮助区分网络延迟和服务器处理延迟
- **完整视图**: TotalTTFB 提供用户体验的真实测量
- **问题分类**: 便于识别是网络连接问题还是服务器处理问题

#### 📋 运营监控
- **清洁日志**: 消除无用噪音，专注真实错误事件
- **时间准确性**: 规范化时间戳确保日志分析的可靠性
- **存储效率**: 减少不必要的日志条目，节省磁盘空间

### 🛡️ 质量保证

#### ✅ 测试覆盖
- **42/44 curl 测试通过**: 高精度计时路径验证
- **32/32 isahc 测试通过**: 后备路径完整验证
- **编译验证**: 所有平台构建成功

#### 🔧 部署就绪
- **二进制更新**: `target/release/ccstatus` 包含所有修复
- **即时生效**: 部署后立即停止产生 tail_scan_complete 记录
- **历史数据保留**: 现有 7,031 条历史记录保持不变

### 📋 修复的缺陷

**v2.2.1 问题**: JSONL 改进未正确合并导致：
- ❌ tail_scan_complete 记录继续污染运营日志
- ❌ 时间戳规范化功能缺失
- ❌ 构建的二进制文件缺少 JSONL 修复

**v2.2.2 解决**:
- ✅ 正确合并所有分支到 master
- ✅ 完整的 JSONL 和 TTFB 功能集成
- ✅ 通过全面测试和构建验证

---

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

