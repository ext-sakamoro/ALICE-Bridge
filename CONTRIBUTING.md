# Contributing to ALICE-Bridge

## Development

```bash
# Run all tests
cargo test

# Run with all features
cargo test --all-features

# Check formatting
cargo fmt --check

# Lint
cargo clippy --all-features -- -D warnings
```

## Adding a New Protocol

1. Create `src/protocol/my_protocol.rs`
2. Implement the `Protocol` trait
3. Add feature gate in `Cargo.toml` if external dependency needed
4. Register module in `src/protocol/mod.rs`
5. Add integration tests

## Code Standards

- All public APIs must have doc comments
- Safety-critical code must have unit tests
- No `unwrap()` in library code (use `?` or explicit error handling)
- Follow ALICE quality standards (see ALICE-KARIKARI.md)
