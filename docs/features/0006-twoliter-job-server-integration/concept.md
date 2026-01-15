---
feature: 0006-twoliter-job-server-integration
status: proposed
---

# Jobserver Proxy

## Problem

Bottlerocket builds many packages in parallel using Docker and BuildKit, but there's no coordination of compute resources across these parallel builds.
Today, Twoliter batches builds heuristically using a fixed `BUILDSYS_JOBS` value that defaults to 8.
This leads to wasteful periods where some builds sit idle while others are starved for resources.

GNU Make solved this problem decades ago with the jobserver protocol—a simple token-passing scheme that pools compute resources across nested builds.
But Docker's isolation boundary prevents the standard pipe-based token passing from working.
Even if a container could inherit jobserver file descriptors, those descriptors point to pipes that exist only in the host's file descriptor table.

The core challenge: how do you share a token pool across a boundary that was designed to prevent exactly this kind of sharing?

## Solution

The jobserver proxy (jobsys) tunnels GNU Make jobserver tokens through the Docker boundary using abstract Unix domain sockets.
Abstract sockets exist in the kernel's network namespace rather than the filesystem, and containers running with `--net host` share the host's network namespace.
This creates a communication channel that crosses the Docker boundary without requiring filesystem mounts or special privileges.

The complete flow:

1. **Twoliter creates a jobserver** with a token pool sized to available CPUs
2. **Twoliter spawns jobsys** as a proxy server, passing it the jobserver's file descriptors
3. **jobsys listens** on an abstract Unix socket (e.g., `@buildsys-jobs-abc123`)
4. **Docker builds receive** the socket name as a build argument
5. **Inside each container**, a jobsys client connects to the abstract socket and creates a FIFO
6. **Build tools** (make, cargo, rpmbuild) read/write the FIFO using standard jobserver protocol
7. **The client proxies** FIFO operations to the host, where real tokens are acquired and released

From the perspective of build tools inside the container, they're talking to a normal GNU Make jobserver.
They don't know or care that the tokens are actually managed by a proxy on the host.

## How It Works

When Twoliter starts a build, it creates a GNU Make jobserver with tokens equal to the desired parallelism.
It then spawns the jobsys server process, passing the jobserver's read/write file descriptors.
The server listens on an abstract Unix domain socket with a unique random name.

Each Docker build receives this socket name as a build argument.
Inside the container, before rpmbuild runs, a jobsys client process connects to the abstract socket.
The client creates a FIFO at `/tmp/jobserver-fifo` and sets `MAKEFLAGS` to point at it, making it look like a standard jobserver to any build tools.

When make or cargo wants to start a parallel job, it reads from the FIFO to acquire a token.
The client intercepts this read and sends an acquire request over the socket to the host.
The server blocks until a real token is available from Twoliter's jobserver, then returns it.
When the job finishes, the tool writes the token back to the FIFO, and the client proxies the release.

Because all containers share the same token pool through the proxy, resources flow naturally to wherever they're needed.
A heavy kernel build can acquire many tokens while lighter packages use fewer.
When the kernel build finishes, those tokens become available for other builds immediately.

## Benefits

True resource pooling replaces heuristic batching.
Instead of guessing how many builds to run in parallel, the system dynamically allocates capacity based on actual demand.
Heavy builds get more resources when others are idle, and the total parallelism stays within the configured limit.

No changes to BuildKit or Docker are required.
The proxy works entirely in userspace, using standard Unix primitives.
Build tools inside containers see a standard jobserver FIFO and work normally.

Crash recovery is automatic.
The server tracks which tokens are held by each client connection.
If a container crashes, the connection drops, and the server immediately reclaims those tokens for other builds.

## Technical Notes

The implementation lives in `twoliter/tools/jobsys` as a standalone binary with `serve` and `client` subcommands.
The protocol is simple single-byte messages: 'A' for acquire, 'R'+token for release.

Abstract sockets are a Linux-specific feature.
They're created by binding to an address starting with a null byte (represented as `@` in socket addresses).
Unlike filesystem sockets, they don't leave files behind and are automatically cleaned up when all references close.

The FIFO approach requires Make 4.4+ which supports `--jobserver-auth=fifo:` format.
Older Make versions only support pipe-based jobservers, which can't work across the Docker boundary.
