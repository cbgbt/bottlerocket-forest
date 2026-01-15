# Jobserver Proxy - Technical Design

## Overview

The jobserver proxy tunnels GNU Make jobserver tokens through the Docker boundary using abstract Unix domain sockets. A host-side server manages the token pool and communicates with container-side clients that present a standard FIFO interface to build tools.

This design reuses the abstract socket pattern from pipesys but implements a simpler request-response protocol rather than fd passing.

**Design Philosophy**: Minimal proxy layer that makes Docker builds appear to have native jobserver access. Build tools (make, cargo, rpmbuild) see a standard FIFO and work unchanged.

## Critical Constraints

| ID | Constraint | Rationale | Anti-pattern to avoid |
|----|------------|-----------|----------------------|
| CC-1 | Server MUST track tokens per-connection and reclaim on disconnect | Prevents token leaks when containers crash (JSP-ERR-1, JSP-5a) | Trusting clients to release tokens before exit |
| CC-2 | Abstract socket name MUST use `@buildsys-jobs-{token}` format | Enables multiple concurrent builds; matches pipesys pattern | Hardcoded socket names causing collisions |
| CC-3 | Client FIFO MUST be at `/tmp/jobserver-fifo` | Predictable path for Dockerfile integration | Random or configurable paths requiring coordination |
| CC-4 | Protocol MUST use single-byte commands with blocking semantics | Matches GNU Make jobserver behavior; simple implementation | Complex framing or async protocols |
| CC-5 | Server MUST complete pending operations before shutdown | Prevents token loss during graceful termination (JSP-NFR-3) | Immediate exit dropping in-flight requests |
| CC-6 | Acquire requests MUST have configurable timeout | Prevents infinite blocking in deadlock scenarios (JSP-4a) | Unbounded blocking on token acquisition |
| CC-7 | Invalid release attempts MUST be rejected | Prevents protocol corruption and token duplication (JSP-5b) | Silently accepting releases of unheld tokens |
| CC-8 | Client MUST release orphaned tokens on FIFO consumer death | Prevents token leak when reader dies mid-acquire (JSP-ERR-5) | Holding tokens for dead consumers |
| CC-9 | Client MUST clean up FIFO on any exit path | Prevents stale FIFOs blocking subsequent builds (JSP-ERR-6) | Leaving FIFO on crash/signal |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         HOST                                │
│  ┌──────────────────┐                                       │
│  │ External Make    │  MAKEFLAGS (optional)                 │
│  │ Jobserver        │                                       │
│  └────────┬─────────┘                                       │
│           │                                                 │
│           ▼                                                 │
│  ┌──────────────────┐                                       │
│  │  jobsys serve    │  Listens: @buildsys-jobs-{token}      │
│  │                  │  Manages: TokenPool                   │
│  │                  │  Tracks: ClientConnection[]           │
│  └────────┬─────────┘                                       │
│           │ Abstract Unix Domain Socket                     │
├───────────┼─────────────────────────────────────────────────┤
│           │              DOCKER (--net host)                │
│           ▼                                                 │
│  ┌──────────────────┐                                       │
│  │  jobsys client   │  Connects to abstract socket          │
│  │                  │  Creates: /tmp/jobserver-fifo         │
│  │                  │  Exports: MAKEFLAGS                   │
│  └────────┬─────────┘                                       │
│           │ FIFO                                            │
│           ▼                                                 │
│  ┌──────────────────┐                                       │
│  │ rpmbuild / make  │  Standard jobserver protocol          │
│  │ / cargo          │  read(fifo) = acquire token           │
│  │                  │  write(fifo) = release token          │
│  └──────────────────┘                                       │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

**jobsys serve (Host)**
- Create abstract Unix domain socket
- Connect to external jobserver OR create standalone token pool
- Accept client connections, track per-connection token ownership
- Handle acquire/release requests with blocking semantics
- Reclaim tokens when connections drop

**jobsys client (Container)**
- Connect to abstract socket via `--net host`
- Create FIFO at well-known path
- Output MAKEFLAGS for shell eval
- Proxy FIFO reads → acquire requests
- Proxy FIFO writes → release requests
- Run until terminated or connection lost

## Domain Model

### Core Types

**TokenPool**
- Holds available tokens (bytes)
- Blocking acquire: wait until token available, return it
- Release: return token to pool
- Source: external jobserver FDs or self-created pool

**ClientConnection**
- Socket connection to one container client
- Tracks tokens currently held by this client
- On drop: release all held tokens back to pool

**AcquireRequest**
- Single byte: `0x41` ('A')
- Response: `0x54` ('T') + token byte

**ReleaseRequest**
- Two bytes: `0x52` ('R') + token byte
- Response: `0x4B` ('K')

### Domain Operations

**acquire_token(client: &mut ClientConnection) -> Result<u8, AcquireError>**
- Blocks until token available from pool
- Records token as held by this client
- Returns token byte
- **Invariant**: Every acquired token is tracked in client's held set

**release_token(client: &mut ClientConnection, token: u8) -> Result<(), ReleaseError>**
- Validates token was held by this client (JSP-5b)
- Returns token to pool
- Removes from client's held set
- **Invariant**: Only tokens in held set can be released
- **Error**: Returns `ReleaseError::NotHeld` if token not in client's held set

**handle_disconnect(client: ClientConnection)**
- Returns all held tokens to pool
- Logs if tokens were reclaimed (indicates crash)
- **Invariant**: No tokens lost on client disconnect

### Error Types

**ServerError**
- `SocketBind` - Cannot create abstract socket
- `JobserverConnect` - Cannot connect to external jobserver
- `ClientAccept` - Error accepting new connection
- `UpstreamDisconnect` - External jobserver disconnected at runtime (JSP-ERR-2a)

**AcquireError**
- `Timeout` - No token available within timeout period (JSP-4a)
- `UpstreamUnavailable` - External jobserver disconnected, cannot acquire new tokens

**ClientError**
- `SocketConnect` - Cannot connect to server socket (JSP-ERR-2)
- `FifoCreate` - Cannot create FIFO (JSP-ERR-3)
- `ProtocolError` - Malformed server response
- `FifoConsumerDeath` - FIFO reader died mid-operation (JSP-ERR-5)

**ProtocolError**
- `InvalidCommand` - Unknown command byte
- `InvalidToken` - Release of unheld token (JSP-5b)

**ReleaseError**
- `NotHeld` - Client attempted to release token it doesn't hold

## Module Structure

```
twoliter/tools/jobsys/
├── Cargo.toml
└── src/
    ├── main.rs        # CLI: serve/client subcommands
    ├── server.rs      # Server implementation (~200 lines)
    ├── client.rs      # Client implementation (~150 lines)
    ├── protocol.rs    # Wire protocol constants and helpers (~50 lines)
    └── lib.rs         # Shared types: TokenPool, errors (~100 lines)
```

Estimated total: ~500 lines, within 550-line module limit.

## Design Patterns

**Connection-scoped resource tracking**: Each `ClientConnection` owns its held tokens. Rust's `Drop` trait ensures tokens return to pool even on panic or unexpected disconnect. This pattern prevents resource leaks without explicit cleanup code.

**Abstract socket for cross-namespace communication**: Reuses pipesys pattern. Abstract sockets exist in kernel namespace, accessible from containers with `--net host`. No filesystem coordination needed.

**FIFO as protocol adapter**: Build tools expect FIFO semantics. Client translates FIFO read/write to socket request/response. Keeps complexity in one place.

## Design Decisions

### DD-1: New tool vs extending pipesys

**Decision**: Create new `jobsys` tool

**Alternatives considered**:
1. Add commands to pipesys
2. Create jobsys as separate tool

**Rationale**: Different protocol (request-response vs fd passing), different lifecycle (long-running proxy vs one-shot fd serve). Separate tool is cleaner.

**Implications**: New Cargo.toml, but can share `uds` crate dependency.

### DD-2: FIFO emulation vs fd passing

**Decision**: FIFO emulation with proxy

**Alternatives considered**:
1. Pass real jobserver FDs via SCM_RIGHTS (like pipesys)
2. Create FIFO in container, proxy operations to host

**Rationale**: 
- FD passing requires container process to receive FDs before build tools run - complex Dockerfile choreography
- FIFO works with Make 4.4+ `--jobserver-auth=fifo:` format
- Proxy can track token ownership for crash recovery

**Implications**: Slightly higher latency per token (socket round-trip), but negligible vs build time.

### DD-3: Token tracking strategy

**Decision**: Per-connection token tracking with automatic reclaim

**Alternatives considered**:
1. Trust clients to release tokens (simple but leaky)
2. Timeout-based reclaim (complex, race-prone)
3. Connection-scoped tracking (chosen)

**Rationale**: Connection drop is reliable signal of client death. Tracking held tokens per-connection enables immediate reclaim with no false positives.

**Implications**: Server maintains `HashSet<u8>` per client. Small memory overhead, O(1) operations.

## Implementation Guidance

**Server (JSP-1 through JSP-6)**:
- Use `tokio` async runtime with `UnixSeqpacketListener` (same as pipesys)
- `TokenPool` can wrap `jobserver::Client` for external jobserver or `Vec<u8>` for standalone
- Spawn task per client connection
- Use `tokio::select!` for graceful shutdown on SIGTERM
- Socket bind with retry on EADDRINUSE, regenerate random token (JSP-ERR-7)
- Log at debug level when pool exhausted and requests are waiting (JSP-NFR-4)

**Client (JSP-7 through JSP-11)**:
- Single-threaded, blocking I/O is fine (simpler than async for FIFO handling)
- Create FIFO with `mkfifo`, open both ends to prevent EOF
- Use `select()` or `poll()` to multiplex FIFO and socket
- Print `MAKEFLAGS=--jobserver-auth=fifo:/tmp/jobserver-fifo` for eval
- Track last FIFO activity time for liveness warning (JSP-10a)
- On server disconnect, remove FIFO and exit non-zero (JSP-ERR-1a)
- On EPIPE/EOF during token delivery, release token back to server (JSP-ERR-5)
- Register signal handlers (SIGTERM, SIGINT) to ensure FIFO cleanup (JSP-ERR-6)
- Use `Drop` impl or `scopeguard` for FIFO removal on any exit path

**Protocol (Appendix A from requirements)**:
- Acquire: client sends `'A'`, server responds `'T'` + token byte
- Acquire timeout: server responds `'E'` + `0x01` (timeout error code)
- Release: client sends `'R'` + token byte, server responds `'K'`
- Invalid release: server responds `'E'` + `0x02` (not-held error code), closes connection
- Error: server sends `'E'` + error code, closes connection

**Integration (JSP-12, JSP-13)**:
- buildsys spawns `jobsys serve` before builds, passes socket name as build-arg
- Dockerfile runs `eval $(jobsys client --socket $JOBSERVER_SOCKET)` before rpmbuild
- Requires `--net host` (already used for pipesys)

## Testing Strategy

**Unit tests**:
- TokenPool: acquire blocks when empty, release makes token available
- Protocol parsing: valid and invalid messages
- ClientConnection: held token tracking, drop releases tokens

**Integration tests**:
- Server + client communication over abstract socket
- Multiple clients sharing token pool
- Client disconnect reclaims tokens
- FIFO read/write proxies correctly
- FIFO consumer death releases orphaned token (JSP-ERR-5)
- Client cleanup removes FIFO on all exit paths (JSP-ERR-6)
- Socket name collision triggers retry (JSP-ERR-7)

**Manual validation**:
- Run `make -j` inside container, verify parallelism matches token count
- Kill container mid-build, verify tokens reclaimed
- Verify latency overhead is negligible

## Notes

- Wire protocol matches requirements Appendix A exactly
- Module structure keeps impl blocks together per CC guidance
- Server pattern mirrors `pipesys/src/server.rs` for consistency
- Client is new pattern - no pipesys equivalent to reference
