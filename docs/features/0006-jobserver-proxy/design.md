# Jobserver Proxy - Technical Design

## Overview

The jobserver proxy (jobsys) tunnels GNU Make jobserver tokens through the Docker boundary.
Twoliter creates and owns the jobserver; jobsys bridges access to containers via abstract Unix domain sockets.

**Design Philosophy**: Minimal proxy layer that makes Docker builds appear to have native jobserver access.
Build tools (make, cargo, rpmbuild) see a standard FIFO and work unchanged.

## Solution Mechanics

This section explains WHY the design choices work, not just what they are.

### How Abstract Sockets + `--net host` Bridges the Docker Boundary

Docker containers are isolated by Linux namespaces.
By default, a container has its own network namespace with separate socket namespaces.
Abstract Unix sockets (addresses starting with `@`) exist in the network namespace, not the filesystem.

When a container runs with `--net host`, it shares the host's network namespace.
This means abstract sockets created on the host are directly accessible from inside the container—no mounts, no port mapping, no special configuration.
The socket appears at the same abstract address in both contexts.

This is the same pattern pipesys uses for build artifact transfer.
It's proven, requires no Docker modifications, and works with standard socket APIs.

### How FIFO Proxying Works Mechanically

GNU Make's jobserver protocol uses file descriptors: read to acquire a token, write to release.
Traditionally these are pipes, but Make 4.4+ supports FIFOs via `--jobserver-auth=fifo:/path`.

The jobsys client creates a FIFO and opens both ends:
- It holds the write end open to prevent EOF when readers disconnect
- It monitors the read end for incoming token releases from build tools

When a build tool reads from the FIFO to acquire a token:
1. The read blocks (FIFO is empty)
2. Client detects a pending reader via `select()`/`poll()`
3. Client sends acquire request to server over abstract socket
4. Server blocks until token available from Twoliter's jobserver
5. Server returns token byte to client
6. Client writes token byte to FIFO
7. Build tool's read completes with the token

When a build tool writes to the FIFO to release a token:
1. Client reads the token byte from FIFO
2. Client sends release request to server
3. Server returns token to Twoliter's jobserver
4. Server acknowledges release to client

The FIFO acts as a protocol adapter—build tools use standard POSIX I/O while the client handles the socket communication.

### How Token Tracking Enables Crash Recovery

If a container crashes while holding tokens, those tokens would be lost forever without recovery.
The server maintains a `HashSet<u8>` per client connection tracking which tokens that client holds.

On every acquire: token added to client's held set.
On every release: token removed from client's held set (with validation).
On connection drop: all tokens in held set returned to jobserver.

Rust's `Drop` trait makes this automatic—when the `ClientConnection` struct is dropped (whether from normal close, error, or panic), its destructor returns held tokens.
No explicit cleanup code paths needed.

This also enables JSP-4b (invalid release rejection): if a client tries to release a token not in its held set, the server knows immediately and can reject the request.

### Why Build Tools Work Unchanged

The entire proxy is invisible to build tools because:

1. **Standard interface**: FIFO at a well-known path with `MAKEFLAGS` pointing to it
2. **Blocking semantics preserved**: Reads block until token available, just like real jobserver
3. **Token bytes unchanged**: The actual token values pass through unmodified
4. **No timing assumptions**: Latency is higher but still negligible vs. build time

Make, Cargo, and rpmbuild all use the same jobserver protocol.
They check `MAKEFLAGS`, find the FIFO path, and use standard read/write.
The proxy is a transparent intermediary.

## Critical Constraints

| ID | Constraint | Rationale | Anti-pattern to avoid |
|----|------------|-----------|----------------------|
| CC-1 | Server MUST track tokens per-connection and reclaim on disconnect | Prevents token leaks when containers crash (JSP-ERR-1) | Trusting clients to release tokens before exit |
| CC-2 | Abstract socket name MUST use `@buildsys-jobs-{token}` format | Enables multiple concurrent builds; matches pipesys pattern | Hardcoded socket names causing collisions |
| CC-3 | Client FIFO MUST be at `/tmp/jobserver-fifo` | Predictable path for Dockerfile integration | Random paths requiring coordination |
| CC-4 | Protocol MUST use single-byte commands with blocking semantics | Matches GNU Make jobserver behavior; simple implementation | Complex framing or async protocols |
| CC-5 | Server MUST complete pending operations before shutdown | Prevents token loss during graceful termination (JSP-NFR-3) | Immediate exit dropping in-flight requests |
| CC-6 | Acquire requests MUST have configurable timeout | Prevents infinite blocking in deadlock scenarios (JSP-3a) | Unbounded blocking on token acquisition |
| CC-7 | Client MUST release orphaned tokens on FIFO consumer death | Prevents token leak when reader dies mid-acquire (JSP-ERR-6) | Holding tokens for dead consumers |
| CC-8 | Client MUST clean up FIFO on any exit path | Prevents stale FIFOs blocking subsequent builds (JSP-ERR-7) | Leaving FIFO on crash/signal |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                           HOST                              │
│                                                             │
│  ┌──────────────────┐                                        │
│  │    Twoliter      │  Creates jobserver, spawns jobsys     │
│  │                  │  Passes jobserver FDs to server        │
│  └────────┬─────────┘                                        │
│           │ jobserver FDs (read/write)                      │
│           ▼                                                 │
│  ┌──────────────────┐                                        │
│  │  jobsys serve    │  Listens: @buildsys-jobs-{token}      │
│  │                  │  Proxies: token acquire/release        │
│  │                  │  Tracks: per-client held tokens        │
│  └────────┬─────────┘                                        │
│           │ Abstract Unix Socket                            │
├───────────┼─────────────────────────────────────────────────┤
│           │          DOCKER (--net host)                    │
│           ▼                                                 │
│  ┌──────────────────┐                                        │
│  │  jobsys client   │  Connects to abstract socket          │
│  │                  │  Creates: /tmp/jobserver-fifo          │
│  │                  │  Exports: MAKEFLAGS                    │
│  └────────┬─────────┘                                        │
│           │ FIFO                                            │
│           ▼                                                 │
│  ┌──────────────────┐                                        │
│  │ rpmbuild / make  │  Standard jobserver protocol          │
│  │ / cargo          │  read(fifo) = acquire token           │
│  │                  │  write(fifo) = release token          │
│  └──────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

**Twoliter**
- Creates GNU Make jobserver with token count based on available CPUs
- Spawns jobsys server, passing jobserver file descriptors
- Passes socket name to Docker builds as build argument
- Owns the jobserver lifecycle

**jobsys serve (Host)**
- Receives jobserver FDs from Twoliter
- Creates abstract Unix domain socket
- Accepts client connections, tracks per-connection token ownership
- Proxies acquire/release to real jobserver
- Reclaims tokens when connections drop

**jobsys client (Container)**
- Connects to abstract socket via `--net host`
- Creates FIFO at well-known path
- Outputs MAKEFLAGS for shell eval
- Proxies FIFO reads → acquire requests
- Proxies FIFO writes → release requests

## Domain Model

### Core Types

**JobserverProxy**
- Holds jobserver read/write FDs from Twoliter
- Blocking acquire: read token byte from jobserver
- Release: write token byte to jobserver

**ClientConnection**
- Socket connection to one container client
- `held_tokens: HashSet<u8>` — tokens currently held by this client
- On drop: release all held tokens back to jobserver

### Domain Operations

**acquire_token(client: &mut ClientConnection) -> Result<u8, AcquireError>**
- Reads token from jobserver (blocks until available)
- Records token in client's held set
- Returns token byte
- **Invariant**: Every acquired token is tracked in client's held set

**release_token(client: &mut ClientConnection, token: u8) -> Result<(), ReleaseError>**
- Validates token is in client's held set
- Writes token to jobserver
- Removes from client's held set
- **Error**: Returns `ReleaseError::NotHeld` if token not in held set

**handle_disconnect(client: ClientConnection)**
- Returns all held tokens to jobserver
- Logs count if tokens were reclaimed
- **Invariant**: No tokens lost on client disconnect

### Error Types

**AcquireError**
- `Timeout` — No token available within timeout period
- `JobserverClosed` — Jobserver FDs closed unexpectedly

**ReleaseError**
- `NotHeld` — Client attempted to release token it doesn't hold

**ClientError**
- `SocketConnect` — Cannot connect to server socket
- `FifoCreate` — Cannot create FIFO
- `ServerDisconnect` — Server connection dropped
- `ConsumerDeath` — FIFO reader died mid-operation

## Module Structure

```
twoliter/tools/jobsys/
├── Cargo.toml
└── src/
    ├── main.rs        # CLI: serve/client subcommands
    ├── server.rs      # Server implementation
    ├── client.rs      # Client implementation
    ├── protocol.rs    # Wire protocol constants
    └── lib.rs         # Shared types and errors
```

## Design Decisions

### DD-1: Separate tool vs extending pipesys

**Decision**: Create new `jobsys` tool

**Alternatives considered**:
1. Add commands to pipesys
2. Create jobsys as separate tool

**Rationale**: Different protocol (request-response vs fd passing), different lifecycle (long-running proxy vs one-shot).
Separate tool is cleaner and avoids coupling unrelated functionality.

### DD-2: FIFO emulation vs fd passing

**Decision**: FIFO emulation with proxy

**Alternatives considered**:
1. Pass real jobserver FDs via SCM_RIGHTS (like pipesys)
2. Create FIFO in container, proxy operations to host

**Rationale**: 
- FD passing requires container process to receive FDs before build tools run—complex Dockerfile choreography
- FIFO works with Make 4.4+ `--jobserver-auth=fifo:` format
- Proxy enables per-client token tracking for crash recovery

**Implications**: Socket round-trip per token, but latency is negligible vs build time.

### DD-3: Token tracking granularity

**Decision**: Per-connection tracking with automatic reclaim

**Alternatives considered**:
1. Trust clients to release tokens (simple but leaky)
2. Timeout-based reclaim (complex, race-prone)
3. Connection-scoped tracking (chosen)

**Rationale**: Connection drop is reliable signal of client death.
Tracking held tokens per-connection enables immediate reclaim with no false positives.

**Implications**: Server maintains `HashSet<u8>` per client. O(1) operations, minimal memory.

## Implementation Guidance

**Server**:
- Use `tokio` async runtime for concurrent client handling
- Spawn task per client connection
- Use `tokio::select!` for graceful shutdown on SIGTERM
- Socket bind with retry on EADDRINUSE (JSP-ERR-8)

**Client**:
- Single-threaded blocking I/O (simpler than async for FIFO handling)
- Create FIFO with `mkfifo`, hold write end open to prevent EOF
- Use `select()` to multiplex FIFO and socket
- Register signal handlers for FIFO cleanup on any exit

**Protocol**:
- Acquire: client sends `'A'`, server responds `'T'` + token byte
- Release: client sends `'R'` + token byte, server responds `'K'`
- Error: server sends `'E'` + error code, closes connection

## Testing Strategy

**Unit tests**:
- Token tracking: acquire adds to held set, release removes, drop reclaims
- Protocol parsing: valid and invalid messages

**Integration tests**:
- Server + client communication over abstract socket
- Multiple clients sharing token pool
- Client disconnect reclaims tokens
- FIFO consumer death releases orphaned token

**Manual validation**:
- Run `make -j` inside container, verify parallelism matches token count
- Kill container mid-build, verify tokens reclaimed
