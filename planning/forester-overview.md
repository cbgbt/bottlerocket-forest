# Forester: Forest Orchestration Tool

## Context

The Bottlerocket Forest is a meta-repository designed to enable AI agents to work effectively on Bottlerocket development. It contains all Bottlerocket component repositories organized in a logical structure, along with skills (documented workflows) and tooling to support development workflows.

## Purpose of Forester

Forester is a Rust CLI tool that orchestrates development workflows across the forest. It provides higher-level commands that wrap and coordinate multiple lower-level operations, making it easier for both humans and AI agents to work with Bottlerocket's multi-repository architecture.

### Why Rust?

We've experimentally found that Rust is harmonious with AI agents because the type system and compiler feedback help steer agents toward correct implementations. The tight feedback loop from the compiler acts as a guide during development.

### Why Not Extend Twoliter?

Twoliter is Bottlerocket's build tool - it's focused on building individual components and is part of the product itself. Forester has a different scope:
- Orchestrates across multiple repositories
- Manages development infrastructure (local registries, test environments)
- Provides convenience workflows for development
- Experimental and can evolve rapidly without affecting production tooling

## Key Responsibilities

### 1. Local OCI Registry Management

Bottlerocket kits are published to OCI registries, and variants consume them from there. For development, we need a local registry so agents don't need external registry access.

Forester should:
- Start/stop a local Docker registry container
- Manage registry persistence
- Provide status information
- Clean up registry data when needed

### 2. Build Orchestration

The Bottlerocket build flow has dependencies:
1. Build kits (core-kit, kernel-kit) using twoliter
2. Publish kits to OCI registry
3. Build variants that consume those kits
4. Deploy and test the resulting images

Forester should provide commands that handle these multi-step workflows, calling twoliter as needed but managing the coordination.

### 3. Development Status

Agents need to understand the current state:
- What's built and where
- What kit versions are published locally
- What's running (registry, test VMs, etc.)

Forester should provide visibility into the development environment state.

### 4. Environment Management

Support for:
- Test VM/container lifecycle
- Cleanup operations
- Configuration management

## Design Principles

### Agent-Friendly

- Clear, predictable command structure
- Informative output that agents can parse
- Validation commands to verify success
- Helpful error messages with recovery suggestions

### Composable

- Commands should be usable independently
- Skills can chain commands together
- Exit codes indicate success/failure clearly

### Minimal External Dependencies

- Leverage Docker for registry (already required for Bottlerocket builds)
- Use Rust standard library where possible
- Add dependencies only when they provide clear value

### Fail Fast

- Validate prerequisites before starting operations
- Check for conflicts (e.g., port already in use)
- Provide clear error messages

## Current State

The tool has a basic CLI structure with:
- Command parsing for `registry` subcommands (start, stop, status, clean)
- A `status` command for overall forest state
- Help text

All commands are currently stubs that need implementation.

## Implementation Priorities

1. **Registry management** - Most critical for the build workflow
2. **Status reporting** - Helps agents understand current state
3. **Build orchestration** - Higher-level workflows once basics work
4. **Test environment management** - Future enhancement

## Integration with Skills

Skills in the `skills/` directory document workflows for AI agents. They reference forester commands. As forester evolves, skills should be updated to reflect new capabilities.

The `local-registry` skill already documents the expected behavior of registry commands.

## Technical Details

### Registry Requirements

The local registry should:
- Run as a Docker container using the official `registry:2` image
- Listen on `localhost:5000`
- Use a named volume for persistence (e.g., `forester-registry-data`)
- Be named consistently (e.g., `forester-registry`) for easy management
- Support standard Docker registry v2 API

### Expected Behaviors

**registry start:**
- Check if registry is already running (idempotent)
- Start registry container if not running
- Wait for registry to be healthy before returning
- Output the registry URL

**registry stop:**
- Stop the registry container if running
- Preserve data volume
- Handle case where registry isn't running gracefully

**registry status:**
- Check if container exists and is running
- Report registry URL if running
- Report volume information
- Exit code 0 if running, non-zero if not

**registry clean:**
- Stop registry if running
- Remove the data volume
- Confirm destructive operation

**status:**
- Show registry status
- Show what kits are published (future)
- Show what variants are built (future)

## Notes

- Forester is in the forest repository itself at `forester/`
- Built with `cargo build --release`
- The binary is named `forester`
- This is experimental - iterate quickly and learn from usage
- Make implementation decisions based on your context and best practices
