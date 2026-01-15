# Jobserver Proxy - Implementation Plan

## Commit Checklist

High-level tracking of implementation progress.
Each commit should be atomic, buildable, and tested.

### Phase 1-3: Core Implementation (Complete)

- [x] **Commit 1**: Add jobsys crate with core types and protocol
- [x] **Commit 2**: Implement TokenPool and ClientConnection
- [x] **Commit 3**: Implement jobsys server
- [x] **Commit 4**: Implement jobsys client with FIFO proxy
- [x] **Commit 5**: Integrate jobsys into buildsys
- [x] **Commit 6**: Update Dockerfile to use jobsys client
- [x] **Commit 7**: Add integration tests
- [x] **Commit 8**: Add standalone Docker integration test

### Phase 4: Twoliter Integration (Complete)

- [x] **Commit 9**: Add external jobserver support to jobsys server
- [x] **Commit 10**: Add jobserver support to twoliter
- [x] **Commit 11**: Update buildsys to inherit jobserver
- [x] **Commit 12**: Remove CARGO_BUILD_JOBS workaround

## Dependency Graph

```
Phase 1-3 (complete):
Commit 1 → Commit 2 → Commit 3 → Commit 4 ─┬→ Commit 5 → Commit 6
                                            ├→ Commit 7
                                            └→ Commit 8

Phase 4 (pending):
                                Commit 9 ─┬→ Commit 10 → Commit 11 → Commit 12
                                          │
                                          └→ (can test independently)
```

## Parallelization Notes

### Phase 1-3 (Complete)
- Commits 1-4 were sequential (each depends on prior)
- Commits 5, 7, and 8 were developed in parallel after Commit 4
- Commit 6 depended on Commit 5

### Phase 4 (Pending)
- **Commit 9** (external jobserver in jobsys): Can start immediately, no dependencies on Phase 1-3 changes
- **Commit 10** (twoliter jobserver): Depends on Commit 9 (needs server to support proxy mode)
- **Commit 11** (buildsys inheritance): Depends on Commit 10 (needs MAKEFLAGS to be set by twoliter)
- **Commit 12** (cleanup): Depends on Commit 11 (remove workaround after proper solution in place)

**Parallelization opportunity**: Commits 9 and 10 touch different codebases (jobsys vs twoliter), so they could potentially be developed in parallel if Commit 10 is written to assume Commit 9's interface. However, testing requires them to be integrated, so sequential development with immediate integration testing is recommended.

## Commit Details

### Phase 1: Foundation

Commits that establish the groundwork.
Later phases depend on these being complete.

---

#### Commit 1: Add jobsys crate with core types and protocol

**Summary**: Create the jobsys crate structure following the pipesys pattern. Define core types (TokenPool, errors) and wire protocol constants. This establishes the foundation for both server and client implementations.

**Files Changed**:
- `twoliter/Cargo.toml` - Add jobsys to workspace members and dependencies
- `twoliter/tools/jobsys/Cargo.toml` - New crate manifest
- `twoliter/tools/jobsys/src/lib.rs` - Core types and error definitions
- `twoliter/tools/jobsys/src/protocol.rs` - Wire protocol constants
- `twoliter/tools/jobsys/src/main.rs` - CLI skeleton with serve/client subcommands

**Key Changes**:
- Add workspace dependency: `jobsys = { version = "0.1", path = "tools/jobsys", lib = true, artifact = [ "bin:jobsys" ] }`
- Define `TokenPool` struct with `acquire()` and `release()` method signatures
- Define error types: `ServerError`, `ClientError`, `AcquireError`, `ReleaseError`, `ProtocolError`
- Define protocol constants: `CMD_ACQUIRE = 0x41`, `CMD_RELEASE = 0x52`, `RESP_TOKEN = 0x54`, `RESP_OK = 0x4B`, `RESP_ERROR = 0x45`
- CLI structure with `serve` and `client` subcommands (stubs)

**Requirements Addressed**: None directly (foundation)

**Constraints Addressed**:
- CC-4: Protocol MUST use single-byte commands with blocking semantics

**Testing**: `cargo check -p jobsys` passes. Unit tests for protocol constants.

**Dependencies**: None (first commit)

---

#### Commit 2: Implement TokenPool and ClientConnection

**Summary**: Implement the token pool with blocking acquire/release semantics and per-connection token tracking. This is the core resource management logic that ensures tokens are never lost.

**Files Changed**:
- `twoliter/tools/jobsys/src/lib.rs` - TokenPool and ClientConnection implementations
- `twoliter/tools/jobsys/src/pool.rs` - TokenPool implementation (if split)

**Key Changes**:
- `TokenPool::new(count: usize)` - Create pool with N tokens (bytes 0..N)
- `TokenPool::acquire(&self) -> Result<u8, AcquireError>` - Blocking acquire with timeout
- `TokenPool::release(&self, token: u8)` - Return token to pool
- `ClientConnection` struct tracking `held_tokens: HashSet<u8>`
- `ClientConnection::acquire()` - Acquire and track
- `ClientConnection::release(token)` - Validate held, release, untrack
- `Drop` impl for `ClientConnection` - Release all held tokens

**Requirements Addressed**:
- JSP-3: Standalone token pool creation
- JSP-4: Token acquisition blocks until available
- JSP-4a: Acquire timeout
- JSP-5: Token release
- JSP-5a: Per-client token tracking
- JSP-5b: Invalid release rejection

**Acceptance Criteria** *(CC-1, CC-6, CC-7)*:
- [ ] ClientConnection tracks tokens per acquire/release
- [ ] Drop impl releases all held tokens to pool
- [ ] Acquire has configurable timeout (default 30 min)
- [ ] Release of unheld token returns `ReleaseError::NotHeld`

**Anti-patterns**:
- Trusting clients to release tokens before exit (CC-1)
- Unbounded blocking on token acquisition (CC-6)
- Silently accepting releases of unheld tokens (CC-7)

**Testing**: Unit tests:
- `test_token_pool_standalone_creation` (JSP-3)
- `test_token_pool_acquire_blocks` (JSP-4)
- `test_acquire_timeout` (JSP-4a)
- `test_token_pool_release` (JSP-5)
- `test_client_connection_tracks_held_tokens` (JSP-5a)
- `test_invalid_release_rejected` (JSP-5b)
- `test_pool_exhaustion_logging` (JSP-NFR-4)

**Dependencies**: Commit 1

---

### Phase 2: Core Implementation

Commits that implement the main functionality.

---

#### Commit 3: Implement jobsys server

**Summary**: Implement the server that listens on an abstract Unix domain socket, manages the token pool, and handles multiple concurrent client connections. Follows the pipesys server pattern but with request-response protocol instead of FD passing.

**Files Changed**:
- `twoliter/tools/jobsys/src/server.rs` - Server implementation
- `twoliter/tools/jobsys/src/lib.rs` - Export server module
- `twoliter/tools/jobsys/src/main.rs` - Wire up serve subcommand

**Key Changes**:
- `Server` struct with socket name, token count, optional external jobserver
- `Server::serve()` async method - main event loop
- Abstract socket creation with `@buildsys-jobs-{token}` format
- Per-connection task spawning with `ClientConnection`
- Protocol handling: parse command byte, dispatch to acquire/release
- Graceful shutdown on SIGTERM (complete pending ops)
- Socket collision retry (up to 3 attempts with new random token)
- External jobserver connection via MAKEFLAGS parsing (optional)

**Requirements Addressed**:
- JSP-1: Server socket creation
- JSP-2: External jobserver connection
- JSP-6: Multiple client support
- JSP-NFR-2: 100+ concurrent connections
- JSP-NFR-3: Graceful shutdown
- JSP-ERR-1: Client disconnect recovery
- JSP-ERR-4: Protocol error handling
- JSP-ERR-7: Socket name collision retry

**Acceptance Criteria** *(CC-1, CC-2, CC-5)*:
- [ ] Socket name uses `@buildsys-jobs-{token}` format
- [ ] Tokens reclaimed when client connection drops
- [ ] Pending operations complete before shutdown on SIGTERM

**Anti-patterns**:
- Hardcoded socket names causing collisions (CC-2)
- Immediate exit dropping in-flight requests (CC-5)

**Testing**: 
- `test_server_creates_abstract_socket` (JSP-1)
- `test_server_connects_external_jobserver` (JSP-2)
- `test_server_multiple_clients` (JSP-6)
- `test_server_100_concurrent_clients` (JSP-NFR-2)
- `test_graceful_shutdown` (JSP-NFR-3)
- `test_client_disconnect_reclaims_tokens` (JSP-ERR-1)
- `test_malformed_request_closes_connection` (JSP-ERR-4)
- `test_socket_collision_retry` (JSP-ERR-7)

**Dependencies**: Commits 1, 2

---

#### Commit 4: Implement jobsys client with FIFO proxy

**Summary**: Implement the client that connects to the server, creates a FIFO at `/tmp/jobserver-fifo`, and proxies FIFO read/write operations to server acquire/release requests. Outputs MAKEFLAGS for shell eval.

**Files Changed**:
- `twoliter/tools/jobsys/src/client.rs` - Client implementation
- `twoliter/tools/jobsys/src/lib.rs` - Export client module
- `twoliter/tools/jobsys/src/main.rs` - Wire up client subcommand

**Key Changes**:
- `Client` struct with socket name, FIFO path
- Connect to abstract socket
- Create FIFO at `/tmp/jobserver-fifo` with `mkfifo`
- Output `MAKEFLAGS=--jobserver-auth=fifo:/tmp/jobserver-fifo`
- FIFO read → send acquire request → write token to FIFO reader
- FIFO write → read token → send release request
- Track held tokens for orphan detection
- Liveness warning after 5 min FIFO inactivity
- Signal handlers (SIGTERM, SIGINT) for cleanup
- FIFO removal on any exit path (Drop or scopeguard)
- Handle FIFO consumer death (EPIPE) → release orphaned token

**Requirements Addressed**:
- JSP-7: Socket connection
- JSP-8: FIFO creation
- JSP-9: MAKEFLAGS export
- JSP-10: Read proxy (acquire)
- JSP-10a: Consumer liveness detection
- JSP-11: Write proxy (release)
- JSP-ERR-1a: Server disconnect handling
- JSP-ERR-2: Server unavailable
- JSP-ERR-3: FIFO creation failure
- JSP-ERR-5: FIFO consumer death
- JSP-ERR-6: Client cleanup

**Acceptance Criteria** *(CC-3, CC-8, CC-9)*:
- [ ] FIFO created at `/tmp/jobserver-fifo`
- [ ] Orphaned token released on FIFO reader death (EPIPE)
- [ ] FIFO removed on any exit path (normal, signal, crash)

**Anti-patterns**:
- Random or configurable FIFO paths requiring coordination (CC-3)
- Holding tokens for dead consumers (CC-8)
- Leaving FIFO on crash/signal (CC-9)

**Testing**:
- `test_client_connects_socket` (JSP-7)
- `test_client_creates_fifo` (JSP-8)
- `test_client_outputs_makeflags` (JSP-9)
- `test_fifo_read_proxies_acquire` (JSP-10)
- `test_consumer_liveness_warning` (JSP-10a)
- `test_fifo_write_proxies_release` (JSP-11)
- `test_server_disconnect_client_exits` (JSP-ERR-1a)
- `test_client_exits_on_missing_socket` (JSP-ERR-2)
- `test_client_exits_on_fifo_failure` (JSP-ERR-3)
- `test_fifo_consumer_death_releases_token` (JSP-ERR-5)
- `test_client_cleanup_on_exit` (JSP-ERR-6)

**Dependencies**: Commits 1, 2, 3

---

### Phase 3: Integration

Commits that wire jobsys into the build system.

---

#### Commit 5: Integrate jobsys into buildsys

**Summary**: Add jobsys server spawning to buildsys following the pipesys pattern. Generate unique socket names, spawn server before builds, pass socket name as Docker build argument, clean up after builds.

**Files Changed**:
- `twoliter/tools/buildsys/Cargo.toml` - Add jobsys dependency
- `twoliter/tools/buildsys/src/builder.rs` - Spawn jobsys server, pass socket arg

**Key Changes**:
- Add `jobsys` dependency to buildsys
- Add `jobserver_socket` field to `CommonBuildArgs`
- Generate socket name: `buildsys-jobs-{token}-{nocache}`
- Spawn `jobsys serve` in tokio runtime (like PipesysServer)
- Add `--build-arg JOBSERVER_SOCKET={socket}` to docker build
- Determine token count from `num_cpus::get()` or environment variable
- Clean up server on build completion

**Requirements Addressed**:
- JSP-12: Docker build argument passing

**Testing**: Manual verification that jobsys server starts and socket name is passed to Docker.

**Dependencies**: Commits 1-4

---

#### Commit 6: Update Dockerfile to use jobsys client

**Summary**: Add jobsys client invocation to build stages that run rpmbuild. The client creates the FIFO and exports MAKEFLAGS so rpmbuild/make can use the jobserver.

**Files Changed**:
- `twoliter/twoliter/embedded/build.Dockerfile` - Add jobsys client setup in rpmbuild stage

**Key Changes**:
- Add `ARG JOBSERVER_SOCKET` to rpmbuild stage
- Mount jobsys binary: `-v {root}/build/tools/jobsys:/usr/local/bin/jobsys:ro`
- Before rpmbuild: `eval $(jobsys client --socket $JOBSERVER_SOCKET)` to set MAKEFLAGS
- Requires `--net host` (already present for pipesys)

**Requirements Addressed**:
- JSP-13: Container network mode (already satisfied by existing --net host)

**Testing**: Manual build verification that rpmbuild respects jobserver parallelism.

**Dependencies**: Commit 5

---

#### Commit 7: Add integration tests

**Summary**: Add integration tests that exercise the full server-client communication, token lifecycle, and error handling paths.

**Files Changed**:
- `twoliter/tools/jobsys/tests/integration.rs` - Integration test suite

**Key Changes**:
- Test server + client over real abstract socket
- Test multiple clients sharing token pool
- Test client disconnect reclaims tokens
- Test FIFO read/write proxies correctly
- Test acquire latency under 1ms (JSP-NFR-1)
- Test external jobserver disconnect handling (JSP-ERR-2a)

**Requirements Addressed**:
- JSP-NFR-1: Token acquisition latency

**Testing**: `cargo test -p jobsys --test integration`

**Dependencies**: Commits 1-4

---

#### Commit 8: Add standalone Docker integration test

**Summary**: A standalone test proving the full jobserver proxy loop works with real GNU Make and Docker, independent of twoliter. Fast feedback, easy CI, validates core mechanism before full integration.

**Files Changed**:
- `twoliter/tools/jobsys/tests/docker-integration/Dockerfile` - Minimal Alpine image with make + jobsys
- `twoliter/tools/jobsys/tests/docker-integration/Makefile` - Parallel targets that log timestamps
- `twoliter/tools/jobsys/tests/docker-integration/run-test.sh` - Test orchestration script
- `twoliter/tools/jobsys/tests/docker_integration.rs` - Rust test harness

**Dockerfile**:
```dockerfile
FROM alpine:latest
RUN apk add --no-cache make
COPY jobsys /usr/local/bin/
COPY Makefile /work/
WORKDIR /work
```

**Makefile (inside container)**:
```makefile
all: a b c d

a b c d:
	@echo "$@ start $$(date +%s.%N)"
	@sleep 0.5
	@echo "$@ end $$(date +%s.%N)"
```

**Test procedure**:
1. Build jobsys binary
2. Build test Docker image with jobsys client
3. Start `jobsys serve --socket @test-jobserver --tokens 2`
4. Run container with `--net host`, invoke `jobsys client` + `make -j`
5. Parse timestamps to verify exactly 2 targets run in parallel
6. Assert total time ≈ 1.0s (2 batches × 0.5s), not 2.0s (serial) or 0.5s (unlimited)
7. Verify tokens returned to pool

**Test variants**:
- `test_docker_single_token` - 1 token → serial execution
- `test_docker_container_crash` - Kill container mid-build → tokens reclaimed
- `test_docker_nested_make` - Sub-make invocation → token sharing

**Requirements Addressed**:
- JSP-12: Docker build argument (via env var)
- JSP-13: Container network mode
- Full end-to-end validation

**Testing**: `cargo test -p jobsys --test docker_integration` (requires Docker)

**Dependencies**: Commits 1-4

---

## Open Questions

- [ ] Should jobsys support connecting to an external jobserver via MAKEFLAGS, or only standalone mode initially? (Design says yes, but could defer)
- [ ] What's the default token count? `num_cpus::get()` or configurable via env var?
- [ ] Should the client run in foreground (blocking) or background (daemonize like pipesys link)?


## Phase 4: Twoliter Integration

Commits that complete the end-to-end jobserver coordination.

---

#### Commit 9: Add external jobserver support to jobsys server

**Summary**: Implement JSP-2 - when MAKEFLAGS contains jobserver info, connect to the external jobserver and proxy tokens instead of creating a standalone pool.

**Files Changed**:
- `twoliter/tools/jobsys/Cargo.toml` - Add `jobserver` crate dependency
- `twoliter/tools/jobsys/src/server.rs` - Add proxy mode support
- `twoliter/tools/jobsys/src/lib.rs` - Add external jobserver types

**Key Changes**:
- Parse MAKEFLAGS for `--jobserver-auth=fifo:/path` or `--jobserver-auth=R,W`
- Connect to external jobserver using `jobserver` crate
- In proxy mode: acquire from external on client request, release to external on client release
- Server tracks which tokens came from external vs local pool
- Graceful handling of external jobserver disconnect

**Requirements Addressed**:
- JSP-2: External jobserver connection
- JSP-17: Server external jobserver detection
- JSP-18: Server external token acquisition
- JSP-19: Server external token release

**Acceptance Criteria** *(CC-11)*:
- [ ] Server detects external jobserver from MAKEFLAGS
- [ ] Tokens proxied to/from external jobserver
- [ ] No standalone pool created in proxy mode
- [ ] Falls back to standalone mode if MAKEFLAGS not set

**Testing**:
- `test_server_detects_external_jobserver`
- `test_server_proxies_acquire`
- `test_server_proxies_release`
- `test_server_proxy_mode` (integration)

**Dependencies**: Commits 1-4

---

#### Commit 10: Add jobserver support to twoliter

**Summary**: Create a jobserver in twoliter before invoking cargo make, ensuring all parallelism is coordinated from the top level.

**Files Changed**:
- `twoliter/twoliter/Cargo.toml` - Add `jobserver` crate dependency
- `twoliter/twoliter/src/jobserver.rs` - New module for jobserver management
- `twoliter/twoliter/src/cargo_make.rs` - Set MAKEFLAGS before exec
- `twoliter/twoliter/src/cmd/build.rs` - Create jobserver before build

**Key Changes**:
- `JobserverGuard` struct that creates jobserver and sets MAKEFLAGS
- Guard implements Drop to clean up jobserver resources
- Token count from `TWOLITER_JOBS` env var or `num_cpus::get()`
- MAKEFLAGS set with `--jobserver-auth=fifo:/tmp/twoliter-jobserver-{pid}`
- All build commands (build-kit, build-variant, build-package) wrapped with jobserver

**Requirements Addressed**:
- JSP-14: Twoliter jobserver creation
- JSP-15: Twoliter MAKEFLAGS export
- JSP-16: Twoliter jobserver cleanup

**Acceptance Criteria** *(CC-10)*:
- [ ] Jobserver created before cargo make invocation
- [ ] MAKEFLAGS set with correct jobserver-auth
- [ ] Jobserver cleaned up on build completion (success or failure)
- [ ] Token count respects TWOLITER_JOBS or defaults to CPU count

**Testing**:
- `test_twoliter_creates_jobserver`
- `test_twoliter_sets_makeflags`
- `test_twoliter_jobserver_cleanup`

**Dependencies**: Commit 9

---

#### Commit 11: Update buildsys to inherit jobserver

**Summary**: Modify buildsys to detect and use inherited jobserver from MAKEFLAGS instead of always creating its own.

**Files Changed**:
- `twoliter/tools/buildsys/src/builder.rs` - Parse MAKEFLAGS, conditional server spawn

**Key Changes**:
- Parse MAKEFLAGS for jobserver socket info
- If found: use inherited socket, don't spawn jobsys server
- If not found: spawn jobsys server (current behavior), log warning
- Pass inherited or generated socket to Docker builds

**Requirements Addressed**:
- JSP-20: Buildsys jobserver socket inheritance
- JSP-21: Buildsys standalone fallback

**Acceptance Criteria** *(CC-12)*:
- [ ] Buildsys detects jobserver from MAKEFLAGS
- [ ] No jobsys server spawned when inheriting
- [ ] Warning logged when no jobserver inherited
- [ ] Docker builds receive correct socket name

**Testing**:
- `test_buildsys_inherits_jobserver`
- `test_buildsys_standalone_warning`

**Dependencies**: Commits 9, 10

---

#### Commit 12: Remove CARGO_BUILD_JOBS workaround

**Summary**: Remove the CARGO_BUILD_JOBS=1 settings now that proper jobserver coordination is in place.

**Files Changed**:
- `twoliter/twoliter/embedded/Makefile.toml` - Remove CARGO_BUILD_JOBS
- `twoliter/twoliter/embedded/build.Dockerfile` - Remove ENV CARGO_BUILD_JOBS

**Key Changes**:
- Remove `CARGO_BUILD_JOBS = "1"` from Makefile.toml
- Remove `ENV CARGO_BUILD_JOBS=1` from Dockerfile
- Cargo will now respect the jobserver via MAKEFLAGS

**Requirements Addressed**: Cleanup of temporary workaround

**Testing**: Full build test to verify parallelism works correctly

**Dependencies**: Commits 9-11

---

## Updated Dependency Graph

```
Phase 1-3 (existing):
Commit 1 → Commit 2 → Commit 3 → Commit 4 ─┬→ Commit 5 → Commit 6
                                            ├→ Commit 7
                                            └→ Commit 8

Phase 4 (new):
Commit 4 → Commit 9 → Commit 10 → Commit 11 → Commit 12
```

## Updated Open Questions

- [x] Should jobsys support connecting to an external jobserver via MAKEFLAGS? **Yes, implemented in Commit 9**
- [x] What's the default token count? **num_cpus::get(), overridable via TWOLITER_JOBS**
- [x] Should the client run in foreground or background? **Foreground, blocking until build completes**
