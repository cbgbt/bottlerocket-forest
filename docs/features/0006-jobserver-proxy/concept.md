---
feature: 0001-jobserver-proxy
status: proposed
---

# Jobserver Proxy

## Problem

Bottlerocket builds many packages in parallel using Docker and BuildKit, but there's no coordination of compute resources across these parallel builds. Today, Twoliter batches builds heuristically using a fixed `BUILDSYS_JOBS` value that defaults to 8. This leads to wasteful periods where some builds sit idle while others are starved for resources, and missed opportunities to dynamically shift capacity to wherever it's needed most.

The lack of coordination also contributes to BuildKit race conditions that require up to 10 retries per build. When multiple builds compete for the same resources without any arbitration, contention spikes unpredictably.

GNU Make solved this problem decades ago with the jobserver protocol—a simple token-passing scheme that pools compute resources across nested builds. But BuildKit doesn't support the jobserver protocol, and Docker's isolation boundary prevents the standard pipe-based token passing from working even if it did.

## Solution

The jobserver proxy tunnels GNU Make jobserver tokens through the Docker boundary using the same abstract Unix domain socket pattern that pipesys already uses for build artifacts. A host-side proxy server connects to the real jobserver and listens on an abstract socket. Inside each container, a client creates a standard FIFO that rpmbuild, make, and cargo can use normally—but reads and writes to that FIFO are proxied through the socket to the host, where real tokens are acquired and released.

From the perspective of build tools inside the container, they're talking to a normal GNU Make jobserver. They don't know or care that the tokens are actually managed by a proxy on the host. This means existing Makefiles and Cargo builds work without modification.

## How It Works

When a Twoliter build starts, the host spawns a jobserver proxy server that either connects to an external Make jobserver or creates its own token pool based on available CPUs. The server listens on an abstract Unix domain socket with a unique name like `@buildsys-jobs-{token}`.

Each Docker build receives this socket name as a build argument. Inside the container, before rpmbuild runs, a client process connects to the abstract socket and creates a FIFO at a well-known path. It then sets `MAKEFLAGS` to point at this FIFO, making it look like a standard jobserver to any build tools that run.

When rpmbuild or make wants to start a parallel job, it reads from the FIFO to acquire a token. The client proxies this request to the host server, which blocks until a real token is available from the global pool. When the job finishes, the tool writes the token back to the FIFO, and the client proxies the release back to the host.

Because all containers share the same token pool on the host, resources flow naturally to wherever they're needed. A heavy kernel build can acquire many tokens while lighter packages use fewer. When the kernel build finishes, those tokens become available for other builds immediately.

## Benefits

True resource pooling replaces heuristic batching. Instead of guessing how many builds to run in parallel, the system dynamically allocates capacity based on actual demand. Heavy builds get more resources when others are idle, and the total parallelism stays within the configured limit.

The existing pipesys pattern proves this approach works. Abstract Unix domain sockets with `--net host` already tunnel file descriptors across the Docker boundary for build artifacts. The jobserver proxy uses the same mechanism with a simpler protocol—just token acquire and release messages.

No changes to BuildKit or Docker are required. The proxy works entirely in userspace, using standard Unix primitives that are already available. Build tools inside containers see a standard jobserver FIFO and work normally.

## Technical Notes

The implementation extends or parallels pipesys with two new commands: a host-side server and a container-side client. The protocol is simple request-response over the abstract socket.

Token cleanup on container crash needs consideration—if a container dies while holding tokens, the proxy must reclaim them. The server can track which tokens are held by which client and release them if the connection drops.

Latency from the extra round-trip per token is negligible compared to actual build time. Token acquisition happens at job boundaries, not on every compiler invocation.
