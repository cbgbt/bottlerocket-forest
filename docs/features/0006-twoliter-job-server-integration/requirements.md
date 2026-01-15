# Jobserver Proxy - Requirements Specification

## Overview

This specification defines requirements for the jobserver proxy feature, which tunnels GNU Make jobserver tokens through the Docker boundary to enable coordinated resource pooling across parallel Bottlerocket builds.
Twoliter creates and owns the jobserver; jobsys proxies access to containers.
See [concept.md](./concept.md) for background.

## Functional Requirements

### Server Requirements

#### JSP-1: Server Socket Creation

**WHEN** the jobserver proxy server starts  
**THEN** the system **SHALL** create an abstract Unix domain socket with a unique name

Example:
```
@buildsys-jobs-{random-token}
```

#### JSP-2: Jobserver Connection

**WHEN** the server starts with jobserver file descriptors from Twoliter  
**THEN** the system **SHALL** use those descriptors to acquire and release tokens

#### JSP-3: Token Acquisition

**WHEN** a client sends an acquire request  
**THEN** the system **SHALL** block until a token is available from the jobserver  
**AND** return the token to the client

#### JSP-3a: Acquire Timeout

**WHILE** a client is waiting for a token  
**WHERE** no token becomes available within a configurable timeout (default: 30 minutes)  
**THEN** the system **SHALL** respond with a timeout error  
**AND** the client **SHALL** exit with a non-zero status

#### JSP-4: Token Release

**WHEN** a client sends a release request with a token  
**THEN** the system **SHALL** return the token to the jobserver

#### JSP-4a: Per-Client Token Tracking

**WHILE** the server is running  
**THEN** the system **SHALL** track which tokens are held by each client connection  
**AND** this tracking **SHALL** be updated on every acquire and release operation

#### JSP-4b: Invalid Release Rejection

**WHEN** a client sends a release request for a token it does not hold  
**THEN** the system **SHALL** respond with a protocol error  
**AND** close the client connection

#### JSP-5: Multiple Client Support

**WHILE** the server is running  
**THEN** the system **SHALL** accept connections from multiple concurrent clients

### Client Requirements

#### JSP-6: Socket Connection

**WHEN** the client starts with a socket name  
**THEN** the system **SHALL** connect to the abstract Unix domain socket on the host

#### JSP-7: FIFO Creation

**WHEN** the client connects successfully  
**THEN** the system **SHALL** create a named FIFO at `/tmp/jobserver-fifo`

#### JSP-8: MAKEFLAGS Export

**WHEN** the client creates the FIFO  
**THEN** the system **SHALL** output the appropriate `MAKEFLAGS` value for the FIFO path

Example:
```
--jobserver-auth=fifo:/tmp/jobserver-fifo
```

#### JSP-9: Read Proxy

**WHEN** a process reads from the FIFO  
**THEN** the system **SHALL** send an acquire request to the server  
**AND** write the received token to the FIFO reader

#### JSP-9a: Consumer Liveness Detection

**WHILE** the client holds tokens on behalf of FIFO readers  
**WHERE** no FIFO activity occurs for a configurable timeout (default: 5 minutes)  
**THEN** the system **SHALL** log a warning  
**AND** continue holding tokens (no automatic release)

*Rationale: Automatic release risks correctness. Warning enables debugging of stuck builds.*

#### JSP-10: Write Proxy

**WHEN** a process writes a token to the FIFO  
**THEN** the system **SHALL** send a release request with that token to the server

### Integration Requirements

#### JSP-11: Docker Build Argument

**WHEN** Twoliter starts a Docker build  
**THEN** the system **SHALL** pass the socket name as a build argument

#### JSP-12: Container Network Mode

**WHILE** the jobserver proxy is in use  
**THEN** Docker builds **SHALL** use `--net host` to access the abstract socket

## Non-Functional Requirements

#### JSP-NFR-1: Token Acquisition Latency

**WHILE** tokens are available in the pool  
**THEN** the system **SHALL** complete token acquisition in under 1 millisecond

#### JSP-NFR-2: Scalability

**WHILE** the server is running  
**THEN** the system **SHALL** support at least 100 concurrent client connections

#### JSP-NFR-3: Graceful Shutdown

**WHEN** the server receives a termination signal  
**THEN** the system **SHALL** complete pending token operations before exiting

#### JSP-NFR-4: Pool Exhaustion Logging

**WHILE** all tokens are held by clients  
**WHEN** a new acquire request arrives  
**THEN** the system **SHALL** log a debug message indicating pool exhaustion  
**AND** include the number of waiting clients

## Error Handling

#### JSP-ERR-1: Client Disconnect Recovery

**WHILE** a client holds tokens  
**WHERE** the client connection drops unexpectedly  
**THEN** the system **SHALL** return all tokens held by that client to the jobserver  
**AND** log the number of tokens reclaimed

#### JSP-ERR-2: Server Disconnect Handling

**WHILE** the client is connected to the server  
**WHERE** the server connection drops unexpectedly  
**THEN** the client **SHALL** exit with a non-zero status  
**AND** the FIFO **SHALL** be removed

#### JSP-ERR-3: Server Unavailable

**WHILE** the client is attempting to connect  
**WHERE** the server socket does not exist  
**THEN** the system **SHALL** exit with a non-zero status and descriptive error message

#### JSP-ERR-4: FIFO Creation Failure

**WHILE** the client is starting  
**WHERE** the FIFO cannot be created  
**THEN** the system **SHALL** exit with a non-zero status and descriptive error message

#### JSP-ERR-5: Protocol Error

**WHILE** the server is processing a request  
**WHERE** the request is malformed  
**THEN** the system **SHALL** close the client connection  
**AND** log the error

#### JSP-ERR-6: FIFO Consumer Death

**WHILE** the client has acquired a token from the server  
**WHERE** the FIFO reader closes before receiving the token (EPIPE/EOF)  
**THEN** the system **SHALL** release the orphaned token back to the server  
**AND** log the orphaned acquisition

#### JSP-ERR-7: Client Cleanup

**WHEN** the client exits (normally or abnormally)  
**THEN** the system **SHALL** remove the FIFO  
**AND** release any held tokens to the server

#### JSP-ERR-8: Socket Name Collision

**WHILE** the server is creating the abstract socket  
**WHERE** the socket name is already in use  
**THEN** the system **SHALL** retry with a new random token (up to 3 attempts)  
**AND** fail with a descriptive error if all retries exhausted

## Appendix A: Wire Protocol

The protocol between client and server uses simple byte messages over the Unix domain socket.

### Acquire Request
```
Client -> Server: 0x41 ('A')
Server -> Client: 0x54 ('T') + token_byte
```

### Release Request
```
Client -> Server: 0x52 ('R') + token_byte
Server -> Client: 0x4B ('K')
```

### Error Response
```
Server -> Client: 0x45 ('E') + error_code

Error codes:
  0x01 - Timeout (no token available within timeout)
  0x02 - NotHeld (release of token not held by this client)
```

## Appendix B: Environment Integration

### Server Invocation (by Twoliter)
```bash
# Twoliter creates jobserver, passes FDs to jobsys
jobsys serve --socket @buildsys-jobs-abc123 --read-fd 3 --write-fd 4
```

### Client Invocation (in container)
```bash
eval $(jobsys client --socket @buildsys-jobs-abc123)
# Sets MAKEFLAGS in environment
```

### Docker Build Argument
```bash
docker build --build-arg JOBSERVER_SOCKET=buildsys-jobs-abc123 --net host ...
```
