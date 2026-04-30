# Contributing to gitpub

Thank you for your interest in contributing to gitpub! This document provides guidelines and instructions for contributing.

## Getting Started

1. Fork the repository
2. Clone your fork locally
3. Create a new branch for your feature or bugfix
4. Make your changes
5. Run tests and linting
6. Submit a pull request

## Development Setup

### Prerequisites

- Rust 1.70 or higher
- PostgreSQL 14 or higher
- Git 2.30 or higher

### Building the Project

```bash
cargo build --workspace
```

### Running Tests

```bash
cargo test --workspace
```

## Code Style

We use `rustfmt` for code formatting and `clippy` for linting.

### Format your code

```bash
cargo fmt --all
```

### Run the linter

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

## Pull Request Process

1. Update documentation if needed
2. Add tests for new functionality
3. Ensure all tests pass
4. Ensure code is formatted and passes linting
5. Write a clear commit message
6. Submit the pull request with a detailed description

## Commit Message Guidelines

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests liberally after the first line

## Testing Guidelines

- Write unit tests for new functionality
- Ensure all tests pass before submitting a PR
- Aim for high code coverage

## Questions?

Feel free to open an issue if you have questions or need clarification on anything.
