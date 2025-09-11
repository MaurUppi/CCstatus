# Repository Guidelines

## Project Structure & Modules
- `src/`: Rust sources — `main.rs`, `cli.rs`, `lib.rs`, and modules: `core/`, `config/`, `ui/`, `themes/`, `updater.rs`.
- `tests/`: Integration tests (`main.rs`, `core/`, `common/`) plus E2E harness (`run_e2e_ccstatus.sh`).
- `assets/`: Screenshots and reference images used in docs.
- `npm/`: Packaging for `@mauruppi/ccstatus` (main and per‑platform packages).
- `target/`: Build artifacts (ignored in VCS).

## Build, Test, and Development
- Build (default): `cargo build --release`
- With features: `cargo build --release --features "tui,self-update,timings-curl"`
- Dev build: `cargo build --features network-monitoring`
- Tests: `cargo test` (or `--features network-monitoring`, `--features timings-curl`)
- Run locally: `cargo run --features tui`
- E2E: `tests/run_e2e_ccstatus.sh [scenario]`
  - Example: `ANTHROPIC_BASE_URL=https://api.anthropic.com ANTHROPIC_AUTH_TOKEN=... tests/run_e2e_ccstatus.sh green`

## Coding Style & Naming
- Rust 2021, format with `cargo fmt`; lint with `cargo clippy -- -D warnings`.
- Indentation: 4 spaces; max line length ~100 where practical.
- Naming: modules/functions `snake_case`, types `CamelCase`, constants `SCREAMING_SNAKE_CASE`.
- Prefer `Result` over panics; avoid `unwrap()` in non‑test paths; keep modules small and cohesive.

## Testing Guidelines
- Framework: Rust test harness (unit/integration) in `src/**` and `tests/**`.
- Integration naming: group by module (e.g., `tests/core/...`). Shared helpers in `tests/common/`.
- Network features: run with `--features network-monitoring`; curl timings via `--features timings-curl`.
- E2E writes state to `~/.claude/ccstatus/ccstatus-monitoring.json` — clean between runs if needed.

## Commit & Pull Requests
- Commits: Conventional Commits (e.g., `feat(network): add TTFB metrics`, `fix(ui): handle empty transcript`).
- PRs must include: clear description, linked issues, affected features (`tui/self-update/timings-*`), test coverage notes, and screenshots for statusline/UI changes.
- Pre‑submit checklist: `cargo check`, `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test` (include feature combos you touched).

## Security & Configuration
- Never commit credentials. Use env vars: `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`.
- Debug locally with `CCSTATUS_DEBUG=true`.
- Prefer static timings builds for portability: `--features timings-curl-static`.
