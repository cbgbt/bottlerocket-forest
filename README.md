# Bottlerocket Forest

A meta-repository for Bottlerocket development.

## Purpose

The forest provides:
- Organized access to all Bottlerocket component repositories
- Skills for AI agents to perform common development workflows
- Tooling to simplify local development and testing
- High-level documentation for understanding the Bottlerocket ecosystem

## Getting Started

After seeding the forest, create a grove to begin working:

```bash
forester grove create develop
cd groves/develop

# Then, start your AI agent
```

Groves are isolated working directories containing all Bottlerocket repositories.
Name yours anything—"develop" is conventional for general work.

AI agents are meant to be run from within a grove when working on Bottlerocket.

## Forest Layout

```
bottlerocket-forest/
├── crates/                    # Local rust tools
│   ├── forester/              # Generic forest management CLI
│   └── brdev/                 # Bottlerocket-specific dev tooling
├── groves/                    # Forest groves for doing work on bottlerocket
├── skills/                    # AI agent skills for common workflows
├── docs/                      # High-level Bottlerocket documentation
├── grove-docs/                # Documentation symlinked into groves upon creation
└── planning/                  # Scratch space for notes and planning (gitignored)
```

## Crumbly

Semantic search tool for exploring documentation.

Install via:
```bash
cargo install --path ./crates/crumbly-cli
```

Usage:
```bash
crumbly build --context ./groves/develop   # Build search index
crumbly search "boot process"              # Search documentation
```

The crumbly index is stored in the forest root under `.crumbly`

## Forester

Generic forest management tool.

Install via:
```bash
cargo install forester
```

Usage:
```bash
forester seed                      # Clone all member repos and set up forest
forester grove create feature-x    # Create grove for all member repos
forester grove list                # List existing groves
forester grove remove feature-x    # Remove a grove
```

Forester is a standalone open-source tool that can be applied to any codebase. See `crates/forester/README.md` for complete documentation.

## brdev

Bottlerocket-specific development tooling. **Must run from within a grove directory.**

```bash
brdev registry start    # Start local registry
brdev registry status   # Check registry status
brdev registry list     # List published images
```

See `crates/brdev/README.md` for complete documentation.

## Development

The forest uses a Cargo workspace for tools. Build all tools:

```bash
make build          # Build all binaries
make check          # Run unit tests, plus common checks (fmt, clippy, deny)
make integ          # Run full test suite (fmt, clippy, deny, unit tests, integ tests)
make release-build  # Build optimized binaries
```

**Always run `make integ` before committing changes to crates.**

## Documentation Guidelines

**Add Keywords for Search:**

Include a keywords line near the top of documentation files:

```markdown
**Keywords:** primary-topic, related-term, technical-concept, component-name
```

Include 5-15 terms: technical concepts, component names, use cases, related features.
Use lowercase, comma-separated. Improves `crumbly search` discoverability.

**Example:**
```markdown
# Bottlerocket Boot Process

**Keywords:** boot, systemd, targets, preconfigured, configured, multi-user, 
fipscheck, services, dependencies, API system, bootstrap containers, settings
```

**One Sentence Per Line:**

Write each sentence on its own line in markdown files.
This makes diffs easier to review—changes to one sentence don't affect adjacent lines.

```bash
# Format a file
python3 scripts/sentence-split.py path/to/file.md --in-place
```

## Skills

The `skills/` directory contains modular workflows for common Bottlerocket development tasks.
These skills are designed to be used from within a grove, rather than from the forest root.

