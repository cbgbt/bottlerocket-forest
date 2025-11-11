# Forester Registry Implementation Checklist

This checklist provides atomic, verifiable steps to implement the registry management feature for forester. Each step builds on the previous ones.

## Module Design

### Project Structure

```
forester/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI entry point (thin wrapper)
│   ├── lib.rs           # Library entry point
│   ├── config.rs        # Configuration loading (dotenv, env vars)
│   └── registry/
│       ├── mod.rs       # Public API for registry operations
│       ├── types.rs     # Domain types
│       ├── docker.rs    # Docker interaction
│       └── health.rs    # Health checking
└── tests/
    └── registry_integration.rs
```

### Domain Types (registry/types.rs)

**RegistryPort** - Newtype for port validation
- Validates port is in valid range (1024-65535)
- Prevents using privileged ports

**RegistryUrl** - Complete registry URL
- Constructed from host + port
- Provides formatted string for display

**ContainerName** - Registry container identifier
- Validates name format
- Default: "forester-registry"

**VolumeName** - Docker volume identifier
- Validates volume name format
- Default: "forester-registry-data"

**ImageRef** - OCI image reference
- Default: "registry:2"
- Validates format

**RegistryState** - Enum representing container state
- NotCreated
- Stopped
- Running { url: RegistryUrl }

**RegistryConfig** - Configuration for registry
- port: RegistryPort
- container_name: ContainerName
- volume_name: VolumeName
- image: ImageRef

**RegistryStatus** - Complete status information
- state: RegistryState
- volume_exists: bool

### Configuration (config.rs)

**ForesterConfig** - Top-level configuration
- registry: RegistryConfig
- Loaded from environment with `envy`
- Supports `.env` file via `dotenvy`

**Environment Variables:**
- `FORESTER_REGISTRY_PORT` (default: 5000)
- `FORESTER_REGISTRY_CONTAINER_NAME` (default: "forester-registry")
- `FORESTER_REGISTRY_VOLUME_NAME` (default: "forester-registry-data")
- `FORESTER_REGISTRY_IMAGE` (default: "registry:2")

### Registry Operations (registry/mod.rs)

Public functions (not methods):
- `start(config: &RegistryConfig) -> Result<RegistryUrl, RegistryError>`
- `stop(config: &RegistryConfig) -> Result<(), RegistryError>`
- `status(config: &RegistryConfig) -> Result<RegistryStatus, RegistryError>`
- `clean(config: &RegistryConfig) -> Result<(), RegistryError>`
- `logs(config: &RegistryConfig, follow: bool) -> Result<(), RegistryError>`

### Docker Interaction (registry/docker.rs)

Internal functions for Docker CLI operations:
- Check docker availability
- Container existence and state checks
- Container lifecycle (create, start, stop, remove)
- Volume operations
- Logs retrieval

### Health Checking (registry/health.rs)

Functions for registry health verification:
- `check_ready(url: &RegistryUrl) -> Result<(), HealthError>`
- `wait_until_ready(url: &RegistryUrl, timeout: Duration) -> Result<(), HealthError>`

### Error Types

**RegistryError** - Using snafu (defined in registry/mod.rs)
- DockerNotFound
- DockerNotRunning
- PortInUse { port: RegistryPort }
- ContainerStartFailed
- HealthCheckTimeout
- PermissionDenied
- VolumeRemovalFailed

**HealthError** - Health check errors (defined in registry/health.rs)
- Timeout
- ConnectionFailed
- UnexpectedStatus

### Key Design Principles

1. **No separate error module** - Errors defined where they're used (per style guide)
2. **Strong typing** - Port, URL, names all have dedicated types with validation
3. **Validation at construction** - Invalid states are unrepresentable
4. **Functional style** - Functions operate on domain types, not methods on structs
5. **No Manager/Service types** - Avoid generic names, use specific domain functions
6. **Builder pattern via bon** - For complex types like RegistryConfig

## Prerequisites

- [ ] Read `planning/forester-overview.md` for context
- [ ] Review the Rust style guide in context
- [ ] Examine `tests/registry_integration.rs` to understand expected behavior

## Phase 1: Project Structure and Configuration

- [x] **Convert forester to a library with binary**
  - Create `src/lib.rs` as the main library entry point
  - Move CLI logic from `src/main.rs` to library functions
  - Keep `src/main.rs` as a thin wrapper that calls library functions
  - Verify: `cargo build` succeeds

- [x] **Add required dependencies to Cargo.toml**
  - Add `argh` for CLI parsing
  - Add `snafu` for error handling
  - Add `dotenvy` for .env file support
  - Add `envy` for environment variable deserialization
  - Add `serde` with derive feature
  - Add `nutype` with serde feature for validated newtypes
  - Add `bon` for builder pattern
  - Add `reqwest` with blocking feature for health checks
  - Verify: `cargo build` succeeds

- [x] **Create configuration module (src/config.rs)**
  - Implement `load_config()` function using `envy` and `dotenvy`
  - Use snafu's `ResultExt` and `.context()` for error handling
  - Support `.env` file loading with `dotenvy`
  - Deserialize directly into `RegistryConfig` using nutype's serde support
  - Verify: Can instantiate config with defaults

## Phase 2: Domain Types

- [x] **Create registry module structure**
  - Create `src/registry/mod.rs`
  - Create `src/registry/types.rs`
  - Export public types from `mod.rs`
  - Verify: Module structure compiles

- [x] **Implement RegistryPort newtype (in types.rs)**
  - Use `nutype` with validation (>= 1024)
  - Derive Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize
  - Implement Default (5000)
  - Verify: Can create valid ports, rejects invalid ones

- [x] **Implement ContainerName newtype (in types.rs)**
  - Use `nutype` with not_empty validation
  - Derive Debug, Clone, PartialEq, Eq, Serialize, Deserialize
  - Implement Default ("forester-registry")
  - Verify: Creates valid container names

- [x] **Implement VolumeName newtype (in types.rs)**
  - Use `nutype` with not_empty validation
  - Derive Debug, Clone, PartialEq, Eq, Serialize, Deserialize
  - Implement Default ("forester-registry-data")
  - Verify: Creates valid volume names

- [x] **Implement ImageRef newtype (in types.rs)**
  - Use `nutype` with not_empty validation
  - Derive Debug, Clone, PartialEq, Eq, Serialize, Deserialize
  - Implement Default ("registry:2")
  - Verify: Creates valid image references

- [x] **Implement RegistryUrl type (in types.rs)**
  - Create struct with host and port
  - Implement bon builder pattern
  - Implement `Display` trait for formatted output (without scheme)
  - Verify: Formats URLs correctly (e.g., "localhost:5000")

- [x] **Implement RegistryState enum (in types.rs)**
  - Create enum: NotCreated, Stopped, Running { url: RegistryUrl }
  - Derive Debug, PartialEq, Eq, Clone
  - Verify: Can represent all states

- [x] **Implement RegistryConfig struct (in types.rs)**
  - Create struct with port, container_name, volume_name, image fields
  - Implement builder pattern using `bon` crate with `#[builder(on(_, into))]`
  - Derive Default, Serialize, Deserialize
  - Add `#[serde(default)]` for environment variable deserialization
  - Verify: Can build config with builder and deserialize from env vars

## Phase 3: Docker Interaction

- [ ] **Create docker module (src/registry/docker.rs)**
  - Create module structure
  - Add error types for docker operations using snafu
  - Verify: Module compiles

- [ ] **Implement docker availability check**
  - Function to check if docker CLI is available
  - Function to check if docker daemon is running
  - Return appropriate errors if not available
  - Verify: Detects docker correctly

- [ ] **Implement container existence check**
  - Function to check if container exists (any state)
  - Use `docker ps -a --filter name=<name> --format json`
  - Parse output to determine existence
  - Verify: Correctly detects existing/non-existing containers

- [ ] **Implement container state check**
  - Function to get current container state (running/stopped)
  - Use `docker inspect <name>`
  - Parse JSON output for state
  - Return RegistryState enum
  - Verify: Correctly identifies container states

- [ ] **Implement container start operation**
  - Function to start existing stopped container
  - Use `docker start <name>`
  - Handle errors appropriately
  - Verify: Can start stopped containers

- [ ] **Implement container create and run operation**
  - Function to create and run new container
  - Use `docker run -d --name <name> -p <port>:5000 -v <volume>:/var/lib/registry --restart unless-stopped <image>`
  - Handle port conflicts with specific error
  - Verify: Creates and starts new containers

- [ ] **Implement container stop operation**
  - Function to stop running container
  - Use `docker stop <name>`
  - Handle case where container doesn't exist
  - Verify: Stops running containers gracefully

- [ ] **Implement container remove operation**
  - Function to remove container
  - Use `docker rm <name>`
  - Handle case where container doesn't exist
  - Verify: Removes containers

- [ ] **Implement volume remove operation**
  - Function to remove volume
  - Use `docker volume rm <name>`
  - Handle case where volume doesn't exist or is in use
  - Verify: Removes volumes

- [ ] **Implement logs retrieval**
  - Function to get container logs
  - Use `docker logs <name>` (with optional `--follow`)
  - Stream output to stdout
  - Verify: Shows container logs

## Phase 4: Health Checking

- [ ] **Create health module (src/registry/health.rs)**
  - Add `reqwest` with blocking feature to dependencies
  - Create module structure
  - Add error types using snafu
  - Verify: Module compiles

- [ ] **Implement single health check**
  - Function to check if registry responds at URL
  - HTTP GET to `<url>/v2/`
  - Return Ok if 200 response
  - Verify: Detects healthy registry

- [ ] **Implement wait for health**
  - Function to poll health check with timeout
  - Retry with exponential backoff
  - Return error if timeout exceeded
  - Verify: Waits for registry to become healthy

## Phase 5: Registry Operations

- [ ] **Implement registry start (in src/registry/mod.rs)**
  - Check docker availability
  - Check if container exists and state
  - If running: return URL (idempotent)
  - If stopped: start container
  - If not exists: create and run container
  - Wait for health check
  - Return registry URL
  - Verify: Passes integration test `test_registry_start_idempotent`

- [ ] **Implement registry stop (in src/registry/mod.rs)**
  - Check if container exists
  - If running: stop container
  - If not running: succeed (idempotent)
  - Verify: Passes integration tests `test_registry_stop` and `test_registry_stop_idempotent`

- [ ] **Implement registry status (in src/registry/mod.rs)**
  - Check container state
  - Return RegistryStatus with state and URL if running
  - Exit code 0 if running, non-zero otherwise
  - Verify: Passes integration tests `test_registry_status_not_created` and `test_registry_status_running`

- [ ] **Implement registry clean (in src/registry/mod.rs)**
  - Stop container if running
  - Remove container
  - Remove volume
  - Verify: Passes integration test `test_registry_clean`

- [ ] **Implement registry logs (in src/registry/mod.rs)**
  - Check if container exists
  - Stream logs to stdout
  - Support follow flag
  - Verify: Passes integration test `test_registry_logs`

## Phase 6: CLI Integration

- [ ] **Update main.rs with argh CLI parsing**
  - Define command structs using argh
  - Parse registry subcommands (start, stop, status, clean, logs)
  - Verify: CLI parsing works

- [ ] **Wire up registry start command**
  - Load config from environment
  - Call `registry::start()`
  - Print output
  - Handle errors with user-friendly messages
  - Verify: `forester registry start` works

- [ ] **Wire up registry stop command**
  - Load config
  - Call `registry::stop()`
  - Print confirmation
  - Verify: `forester registry stop` works

- [ ] **Wire up registry status command**
  - Load config
  - Call `registry::status()`
  - Print formatted status
  - Exit with appropriate code
  - Verify: `forester registry status` works

- [ ] **Wire up registry clean command**
  - Load config
  - Call `registry::clean()`
  - Print confirmation
  - Verify: `forester registry clean` works

- [ ] **Wire up registry logs command**
  - Load config
  - Parse follow flag
  - Call `registry::logs()`
  - Verify: `forester registry logs` works

## Phase 7: Testing and Polish

- [ ] **Run all integration tests**
  - Build release binary: `cargo build --release`
  - Run tests: `cargo test --test registry_integration`
  - Verify: All tests pass

- [ ] **Test custom port configuration**
  - Create `.env` file with `FORESTER_REGISTRY_PORT=5001`
  - Test all commands with custom port
  - Verify: Uses custom port correctly

- [ ] **Add error message improvements**
  - Ensure all errors have helpful messages
  - Add suggestions for common issues (docker not running, port in use, etc.)
  - Verify: Error messages are clear and actionable

- [ ] **Run cargo fmt and cargo clippy**
  - Format code: `cargo fmt`
  - Check lints: `cargo clippy`
  - Fix any warnings
  - Verify: No warnings remain

- [ ] **Update local-registry skill documentation**
  - Verify skill matches actual behavior
  - Update any discrepancies
  - Add examples of custom port usage

## Completion Criteria

All integration tests pass:
```bash
cargo build --release
cargo test --test registry_integration
```

Manual verification:
```bash
./target/release/forester registry start
./target/release/forester registry status
./target/release/forester registry logs
./target/release/forester registry stop
./target/release/forester registry clean
```
