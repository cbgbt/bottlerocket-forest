# Agent Workflow Guide

This document contains the mandatory workflow for AI agents working in the Bottlerocket Forest.

## 🚨 MANDATORY WORKFLOW - START HERE

**Complete these steps IN ORDER before ANY response:**

### Step 1: Seed the Forest

```bash
./seed-forest.sh
```

**Run this every session.** It ensures:
- Sembly and forester tools are built and available
- Knowledge index is current
- All repositories are present

**If you skip this:**
- ❌ Sembly and forester commands will fail
- ❌ Documentation search won't work
- ❌ You'll reference outdated code

### Step 2: Read Core Documentation

```bash
cat ./docs/architecture.md
cat ./docs/build-system.md
```

**Always read these files.** They explain:
- How kits and variants relate
- The build system workflow
- Component dependencies
- Common development patterns

Takes 30 seconds, prevents hours of mistakes.

**If you skip this:**
- ❌ Will guess instead of citing facts
- ❌ Will misunderstand component relationships
- ❌ Will give outdated or incorrect guidance

### Step 3: Read Skills Documentation

```bash
cat skills/README.md
```

**REQUIRED: Read the entire skills/README.md file.** It contains:
- The skill announcement protocol (mandatory format)
- Complete index of available skills with descriptions
- When and how to use each skill

**Use the index in skills/README.md to determine if a skill exists for your task.**

**If you skip reading skills/README.md:**
- ❌ Won't know the correct announcement format
- ❌ Won't know which skills are available
- ❌ Will reinvent tested procedures incorrectly
- ❌ Will miss validation steps

### Verification Checklist

Before answering, confirm you completed:
- [ ] Ran `./seed-forest.sh` and verified output
- [ ] Read `./docs/architecture.md`
- [ ] Read `./docs/build-system.md`
- [ ] Read `./skills/README.md` (contains protocol and skill index)

## Multi-Step Workflows

For complex tasks with multiple steps:

1. **Use todolist functionality** if available
2. Create a task list with clear, actionable items
3. Update status as you progress
4. Mark tasks complete when verified

This helps track progress, prevents skipped steps, and provides clear status updates.

## Documentation Research

**ANY question about how Bottlerocket works requires research.**

Process:
1. **Read `skills/README.md`** to see the skill index
2. Check if a skill exists for your task (e.g., `research-with-citations`)
3. If yes: Follow the protocol from skills/README.md
4. If no: Use `sembly search` to find relevant documentation
5. Always cite sources in your response

Never guess or rely on training data for Bottlerocket-specific questions.

## Sembly Usage

Sembly provides semantic search for Bottlerocket documentation.

**⚠️ CRITICAL: Always run from forest root directory**

```bash
# Build or rebuild the search index
sembly build

# Search documentation semantically
sembly search "boot process"
sembly search "systemd targets"

# Check index status
sembly status

# Update index incrementally
sembly update

# Rebuild from scratch
sembly rebuild
```

## Forester Usage

Forester provides registry management for local kit development.

**⚠️ CRITICAL: Always run from forest root directory**

```bash
# Start local OCI registry for kit development
forester registry start

# Check if registry is running
forester registry status

# List published images
forester registry list
```

## Common Patterns

### Answering "How does X work?" Questions

1. Check for `research-with-citations` skill
2. Use `sembly search` to find relevant docs
3. Read the source files
4. Cite specific files and line numbers in your answer

### Making Code Changes

1. Check for applicable skills (e.g., `add-package-to-kit`)
2. Read relevant documentation first
3. Understand the component's role in the system
4. Make minimal, focused changes
5. Verify changes build successfully

### Building and Testing

1. Understand the dependency chain (kit → registry → variant)
2. Use Makefile targets, not direct twoliter commands
3. Verify each step before proceeding
4. Check build artifacts exist

## Skills Protocol

**IMPORTANT: Read `skills/README.md` for the complete protocol and skill index.**

Skills are tested procedures for common tasks.

**When a skill exists for your task, you MUST use it.**

The skills/README.md file contains:
- The exact announcement format (mandatory)
- Complete three-step protocol
- Index of available skills with descriptions
- When to use each skill

**Use the index in skills/README.md to determine which skill applies to your task.**

**Do not skip reading skills/README.md** - it contains critical information not duplicated here.

### Common Skills

From the skills/README.md index:
- `research-with-citations` - Answer questions about Bottlerocket
- `add-package-to-kit` - Add a new package to a kit
- `update-package-version` - Update an existing package

(See skills/README.md for complete list and descriptions)

## Error Recovery

If something goes wrong:

1. **Don't guess** - Check documentation or ask for clarification
2. **Verify assumptions** - Re-read relevant docs
3. **Check build logs** - Errors often indicate what's wrong
4. **Start fresh** - Run `./seed-forest.sh` again if needed

## Best Practices

- **Always cite sources** - Reference specific files and line numbers
- **Verify before claiming** - Check that files exist, commands work, builds succeed
- **Use exact commands** - Don't paraphrase or modify tested procedures
- **Ask when uncertain** - Better to ask than to give wrong information
- **Keep responses focused** - Answer the specific question asked
- **Update documentation** - If you find gaps, note them for improvement

## Anti-Patterns to Avoid

- ❌ Skipping the mandatory workflow steps
- ❌ Guessing about Bottlerocket internals
- ❌ Ignoring available skills
- ❌ Running forester from wrong directory
- ❌ Using `twoliter` directly instead of Makefile targets
- ❌ Making changes without understanding the system
- ❌ Claiming success without verification

## Getting Help

If you're stuck:
1. Re-read the relevant documentation
2. Search for similar examples in the codebase
3. Check if a skill exists for the task
4. Ask the user for clarification

Remember: The forest is designed to help you succeed. Use the tools provided.
