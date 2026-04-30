# gitpub Architecture

## Overview

gitpub is built as a modular Rust workspace with clear separation of concerns across four main components.

## Components

### gitpub-core

The core library provides shared functionality:

- **Data Models**: Repository, User, Commit structs
- **Database**: PostgreSQL connection management via sqlx
- **Utilities**: Common utilities used across components

### gitpub-server

The web server provides:

- **REST API**: HTTP endpoints for repository operations
- **Git Protocol**: Smart HTTP protocol for git operations
- **Authentication**: User authentication and authorization
- **Web Framework**: Built with Axum for async performance

Key endpoints:
- `GET /health` - Health check
- `GET /api/repositories` - List repositories
- `POST /api/repositories` - Create repository
- `GET /api/repositories/:owner/:name` - Get repository details

### gitpub-cli

The command-line interface provides:

- **Git Operations**: Clone, push, pull, init
- **Repository Management**: Create, delete, list repositories
- **Configuration**: User settings and authentication
- **CLI Framework**: Built with Clap for ergonomic commands

### gitpub-sliplane

The Sliplane integration provides:

- **API Client**: HTTP client for Sliplane API
- **Deployment Config**: Configuration models for deployments
- **Auto-deployment**: Automatic deployment triggers
- **Environment Management**: Development, staging, production

## Data Flow

```
User → gitpub-cli → gitpub-server → gitpub-core → Database
                          ↓
                    Git Repository
                          ↓
                   gitpub-sliplane → Sliplane API
```

## Technology Stack

- **Language**: Rust 2021 edition
- **Web Framework**: Axum (async web framework)
- **CLI Framework**: Clap (command-line argument parsing)
- **Database**: PostgreSQL with sqlx
- **Git Operations**: libgit2 via git2-rs
- **Serialization**: serde with JSON support
- **Async Runtime**: Tokio

## Security

- User authentication required for all operations
- Repository access control (public/private)
- API key authentication for Sliplane integration
- HTTPS required for production deployments

## Scalability

- Async/await for non-blocking operations
- Connection pooling for database
- Horizontal scaling supported
- Sliplane auto-scaling integration
