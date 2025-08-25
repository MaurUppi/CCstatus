### CredentialManager — Review against Network Monitoring Pipeline v2

Date: 2025-08-25
Target file: `src/core/network/credential.rs`

---

#### Classification Overview
- Critical: 1 finding
- Medium: 3 findings
- Low: 2 findings

---

### Critical
- `new()` hard-depends on HOME/USERPROFILE, blocking env-only setups
  - Behavior: If neither `HOME` nor `USERPROFILE` is set, `CredentialManager::new()` returns `HomeDirNotFound` and the manager cannot be created, even if environment variables are correctly provided.
  - Evidence:
```rust
pub fn new() -> Result<Self, NetworkError> {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map_err(|_| NetworkError::HomeDirNotFound)?;
    // ... uses home to build claude_config_paths
}
```
  - Impact: Violates the spec's fail-silent posture and "parse each stdin trigger, no cache" flow by preventing any credential resolution in minimal/container environments. Environment-first should still work without a home dir.
  - Recommendation: Make HOME optional. If missing, still construct the manager (with only relative config paths) so `get_credentials()` can resolve from environment and then shell/config opportunistically.

---

### Medium
- Claude config key mismatch; likely misses valid credentials
  - Current: expects `{ "api_base_url" | "base_url", "auth_token" }`.
  - Spec: only states "Claude Code 配置：JSON 键值读取"; internal docs/examples often use `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN`.
  - Evidence:
```rust
if let (Some(base_url), Some(auth_token)) = (
    config.get("api_base_url").and_then(|v| v.as_str()),
    config.get("auth_token").and_then(|v| v.as_str())
) { /* ... */ }
// alt
if let (Some(base_url), Some(auth_token)) = (
    config.get("base_url").and_then(|v| v.as_str()),
    config.get("auth_token").and_then(|v| v.as_str())
) { /* ... */ }
```
  - Recommendation: Also accept `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN` and nested shapes (e.g., `credentials.{base_url,auth_token}`).

- Shell function parsing is narrow; requires `local` arrays
  - Current regex only detects `local VAR=(` to start array blocks; misses common patterns like `VAR=(`, `declare -a VAR=(`.
  - Evidence:
```rust
let array_start_regex = Regex::new(r#"^\s*local\s+[a-zA-Z_][a-zA-Z0-9_]*\s*=\s*\("#)?;
```
  - Recommendation: Broaden to `(?:local|declare\s+-a)?\s*NAME\s*=\s*\(` and iterate until `)`.

- Export parsing doesn’t handle inline comments and richer quoting
  - Current regex may capture trailing comments or unmatched quotes, leading to invalid values.
  - Evidence:
```rust
let export_regex = Regex::new(r#"^\s*export\s+([A-Z_]+)=(["']?)([^\n\r]+)"#)?;
// later only trims a single trailing quote char
```
  - Recommendation: Support `export VAR = value`, quoted/unquoted with escapes, and strip `# ...` comments safely.

---

### Low
- Minor whitespace/quoting normalization
  - Trim surrounding whitespace after unquoting; collapse stray spaces to avoid subtle mismatches.

- Limited shell config coverage
  - Only core files are scanned. This is acceptable per spec, but consider optional inclusion of `~/.zshrc.d/*.zsh`, `~/.bash_aliases` if present.

---

### Meets Requirements
- Priority: Environment > Shell config > Claude config.
- Returns `Some(base_url, auth_token, source)` on success; `Ok(None)` on absence.
- No state writes; source tracking via `CredentialSource` for downstream `HttpMonitor`.
- No secret logging: only endpoint and token length are logged when debug is enabled.
- No caching: re-evaluates on each call.

---

### Recommendations (Ordered)
1. Make `new()` resilient to missing home directory; allow env-first resolution to proceed.
   - Acceptance: With `ANTHROPIC_*` env vars set and no HOME, `get_credentials()` returns `Some(...)`.
2. Accept additional Claude config shapes: `ANTHROPIC_BASE_URL`/`ANTHROPIC_AUTH_TOKEN` and `credentials.{base_url,auth_token}`.
   - Acceptance: All shapes resolve correctly with `source=ClaudeConfig(path)`.
3. Broaden function-array detection beyond `local` and support `declare -a` and bare `NAME=(`.
4. Harden export parsing for comments/quoting/spacing; add tests for typical variants.
5. Trim/normalize extracted values (after unquoting) to reduce edge-case parsing errors.
