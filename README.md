# git-changes

A Rust library and CLI tool designed to detect and process file changes in Git repositories, optimized for CI/CD environments like GitHub Actions and GitLab CI.

## Features

- 🔍 **Simple Analysis**: List changed files or export them with diffs
- 🎯 **Branch Analysis**: Compare changes between branches
- 🚀 **CI-optimized**: Works seamlessly in GitHub Actions and GitLab CI
- 📊 **Verbose logging**: Optional detailed output for debugging
- 📚 **Dual usage**: Can be used as a library or CLI tool

## Installation

### As a CLI Tool

```bash
# From crates.io
cargo install git-changes

# From source
git clone <your-repo-url>
cd git-changes
cargo install --path .
```

### As a Library Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
git-changes = "0.1.0"
```

### Prerequisites

- Rust 1.70+
- Git repository access

## Usage

### CLI Usage

```bash
# List changed files (no output directory)
git-changes --repo . --branch feature/my-feature

# Using HTTPS repository
git-changes --repo https://github.com/username/repo.git --branch feature/my-feature

# Using SSH repository
git-changes --repo git@github.com:username/repo.git --branch feature/my-feature

# Export changed files with diffs
git-changes --repo . --branch feature/my-feature --output-dir ./changes

# Advanced Options
git-changes --repo . --branch feature/my-feature --target-branch main  # Compare against specific branch
git-changes --repo . --commits abc123,def456 --output-dir ./changes    # Analyze specific commits
git-changes --repo . --branch feature/my-feature --log debug          # Enable debug logging
```

### Library Usage

```rust
use git_changes::{self, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    // Initialize the processor
    let processor = git_changes::new(".", Some("main"))?;

    // Analyze a branch
    let target = processor.analyze_branch("feature/my-branch")?;

    // List changes without writing to disk
    let changes = processor.list_changes(&target)?;

    // Or export changes to a directory
    let changes = processor.export_changes(&target, PathBuf::from("./changes"))?;

    Ok(())
}
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.
