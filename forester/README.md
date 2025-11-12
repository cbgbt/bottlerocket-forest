# Forester

Forester is a Rust CLI tool that orchestrates development workflows across the Bottlerocket Forest. It provides higher-level commands that coordinate operations across multiple repositories, making it easier for both humans and AI agents to work with Bottlerocket's multi-repository architecture.

## Purpose

Forester manages:
- **Local OCI registry** - Run a local Docker registry for kit development
- **Build orchestration** - Coordinate builds across kits and variants (future)
- **Development status** - Show what's built and running (future)
- **Environment management** - Test infrastructure lifecycle (future)

## Installation

Build from source:

```bash
cd forester
cargo build --release
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

- Docker installed and running
- User must be in the `docker` group (or have Docker permissions)

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

Run all quality checks:

```bash
make check
```

This runs formatting checks, lints, and all tests (unit and integration).

Individual checks:

```bash
make fmt      # Check code formatting
make clippy   # Run lints
make test     # Run all tests
```

Integration tests use the `serial_test` crate with `#[serial(registry)]` to ensure tests that manipulate the Docker registry run one at a time. Tests use a dedicated test port (5555), and each test starts with a clean state and cleans up after itself.

### Project Structure

```
forester/
├── src/
│   ├── main.rs          # CLI entry point (thin wrapper)
│   ├── lib.rs           # Library entry point
│   ├── config.rs        # Configuration loading
│   └── registry/        # Registry management
│       ├── mod.rs       # Public API
│       ├── types.rs     # Domain types
│       ├── docker.rs    # Docker interaction
│       └── health.rs    # Health checking
└── tests/
    └── registry_integration.rs
```

### Implementation Guide

See `planning/forester-registry-impl.md` for the detailed implementation checklist.

## Design Principles

- **Agent-friendly** - Clear commands, informative output, helpful errors
- **Composable** - Commands work independently and can be chained
- **Type-safe** - Strong domain types prevent invalid states
- **Minimal dependencies** - Leverage existing tools (Docker, Twoliter)
- **Fail fast** - Validate early, provide clear error messages

## License

See the forest repository root for license information.
