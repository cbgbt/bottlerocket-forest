# Test Plan: Jobserver Proxy

## Overview

This test plan verifies the jobserver proxy feature that tunnels GNU Make jobserver tokens through the Docker boundary. Tests cover the server (token pool management, client connections), client (FIFO proxy, MAKEFLAGS export), and their integration.

## Test Types

- **Unit**: Test internal logic in isolation, mocks allowed
- **Integration**: Test actual socket/FIFO communication, NO mocks
- **Not testable**: Cannot be verified by automated test
- **Out of scope**: Requires Docker/container environment beyond unit test scope

## Requirements Coverage

| Req ID | Test Type | Test Name | Description |
|--------|-----------|-----------|-------------|
| JSP-1 | integration | test_server_creates_abstract_socket | Server creates abstract socket with unique name |
| JSP-2 | integration | test_server_connects_external_jobserver | Server proxies tokens from external MAKEFLAGS jobserver |
| JSP-3 | unit | test_token_pool_standalone_creation | TokenPool creates configurable number of tokens |
| JSP-4 | unit | test_token_pool_acquire_blocks | Acquire blocks when pool empty, returns when available |
| JSP-4a | unit | test_acquire_timeout | Acquire returns timeout error after configured duration |
| JSP-5 | unit | test_token_pool_release | Release returns token to pool |
| JSP-5a | unit | test_client_connection_tracks_held_tokens | ClientConnection tracks tokens per acquire/release |
| JSP-5b | unit | test_invalid_release_rejected | Release of unheld token returns error |
| JSP-6 | integration | test_server_multiple_clients | Server handles multiple concurrent client connections |
| JSP-7 | integration | test_client_connects_socket | Client connects to abstract socket |
| JSP-8 | integration | test_client_creates_fifo | Client creates FIFO at /tmp/jobserver-fifo |
| JSP-9 | integration | test_client_outputs_makeflags | Client outputs correct MAKEFLAGS value |
| JSP-10 | integration | test_fifo_read_proxies_acquire | FIFO read triggers acquire request to server |
| JSP-10a | integration | test_consumer_liveness_warning | Warning logged after FIFO inactivity timeout |
| JSP-11 | integration | test_fifo_write_proxies_release | FIFO write triggers release request to server |
| JSP-12 | out-of-scope | - | Docker build argument passing requires container runtime |
| JSP-13 | out-of-scope | - | Container network mode requires Docker |
| JSP-NFR-1 | integration | test_acquire_latency_under_1ms | Token acquisition completes in <1ms when available |
| JSP-NFR-2 | integration | test_server_100_concurrent_clients | Server supports 100+ concurrent connections |
| JSP-NFR-3 | integration | test_graceful_shutdown | Server completes pending ops before exit on SIGTERM |
| JSP-NFR-4 | unit | test_pool_exhaustion_logging | Debug log emitted when pool exhausted |
| JSP-ERR-1 | integration | test_client_disconnect_reclaims_tokens | Tokens returned to pool when client disconnects |
| JSP-ERR-1a | integration | test_server_disconnect_client_exits | Client exits non-zero and removes FIFO on server disconnect |
| JSP-ERR-2 | integration | test_client_exits_on_missing_socket | Client exits with error when socket doesn't exist |
| JSP-ERR-2a | integration | test_external_jobserver_disconnect | Server logs error, fails new acquires on upstream disconnect |
| JSP-ERR-3 | integration | test_client_exits_on_fifo_failure | Client exits with error when FIFO creation fails |
| JSP-ERR-4 | integration | test_malformed_request_closes_connection | Server closes connection on protocol error |
| JSP-ERR-5 | integration | test_fifo_consumer_death_releases_token | Orphaned token released on FIFO reader death |
| JSP-ERR-6 | integration | test_client_cleanup_on_exit | FIFO removed and tokens released on any exit path |
| JSP-ERR-7 | integration | test_socket_collision_retry | Server retries with new token on EADDRINUSE |

## Critical Constraints Verification

| CC ID | Verification Approach | Test Name(s) |
|-------|----------------------|---------------|
| CC-1 | Verify tokens reclaimed on connection drop | test_client_disconnect_reclaims_tokens, test_client_connection_tracks_held_tokens |
| CC-2 | Verify socket name format | test_server_creates_abstract_socket |
| CC-3 | Verify FIFO path is /tmp/jobserver-fifo | test_client_creates_fifo |
| CC-4 | Verify protocol uses single-byte commands | test_fifo_read_proxies_acquire, test_fifo_write_proxies_release |
| CC-5 | Verify pending ops complete on shutdown | test_graceful_shutdown |
| CC-6 | Verify timeout on acquire | test_acquire_timeout |
| CC-7 | Verify invalid release rejected | test_invalid_release_rejected |
| CC-8 | Verify orphaned token release | test_fifo_consumer_death_releases_token |
| CC-9 | Verify FIFO cleanup on all exits | test_client_cleanup_on_exit, test_server_disconnect_client_exits |

## Integration Test Requirements

Integration tests MUST:
- Exercise actual `jobsys serve` and `jobsys client` binaries
- Use real abstract Unix domain sockets
- Use real FIFOs for client tests
- NOT mock socket or FIFO operations

## Test Implementation Notes

### Unit Tests (server.rs, client.rs, lib.rs)

**TokenPool tests:**
- Create pool with N tokens, verify N acquires succeed
- Verify acquire blocks when empty (use timeout to detect)
- Verify release makes token available for next acquire
- Verify timeout error returned after configured duration

**ClientConnection tests:**
- Track held tokens across acquire/release calls
- Verify Drop impl releases all held tokens
- Verify release of unheld token returns NotHeld error

**Protocol tests:**
- Parse valid acquire/release messages
- Reject malformed messages

### Integration Tests

**Server tests:**
- Spawn server, verify socket exists at expected abstract address
- Connect multiple clients, verify fair token distribution
- Kill client connection, verify tokens reclaimed (check pool size)
- Send SIGTERM, verify in-flight acquire completes before exit

**Client tests:**
- Start server, start client, verify FIFO created
- Read from FIFO, verify token received from server
- Write to FIFO, verify token returned to server
- Kill server, verify client exits and FIFO removed
- Simulate FIFO reader death (close read end), verify token released

**End-to-end tests:**
- Server + client + subprocess running `make -j` with MAKEFLAGS
- Verify parallelism matches token count

## Standalone Docker Integration Test

A standalone test that proves the full jobserver proxy loop works with real GNU Make and Docker, independent of twoliter.

### Test: `test_real_make_docker_integration`

**Purpose:** Validate that tokens flow correctly from host jobserver → jobsys server → Docker container → jobsys client → GNU Make inside container.

**Setup:**
```
tests/integration/
├── docker-jobserver-test/
│   ├── Dockerfile          # Minimal image with make, jobsys client binary
│   ├── Makefile            # Parallel targets that sleep and log
│   └── run-test.sh         # Orchestrates the test
```

**Dockerfile:**
```dockerfile
FROM alpine:latest
RUN apk add --no-cache make
COPY jobsys /usr/local/bin/
COPY Makefile /work/
WORKDIR /work
```

**Makefile (inside container):**
```makefile
# Targets that prove parallelism by logging timestamps
all: a b c d

a b c d:
	@echo "$@ start $$(date +%s.%N)"
	@sleep 0.5
	@echo "$@ end $$(date +%s.%N)"
```

**Test procedure:**
1. Build test Docker image with jobsys client binary
2. Start `jobsys serve --socket @test-jobserver --tokens 2`
3. Run Docker container:
   ```bash
   docker run --rm --net host      -e JOBSERVER_SOCKET=test-jobserver      test-image      sh -c 'eval $(jobsys client --socket @$JOBSERVER_SOCKET) && make -j'
   ```
4. Parse output timestamps to verify:
   - Exactly 2 targets run in parallel (not 1, not 4)
   - Total time ≈ 1.0s (2 batches of 2 × 0.5s), not 2.0s (serial) or 0.5s (unlimited)

**Assertions:**
- Exit code 0
- Parallelism matches token count (2)
- All 4 targets complete
- Tokens returned to pool after completion

**Variants:**
- `test_real_make_docker_token_exhaustion`: Use 1 token, verify serial execution
- `test_real_make_docker_container_crash`: Kill container mid-build, verify tokens reclaimed
- `test_real_make_docker_nested_make`: Makefile that invokes sub-make, verify token sharing

**Requirements covered:**
- JSP-12 (Docker build argument - via env var)
- JSP-13 (Container network mode)
- Full end-to-end token flow

**Why standalone:**
- Fast feedback (seconds, not minutes)
- No twoliter/buildsys dependencies
- Easy to run in CI
- Proves the core mechanism works before integration
