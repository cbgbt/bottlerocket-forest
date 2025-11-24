# Bottlerocket Forest

A meta-repository for AI-assisted Bottlerocket development.

## Purpose

The forest provides:
- Organized access to all Bottlerocket component repositories
- Skills for AI agents to perform common development workflows
- Tooling to simplify local development and testing
- High-level documentation for understanding the Bottlerocket ecosystem

## Layout

```
bottlerocket-forest/
├── bottlerocket/              # Main Bottlerocket OS repository (variants, build configs)
├── kits/
│   ├── bottlerocket-core-kit/     # Core OS packages and dependencies
│   └── bottlerocket-kernel-kit/   # Kernel packages
├── sdk/
│   └── bottlerocket-sdk/          # Build SDK and toolchain
├── host-containers/
│   ├── bottlerocket-admin-container/   # Admin container for system access
│   └── bottlerocket-control-container/ # Control container for orchestration
├── twoliter/                  # Bottlerocket build tool
├── bottlerocket-settings-sdk/ # SDK for settings plugins
├── forester/                  # Forest orchestration CLI (Rust)
├── skills/                    # AI agent skills for common workflows
├── docs/                      # High-level Bottlerocket documentation
└── planning/                  # Scratch space for notes and planning (gitignored)
```

## Getting Started

### ⚠️ REQUIRED FIRST-TIME SETUP ⚠️

**Run this once to set up everything:**
```bash
./seed-forest.sh
```

This script will:
1. Clone all Bottlerocket repositories
2. Build the forester tool
3. Build the knowledge index
4. Verify everything works

The script is idempotent and quiet by default. Use `./seed-forest.sh --verbose` for detailed output.

## Working with the Forest

**MANDATORY: Start every session by running `./seed-forest.sh`**

**CRITICAL**: Before answering questions about Bottlerocket, ALWAYS:
1. Use `forester index search` to locate relevant documentation files
2. Read the identified files directly
3. Never rely on general knowledge when documentation exists

This ensures accurate, up-to-date information from actual documentation. Note that the index searches documentation only, not source code - use traditional search for code.

## Forester

Forester is the forest's orchestration tool, providing commands for common development tasks:

**Knowledge Index** - Semantic search across all forest documentation:
```bash
forester index build                    # Build the search index
forester index search "boot process"    # Search documentation
forester index status                   # Check index status
```

**Local Registry** - Manage a local OCI registry for kit development:
```bash
forester registry start    # Start local registry
forester registry status   # Check registry status
forester registry list     # List published images
```

See `forester/README.md` for complete documentation.

## Using the Forest

**IMPORTANT: Before starting work or answering questions**, read `@./docs/architecture.md` to understand:
- How kits, variants, and the build system work together
- The development workflow and dependencies between components
- Where to find key configuration files

**When investigating system internals** (partitions, disk layout, boot process, encryption, etc.), check:
1. `docs/architecture.md` for high-level system design
2. Component-specific documentation in relevant repositories

This context is essential for understanding the Bottlerocket ecosystem before making changes or answering questions about system behavior.

## Skills

The `skills/` directory contains modular workflows for common Bottlerocket development tasks. Skills are model-invoked—Claude autonomously decides when to use them based on your request and each skill's description. When working on any task, evaluate available skills and use them when relevant to ensure consistent, reproducible workflows.

## Component Dependencies

When developing features:
1. Changes to kits (core-kit, kernel-kit) require building and publishing to OCI registry
2. Variants in `bottlerocket/` consume kits from OCI registries
3. Building a variant requires specifying kit versions
4. Testing requires deploying the built variant image

The forest tool helps orchestrate these dependencies.
