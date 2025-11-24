# Bottlerocket Forest

A meta-repository for AI-assisted Bottlerocket development.

## 🚨 MANDATORY WORKFLOW - START HERE

**STOP. Before ANY response, you MUST complete these steps IN ORDER:**

### Step 1: Seed the Forest (REQUIRED EVERY SESSION)

```bash
./seed-forest.sh
```

**Do this even if you think it's already done.** Run it and verify the output shows success.

**If you skip this step, your response will be WRONG because:**
- ❌ Forester tool won't exist → Can't search documentation
- ❌ Knowledge index missing → Can't find relevant files
- ❌ Repositories outdated → Will reference old code

### Step 2: Read Core Documentation (REQUIRED EVERY SESSION)

```bash
# Read these files before any work:
cat ./docs/architecture.md
cat ./docs/build-system.md
```

**These docs are ALWAYS relevant.** Reading takes 30 seconds, prevents hours of mistakes.

**If you skip this step, your response will be WRONG because:**
- ❌ Will guess instead of citing facts
- ❌ Will misunderstand component relationships
- ❌ Will give outdated or incorrect guidance

### Step 3: Check for Applicable Skills (REQUIRED BEFORE ANY TASK)

```bash
cat skills/README.md
ls skills/
```

- If a skill exists for the task, ANNOUNCE it, LOAD it, then USE IT
- Skills are tested procedures, not optional suggestions

**If you skip this step, your response will be WRONG because:**
- ❌ Will reinvent tested procedures incorrectly
- ❌ Will miss validation steps
- ❌ Will waste time on solved problems

### Workflow Verification

**Before answering, state which steps you completed:**
- [ ] Ran `./seed-forest.sh` and verified output
- [ ] Read `./docs/architecture.md`
- [ ] Read `./docs/build-system.md`
- [ ] Checked `skills/` directory for applicable skills

**💡 TIP: If you have todolist functionality, create a task list for multi-step workflows.** This helps track progress, prevents skipped steps, and provides clear status updates.

**Only after completing ALL steps should you proceed with the user's request.**

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

### Documentation Research

**ANY question about how Bottlerocket works requires using a skill.**

Before answering questions about Bottlerocket:
1. **Read `skills/README.md`** for the skill usage protocol
2. Check if a skill exists for the task (e.g., `research-with-citations`)
3. Follow the protocol: ANNOUNCE → LOAD → FOLLOW

## Forester

Forester is the forest's orchestration tool, providing commands for common development tasks:

**⚠️ CRITICAL: Forester must be run from the forest root directory**
- Forester searches for documentation in the current working directory
- Running it from `forester/` or using `cargo run` from inside `forester/` will NOT work
- Always use `./forester/target/release/forester` from the forest root

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

## Skills

The `skills/` directory contains modular workflows for common Bottlerocket development tasks. **Skills are mandatory when they exist for a task** - they are not optional suggestions. Always check for applicable skills before starting any work. See `skills/README.md` for the complete protocol.

## Component Dependencies

When developing features:
1. Changes to kits (core-kit, kernel-kit) require building and publishing to OCI registry
2. Variants in `bottlerocket/` consume kits from OCI registries
3. Building a variant requires specifying kit versions
4. Testing requires deploying the built variant image

The forest tool helps orchestrate these dependencies.
