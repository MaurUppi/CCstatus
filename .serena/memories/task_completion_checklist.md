# Task Completion Checklist

## Code Quality Checks (MUST RUN)
Before considering any task complete, run these commands in order:

1. **Format Check** (CI requirement)
   ```bash
   cargo fmt -- --check
   ```
   If fails, run `cargo fmt` to fix formatting.

2. **Lint Check** (CI requirement)  
   ```bash
   cargo clippy -- -D warnings
   ```
   Fix all warnings before proceeding.

3. **Test Suite** (CI requirement)
   ```bash
   cargo test --verbose
   ```
   All tests must pass.

4. **Build Check** (Release verification)
   ```bash
   cargo build --release
   ```
   Ensure clean release build.

## Integration Testing
5. **Functional Testing**
   ```bash
   # Test basic functionality with mock input
   echo '{"workingDirectory": "/test", "model": "claude-3-5-sonnet"}' | cargo run
   
   # Test configuration commands
   cargo run -- --print
   cargo run -- --check
   ```

## Git Workflow (per CLAUDE.md)
6. **Commit Standards**
   - Use Conventional Commits: `feat(scope):`, `fix(scope):`, `chore(scope):`
   - Only commit when phase is complete (no partial work)
   - Use feature branches: `feature/<module-or-task>`

7. **Pre-commit Verification**
   ```bash
   git status          # Check what will be committed
   git diff --cached   # Review staged changes
   ```

## Documentation Updates
8. **Update Documentation** (if applicable)
   - Update README.md for user-facing changes
   - Update inline documentation for API changes
   - Update CHANGELOG.md for version releases

## Performance Verification
9. **Performance Check** (for core changes)
   ```bash
   # Verify startup time remains under 50ms
   time cargo run -- --print
   
   # Check binary size
   ls -lh target/release/ccometixline
   ```

## Final Validation
10. **Cross-platform Compatibility** (major changes)
    - Test on different platforms if possible
    - Verify CLI arguments work consistently
    - Check file path handling for Windows/Unix differences

The CI pipeline runs tests on ubuntu-latest, windows-latest, and macos-latest, so local testing should cover basic cross-platform concerns.