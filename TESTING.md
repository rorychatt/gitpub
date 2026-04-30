# Testing Guide

This document describes how to run and write tests for the gitpub project.

## Running Tests

### Run all tests

```bash
cargo test --workspace --all-features
```

### Run tests for a specific component

```bash
# Core library tests
cargo test -p gitpub-core

# Server tests
cargo test -p gitpub-server

# CLI tests
cargo test -p gitpub-cli

# Sliplane integration tests
cargo test -p gitpub-sliplane
```

### Run specific test files

```bash
# Run integration tests
cargo test -p gitpub-core --test integration_tests

# Run API tests
cargo test -p gitpub-server --test api_tests

# Run CLI tests
cargo test -p gitpub-cli --test cli_tests

# Run Sliplane tests
cargo test -p gitpub-sliplane --test sliplane_tests
```

### Run tests with output

```bash
cargo test -- --nocapture
```

### Run a specific test

```bash
cargo test test_repository_creation
```

## Database Setup for Integration Tests

The integration tests in `gitpub-core` require a PostgreSQL database.

### Local database setup

If you want to run database tests locally, set the `DATABASE_URL` environment variable:

```bash
export DATABASE_URL=postgresql://postgres:postgres@localhost/gitpub_test
cargo test -p gitpub-core
```

The database connection test will be skipped if the database is not available.

## Test Organization

### Unit Tests

Unit tests are located in the same file as the code they test, in a `tests` module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // test code
    }
}
```

### Integration Tests

Integration tests are in the `tests/` directory of each component:

- `gitpub-core/tests/` - Core library integration tests
- `gitpub-server/tests/` - API endpoint tests
- `gitpub-cli/tests/` - CLI parsing and command tests
- `gitpub-sliplane/tests/` - Sliplane API client tests

## Writing Tests

### Testing Dependencies

The workspace provides common test dependencies:

- `tokio-test` - Async test utilities
- `mockall` - Mocking framework
- `rstest` - Parameterized testing
- `assert_matches` - Pattern matching assertions
- `wiremock` - HTTP mocking (for API tests)
- `testcontainers` - Docker containers for integration tests
- `axum-test` - Axum application testing

### Example: Unit Test

```rust
#[test]
fn test_user_creation() {
    let user = User::new("testuser".to_string(), "test@example.com".to_string());
    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "test@example.com");
}
```

### Example: Async Test

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = some_async_function().await;
    assert!(result.is_ok());
}
```

### Example: HTTP API Test

```rust
use axum_test::TestServer;

#[tokio::test]
async fn test_endpoint() {
    let server = TestServer::new(create_app()).unwrap();
    let response = server.get("/health").await;
    response.assert_status_ok();
}
```

### Example: Mock API Test

```rust
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_with_mock() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Test code using mock_server.uri()
}
```

## Mocking Guidelines

### When to Mock

- External API calls (Sliplane, GitHub, etc.)
- Database operations in unit tests (use real database in integration tests)
- File system operations that would be destructive
- Network operations that require external resources

### When NOT to Mock

- Simple data structures and models
- Pure functions without side effects
- Integration tests (use real dependencies when possible)

## Continuous Integration

Tests run automatically on:

- Push to `main` branch
- Pull requests to `main` branch

The CI workflow is defined in `.github/workflows/test.yml` and includes:

1. Code formatting check (`cargo fmt`)
2. Linting (`cargo clippy`)
3. Build verification
4. Test execution
5. Documentation tests

## Code Coverage

To generate code coverage locally:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --all-features

# Open coverage report
open tarpaulin-report.html
```

## Best Practices

1. **Test behavior, not implementation** - Focus on what the code does, not how it does it
2. **One assertion per test** - Keep tests focused and easy to debug
3. **Use descriptive test names** - Test names should explain what is being tested
4. **Arrange-Act-Assert** - Structure tests with clear setup, execution, and verification phases
5. **Test edge cases** - Don't just test the happy path
6. **Keep tests fast** - Unit tests should run in milliseconds
7. **Make tests independent** - Tests should not depend on each other
8. **Clean up resources** - Especially in integration tests

## Troubleshooting

### Tests failing locally but passing in CI

- Check Rust version: CI uses stable, ensure you're on stable locally
- Check environment variables: Some tests may need specific env vars

### Database connection failures

- Ensure PostgreSQL is running
- Check `DATABASE_URL` environment variable
- Verify database credentials

### Slow tests

- Use `--release` flag for release-mode testing
- Run specific test suites instead of all tests
- Use `cargo test -- --test-threads=1` to run tests sequentially

### Mock server issues

- Ensure wiremock server is properly started
- Check that mocks are mounted before making requests
- Verify request matchers are correct
