# gitpub CLI Usage Guide

## Installation

```bash
cargo install --path gitpub-cli
```

## Commands

### init

Initialize a new gitpub repository.

```bash
gitpub init <repository-name>
```

Example:
```bash
gitpub init my-awesome-project
```

### clone

Clone a repository from gitpub.

```bash
gitpub clone <repository-url> [--directory <path>]
```

Examples:
```bash
# Clone to default directory
gitpub clone https://gitpub.io/user/repo

# Clone to specific directory
gitpub clone https://gitpub.io/user/repo --directory my-project
```

### push

Push changes to a gitpub repository.

```bash
gitpub push [remote] [branch]
```

Examples:
```bash
# Push to default (origin/main)
gitpub push

# Push to specific remote and branch
gitpub push origin develop
```

### pull

Pull changes from a gitpub repository.

```bash
gitpub pull [remote] [branch]
```

Examples:
```bash
# Pull from default (origin/main)
gitpub pull

# Pull from specific remote and branch
gitpub pull origin develop
```

## Configuration

gitpub CLI stores configuration in `~/.gitpub/config.json`.

### Setting User Info

```bash
gitpub config set user.name "Your Name"
gitpub config set user.email "your@email.com"
```

### Setting API Endpoint

```bash
gitpub config set api.url "https://gitpub.io"
```

### Authentication

```bash
gitpub login
```

This will prompt for your gitpub credentials and store an authentication token.

## Examples

### Create and Push a New Project

```bash
# Initialize repository
gitpub init my-project
cd my-project

# Add files
echo "# My Project" > README.md
git add README.md
git commit -m "Initial commit"

# Push to gitpub
gitpub push origin main
```

### Clone and Contribute

```bash
# Clone repository
gitpub clone https://gitpub.io/user/repo

# Make changes
cd repo
echo "new feature" > feature.txt
git add feature.txt
git commit -m "Add new feature"

# Push changes
gitpub push origin main
```

## Tips

- The CLI works seamlessly with standard git commands
- You can use git for local operations and gitpub CLI for remote operations
- Authentication tokens are stored securely in your system keychain
