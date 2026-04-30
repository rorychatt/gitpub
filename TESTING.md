# Testing Guide

## Running Tests

### All Tests

```bash
cargo test --workspace
```

### Per-Component Tests

```bash
cargo test --package gitpub-core
cargo test --package gitpub-server
cargo test --package gitpub-cli
cargo test --package gitpub-sliplane
```

### Integration Tests Only

```bash
cargo test --workspace --test '*'
```

### Specific Test

```bash
cargo test --package gitpub-server --test api_tests
cargo test --package gitpub-core --test repository_tests
cargo test --package gitpub-cli --test command_tests
cargo test --package gitpub-sliplane --test deployment_tests
```

## Test Structure

Each crate has:
- **Unit tests** in `src/*.rs` (`#[cfg(test)]` modules)
- **Integration tests** in `tests/` directory
- **Test utilities** in `tests/common.rs`

### gitpub-core
- `tests/repository_tests.rs` — Repository, User, and Commit type tests
- `tests/common.rs` — Fixture builders for core types

### gitpub-server
- `tests/api_tests.rs` — HTTP endpoint tests using Tower's `ServiceExt::oneshot`
- `tests/common.rs` — Test app factory

### gitpub-cli
- `tests/command_tests.rs` — CLI binary invocation tests

### gitpub-sliplane
- `tests/deployment_tests.rs` — Deployment config, result, and client tests

## Writing New Tests

1. Place unit tests in a `#[cfg(test)]` module within the source file
2. Place integration tests in the crate's `tests/` directory
3. Share fixtures via `tests/common.rs`
4. Use `#[tokio::test]` for async tests
5. For server endpoint tests, use `tower::ServiceExt::oneshot` with the app from `common::test_app()`
6. For CLI tests, invoke the binary via `std::process::Command` with `env!("CARGO_BIN_EXE_gitpub-cli")`

## CI

Tests run automatically on push to `main` and on pull requests via GitHub Actions (`.github/workflows/test.yml`). CI includes:
- `cargo test --workspace --all-features`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-features -- -D warnings`

## Database Tests

Database integration tests require a running PostgreSQL instance. Set `DATABASE_URL` environment variable:

```bash
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/gitpub_test
```

CI provides a PostgreSQL service container automatically.
