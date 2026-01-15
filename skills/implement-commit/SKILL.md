---
name: implement-commit
description: Implement commits from an implementation plan using a TDD pipeline with parallel execution
---

# Implement Commit Skill

## Purpose

Guide an orchestrating agent through implementing commits from an implementation plan.
Uses a TDD pipeline with phase isolation, worktree parallelization, and efficient validation.

## When to Use

- Implementation plan exists and is approved
- Ready to write code for one or more commits
- Want structured TDD workflow with verification

## Prerequisites

- Implementation plan at `docs/features/NNNN-feature-name/implementation-plan.md`
- Design and test plan approved
- Style guides available at `docs/style/rust-*.md`

## Orchestrator Efficiency Principles

**Your context is expensive. Subagent context is cheap.**

1. **Validate via exit codes, not output** - If `cargo check` passes, the code compiles. Don't read it.
2. **Trust subagents** - They report success/failure. Only investigate failures.
3. **Batch independent work** - Spawn parallel commits in parallel.
4. **Pass context forward** - Refer to relevant context files so subagents don't re-discover unless they must.
5. **Read only on failure** - When something breaks, then investigate.

## Procedure

### 1. Load Implementation Plan

Read the implementation plan once. Extract:
- Dependency graph (at top of plan)
- Commit checklist
- Per-commit details (files, requirements, constraints, tests)

### 2. Identify Parallelizable Commits

The implementation plan's dependency graph shows which commits can run in parallel.
Commits with no unfinished dependencies can start immediately.

```
Example dependency graph:
  Commit 1: (none)
  Commit 2: 1
  Commit 3: 1
  Commit 4: 2, 3

Parallel groups:
  Group 1: [1]
  Group 2: [2, 3]  <- parallel after 1 completes
  Group 3: [4]     <- after 2 and 3 complete
```

### 3. Execute Commits via TDD Pipeline

For each commit (or parallel batch), run the five-phase pipeline.

**Each commit gets its own worktree** to enable parallel execution:
```bash
git worktree add /tmp/commit-N -b impl-commit-N HEAD
```

#### Phase 1: Designer

Create module structure, types, and function stubs.

**Style guide:** `docs/style/rust-design.md`

**Input to subagent:**
- context_files: `docs/style/rust-design.md`, relevant existing modules
- context_data: commit details (files, types, signatures from plan)

**Success criteria:** `cargo check` exits 0

**Subagent task:** Create the module skeleton with types and stub functions that compile but don't implement logic (use `todo!()` or `unimplemented!()`).

#### Phase 2: Tester

Write tests against the stubs (TDD red phase).

**Style guide:** `docs/style/rust-test.md`

**Input to subagent:**
- context_files: `docs/style/rust-test.md`, test plan section for this commit
- context_data: commit details, test names from plan, requirements being tested

**Success criteria:** `cargo test` compiles (exits 0 or with test failures, not compile errors)

**Subagent task:** Write tests that exercise the stubs. Tests should fail (red) because stubs aren't implemented. Tests must compile.

#### Phase 3: Implementor

Make tests pass (TDD green phase).

**Style guide:** `docs/style/rust-impl.md`

**Input to subagent:**
- context_files: `docs/style/rust-impl.md`
- context_data: commit details, constraints from plan

**Constraints:**
- Cannot modify test files
- Cannot change public API signatures
- Must use `agent_feedback()` if tests are problematic

**Success criteria:** `cargo test` exits 0 (all tests pass)

**Subagent task:** Implement the logic to make all tests pass without modifying tests or public API.

#### Phase 4: Reviewers (Parallel)

Two independent reviews run in parallel using dedicated skills.

**Spawn both reviewers via spawn_batch:**

```python
results = spawn_batch([
    {
        "prompt": """USING SKILL "review-scope"
        
        Review changes for scope compliance.""",
        "context_files": [
            "skills/review-scope/SKILL.md",
            "docs/features/NNNN-feature-name/implementation-plan.md",
            "docs/features/NNNN-feature-name/design.md"
        ],
        "context_data": {
            "commit_details": "<commit message and scope>",
            "requirements": ["REQ-001", "REQ-002"],
            "constraints": ["CC-001"],
            "allowed_files": ["src/foo.rs", "src/bar.rs"]
        },
        "cwd": "/tmp/commit-N",
        "allow_tools": True
    },
    {
        "prompt": """USING SKILL "review-style"
        
        Review changes for style compliance.""",
        "context_files": [
            "skills/review-style/SKILL.md",
            "docs/style/rust-design.md",
            "docs/style/rust-impl.md",
            "docs/style/rust-test.md"
        ],
        "context_data": {
            "phase": "impl",
            "changed_files": ["src/foo.rs", "src/bar.rs"]
        },
        "cwd": "/tmp/commit-N",
        "allow_tools": True
    }
])
```

**Success criteria:** Both reviewers respond with `ACCEPT`

**On any REJECT:** Collect all violations from both reviewers, return to Phase 3 (Implementor) with combined violation list. Max 2 cycles.

#### Phase 5: Close

Finalize the commit.

1. Commit changes in worktree:
   ```bash
   cd /tmp/commit-N
   git add -A && git commit -m "<commit message from plan>"
   ```

2. Cherry-pick to main branch (in dependency order):
   ```bash
   cd <main-worktree>
   git cherry-pick <commit-sha>
   ```

3. Verify build still passes: `cargo check`

4. Mark commit complete in plan (check the box)

5. Cleanup worktree:
   ```bash
   git worktree remove /tmp/commit-N
   git branch -D impl-commit-N
   ```

### 4. Handle Failures

#### Phase Failure (Designer, Tester, Implementor)

1. Retry with error context (max 2 retries)
2. If still failing: escalate to human with error details

#### Verifier Rejection

1. Return to Implementor with rejection reason
2. Max 2 rejection cycles
3. If still rejected: escalate to human

#### Cherry-pick Conflict

1. Resolve conflict or escalate to human
2. Re-run `cargo check` after resolution

### 5. Parallel Execution Pattern

For commits that can run in parallel:

1. Create worktrees for all commits in the batch
2. Spawn Designer phase for all in parallel
3. Wait for all to complete
4. Spawn Tester phase for all in parallel
5. Continue through phases
6. Cherry-pick in dependency order (sequential)

**Important:** Cherry-picking must be sequential in dependency order, even if implementation was parallel.

## Context Handoff Reference

### What to pass via context_files:

| Phase | Style Guide | Other Files |
|-------|-------------|-------------|
| Designer | `docs/style/rust-design.md` | Existing modules being extended |
| Tester | `docs/style/rust-test.md` | Test plan section for this commit |
| Implementor | `docs/style/rust-impl.md` | (none - work in worktree) |
| Verifier | (none) | Implementation plan, design doc |

Plus: relevant existing source files the subagent needs to understand

### What to pass via context_data:
- Commit number and message
- Files to create/modify (from plan)
- Requirements addressed (REQ-*)
- Constraints to satisfy (CC-*)
- Test names to implement
- Error context (on retry)
- Rejection reason (on verifier reject)

### What subagents should NOT need:
- Full implementation plan (orchestrator extracts relevant parts)
- Other commits' details
- Design doc (unless Verifier)

## Validation

After completing all commits:

```bash
# Full test suite passes
cargo test

# All commits checked off in plan
grep -E "^\- \[x\]" docs/features/NNNN-feature-name/implementation-plan.md

# No unchecked commits remain
grep -E "^\- \[ \]" docs/features/NNNN-feature-name/implementation-plan.md
```

## Common Issues

**Subagent modifies tests:** Implementor phase must be explicitly told tests are read-only. Use `agent_feedback()` if tests are wrong.

**Verifier too strict:** Verifier should check scope and constraints, not style preferences. Rephrase verification prompt if rejecting valid work.

**Cherry-pick order wrong:** Always cherry-pick in dependency order from the plan, not completion order.

**Worktree conflicts:** Each commit needs a unique worktree. Clean up failed worktrees before retrying.

**Orchestrator reading code:** Trust exit codes. Only read files when debugging failures.

## Example Orchestration Flow

```
1. Read implementation plan
2. Parse dependency graph: {1: [], 2: [1], 3: [1], 4: [2,3]}
3. Start Commit 1 (no deps)
   - Designer -> cargo check passes
   - Tester -> cargo test compiles
   - Implementor -> cargo test passes
   - Verifier -> ACCEPT
   - Close -> cherry-pick, cleanup
4. Start Commits 2 and 3 in parallel (both depend only on 1)
   - [parallel Designer phases]
   - [parallel Tester phases]
   - [parallel Implementor phases]
   - [parallel Verifier phases]
   - Close 2 -> cherry-pick (2 before 3 if 3 depends on 2, else either order)
   - Close 3 -> cherry-pick
5. Start Commit 4 (depends on 2 and 3)
   - [full pipeline]
   - Close -> cherry-pick, cleanup
6. Validate: cargo test, check plan completion
```
