# Forester

Forester is a Rust CLI tool that orchestrates development workflows across the Bottlerocket Forest. It provides higher-level commands that coordinate operations across multiple repositories, making it easier for both humans and AI agents to work with Bottlerocket's multi-repository architecture.

## Purpose

Forester provides:

- **Knowledge Index** - Semantic search across forest documentation using ML embeddings
- **Local OCI Registry** - Run a local Docker registry for kit development

## Installation

Build from source:

```bash
cd forester
cargo build --release
```

The binary will be at `target/release/forester`.

## Usage

### Knowledge Index

The knowledge index provides semantic search across all forest documentation, enabling fast, targeted documentation lookup.

Build the index (first time or after major changes):

```bash
forester index build
```

Search the documentation:

```bash
forester index search "how to build a kit"
forester index search "boot process" --limit 5
forester index search "systemd configuration" --show-chunks
```

Check index status:

```bash
forester index status
```

Update incrementally (faster than full rebuild):

```bash
forester index update
```

Rebuild from scratch:

```bash
forester index rebuild
```

The index uses the `sentence-transformers/all-MiniLM-L6-v2` model for embeddings and stores data in `.forester/knowledge/` at the forest root. It automatically discovers documentation from all repositories and supports markdown files, Rust source files, and other text formats.

### Registry Management

Start a local OCI registry for development:

```bash
forester registry start
```

Check registry status:

```bash
forester registry status
```

View registry logs:

```bash
forester registry logs
```

Stop the registry (preserves data):

```bash
forester registry stop
```

Remove registry and all data:

```bash
forester registry clean
```

### Configuration

Forester uses environment variables for configuration. Create a `.env` file in the forester directory or set environment variables:

```bash
# Registry port (default: 5000, minimum: 1024)
FORESTER_REGISTRY_PORT=5000

# Registry image (default: registry:2)
FORESTER_REGISTRY_IMAGE=registry:2
```

Note: Container and volume names are automatically derived from the port as `forester-registry-{port}` and `forester-registry-data-{port}`.

## Requirements

- Rust toolchain (for building)
- Docker installed and running (for registry commands)
- User must be in the `docker` group (or have Docker permissions)

The knowledge index downloads ML models automatically on first use (~90MB for the embedding model).

## Development

### Building

Build for development:

```bash
make build
```

Build for release:

```bash
make release-build
```

The binary will be at `target/debug/forester` or `target/release/forester` respectively.

### Code Quality

Run all quality checks, including integration tests:

```bash
make integ
```

This runs formatting checks, lints, and all tests (unit and integration).
If you need a quicker validation, you can run:

```bash
make check
```

Integration tests use the `serial_test` crate with `#[serial(registry)]` to ensure tests that manipulate the Docker registry run one at a time.
Tests use a dedicated test port (5555), and each test starts with a clean state and cleans up after itself.
