# Code Style and Conventions

## Rust Style Guidelines
- **Standard Rust formatting** using `cargo fmt`
- **Clippy linting** with warnings treated as errors (`-D warnings`)
- **Snake_case** for variables, functions, and module names
- **PascalCase** for structs, enums, and traits
- **SCREAMING_SNAKE_CASE** for constants

## Code Organization
- **Module structure**: 
  - `core/` - Core functionality (statusline, segments)
  - `ui/` - TUI interface components
  - `config/` - Configuration types and loading
  - `themes/` - Theme system and presets
- **Trait-based design** for segments (implements `Segment` trait)
- **Error handling** with `Result<T, Box<dyn std::error::Error>>`

## Documentation
- **Doc comments** using `///` for public APIs
- **Module documentation** using `//!` at module level
- **Inline comments** for complex logic only

## Git Conventions (from CLAUDE.md)
- **Conventional Commits** format:
  - `feat(<scope>): ...` for new features
  - `fix(<scope>): ...` for bug fixes  
  - `chore(<scope>): ...` for configs/docs/misc
  - `test(<scope>): ...` for tests
- **Feature branches**: `feature/<module-or-task>`
- **Complete phase commits** only (no partial work)

## Naming Patterns
- **Segment modules**: `segments/directory.rs`, `segments/git.rs`
- **Config types**: Clear, descriptive names like `SegmentConfig`, `StyleConfig`
- **UI components**: Component-based organization in `ui/components/`

## Performance Considerations
- **Minimal allocations** in hot paths
- **Efficient string handling** with proper borrowing
- **Optional features** to keep binary size small
- **Fast startup** target of <50ms