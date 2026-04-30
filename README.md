# gitpub

A GitHub-like platform built with Rust, featuring direct integration with Sliplane for seamless deployment and hosting.

## Features

- 🦀 **Rust-Powered**: Built entirely in Rust for performance and reliability
- 🔄 **Git Repository Hosting**: Full-featured git repository management
- 🚀 **Sliplane Integration**: One-click deployment and hosting integration
- 🛠️ **CLI Tool**: Powerful command-line interface for all git operations
- 🔌 **Standard Git Compatible**: Works seamlessly with existing git repositories

## Architecture

The project is organized as a Rust workspace with four main components:

- **gitpub-server**: Web server and REST API for git operations
- **gitpub-cli**: Command-line interface for repository management
- **gitpub-core**: Shared core library with data models and utilities
- **gitpub-sliplane**: Sliplane integration module for deployment

## Getting Started

### Prerequisites

- Rust 1.70 or higher
- PostgreSQL 14 or higher
- Git 2.30 or higher

### Installation

```bash
# Clone the repository
git clone https://github.com/rorychatt/gitpub.git
cd gitpub

# Build all components
cargo build --workspace

# Run tests
cargo test --workspace
```

### Running the Server

```bash
# Set up database connection
export DATABASE_URL="postgresql://user:password@localhost/gitpub"

# Start the server
cargo run --package gitpub-server
```

The server will start on `http://localhost:3000`.

### Using the CLI

```bash
# Install the CLI
cargo install --path gitpub-cli

# Initialize a new repository
gitpub init my-repo

# Clone a repository
gitpub clone https://gitpub.io/user/repo

# Push changes
gitpub push origin main

# Pull changes
gitpub pull origin main
```

## Sliplane Integration

gitpub integrates directly with Sliplane for easy deployment:

1. Configure your Sliplane API credentials
2. Push your code to gitpub
3. Deploy automatically with zero configuration

See `docs/sliplane-integration.md` for detailed setup instructions.

## Development

### Project Structure

```
gitpub/
├── gitpub-server/      # Web server and API
│   └── src/
│       └── main.rs     # Server entry point
├── gitpub-cli/         # Command-line interface
│   └── src/
│       └── main.rs     # CLI entry point
├── gitpub-core/        # Shared core library
│   └── src/
│       └── lib.rs      # Core models and utilities
└── gitpub-sliplane/    # Sliplane integration
    └── src/
        └── lib.rs      # Sliplane API client
```

### Building

```bash
# Build all components
cargo build --workspace

# Build in release mode
cargo build --workspace --release
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run specific component tests
cargo test --package gitpub-core
```

### Formatting and Linting

```bash
# Format code
cargo fmt --all

# Run linter
cargo clippy --all-targets --all-features -- -D warnings
```

## Contributing

Contributions are welcome! Please see `CONTRIBUTING.md` for guidelines.

## License

MIT License - see `LICENSE` for details
