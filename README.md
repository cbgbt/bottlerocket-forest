# Bottlerocket Forest

A meta-repository for AI-assisted Bottlerocket development.

## 🤖 For AI Agents

**Read [AGENTS.md](./AGENTS.md) first** - it contains the mandatory workflow and detailed guidance.

**Quick checklist:**
1. Run `./seed-forest.sh` (every session)
2. Read `./docs/architecture.md` and `./docs/build-system.md`
3. Read `./skills/README.md` - contains skill protocol and index of available skills

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
├── crates/                    # Rust workspace
│   ├── sembly-core/           # Semantic search library
│   ├── sembly-cli/            # Sembly CLI tool
│   └── forester/              # Forest orchestration CLI
├── skills/                    # AI agent skills for common workflows
├── docs/                      # High-level Bottlerocket documentation
└── planning/                  # Scratch space for notes and planning (gitignored)
```

## Getting Started

### First-Time Setup

```bash
./seed-forest.sh
```

This script will:
1. Clone all Bottlerocket repositories
2. Build the forest tools (sembly, forester)
3. Build the knowledge index
4. Verify everything works

The script is idempotent and quiet by default. Use `./seed-forest.sh --verbose` for detailed output.

## Sembly

Semantic search tool for exploring Bottlerocket documentation. **Must run from forest root directory.**

```bash
sembly build                    # Build search index
sembly search "boot process"    # Search documentation
sembly status                   # Check index status
sembly update                   # Update index incrementally
sembly rebuild                  # Rebuild from scratch
```

Sembly is a standalone open-source tool that can be applied to any codebase. See `crates/sembly-cli/` for details.

## Forester

Forest orchestration tool. **Must run from forest root directory.**

```bash
forester registry start    # Start local registry
forester registry status   # Check registry status
forester registry list     # List published images
```

See `crates/forester/README.md` for complete documentation.

## Development

The forest uses a Cargo workspace. Build all tools:

```bash
make build          # Build all binaries
make test           # Run unit tests
make integ          # Run full test suite (fmt, clippy, deny, tests)
make release-build  # Build optimized binaries
```

Binaries are output to `./target/release/sembly` and `./target/release/forester`.

## Documentation Guidelines

**Add Keywords for Search:**

Include a keywords line near the top of documentation files:

```markdown
**Keywords:** primary-topic, related-term, technical-concept, component-name
```

Include 5-15 terms: technical concepts, component names, use cases, related features.
Use lowercase, comma-separated. Improves `sembly search` discoverability.

**Example:**
```markdown
# Bottlerocket Boot Process

**Keywords:** boot, systemd, targets, preconfigured, configured, multi-user, 
fipscheck, services, dependencies, API system, bootstrap containers, settings
```

## Skills

The `skills/` directory contains modular workflows for common Bottlerocket development tasks. Skills are mandatory when they exist for a task. See `skills/README.md` for the complete protocol.

## Component Dependencies

When developing features:
1. Changes to kits (core-kit, kernel-kit) require building and publishing to OCI registry
2. Variants in `bottlerocket/` consume kits from OCI registries
3. Building a variant requires specifying kit versions
4. Testing requires deploying the built variant image

The forest tools help orchestrate these dependencies.
