# Forester

Forester is a Rust CLI tool that orchestrates development workflows across the Bottlerocket Forest. It provides higher-level commands for managing local development infrastructure.

## Purpose

Forester provides:

- **Local OCI Registry** - Run a local Docker registry for kit development

## Installation

Build from source:

```bash
cd bottlerocket-forest
cargo build --release -p forester
```

The binary will be at `target/release/forester`.

## Usage

### Registry Management

Start a local OCI registry for development:

```bash
forester registry start
```

Check registry status:

```bash
forester registry status
```

List published images:

```bash
forester registry list
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

Forester uses environment variables for configuration. Create a `.env` file in the forest root or set environment variables:

```bash
# Registry port (default: 5000, minimum: 1024)
FORESTER_REGISTRY_PORT=5000

# Registry image (default: registry:2)
FORESTER_REGISTRY_IMAGE=registry:2
```

Note: Container and volume names are automatically derived from the port as `forester-registry-{port}` and `forester-registry-data-{port}`.

## Requirements

- Rust toolchain (for building)
- Docker installed and running
- User must be in the `docker` group (or have Docker permissions)

## Development

### Building

Build from the workspace root:

```bash
make build          # Development build
make release-build  # Optimized build
```

### Code Quality

Run all quality checks from the workspace root:

```bash
make integ  # Full test suite (fmt, clippy, deny, tests)
make check  # Quick validation (fmt, clippy, deny, unit tests)
```

Integration tests use the `serial_test` crate with `#[serial(registry)]` to ensure tests that manipulate the Docker registry run one at a time. Tests use a dedicated test port (5555), and each test starts with a clean state and cleans up after itself.
