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

1. **Seed the forest**: `./seed-forest.sh` (clones any missing repositories)
2. **Build forester**: `cd forester && cargo build --release`
3. **Explore skills**: See `skills/` directory for available workflows

## Using the Forest

**IMPORTANT: Before starting work or answering questions**, read `@./docs/architecture.md` to understand:
- How kits, variants, and the build system work together
- The development workflow and dependencies between components
- Where to find key configuration files

**When investigating system internals** (partitions, disk layout, boot process, encryption, etc.), check:
1. `docs/architecture.md` for high-level system design
2. Component-specific documentation in relevant repositories

This context is essential for understanding the Bottlerocket ecosystem before making changes or answering questions about system behavior.

## Component Dependencies

When developing features:
1. Changes to kits (core-kit, kernel-kit) require building and publishing to OCI registry
2. Variants in `bottlerocket/` consume kits from OCI registries
3. Building a variant requires specifying kit versions
4. Testing requires deploying the built variant image

The forest tool helps orchestrate these dependencies.

## Skills

Skills are documented workflows that AI agents can follow. Each skill includes:
- Purpose and when to use it
- Prerequisites and dependencies
- Step-by-step procedures
- Validation steps
- Common failure modes and recovery

See individual skill directories for details.
