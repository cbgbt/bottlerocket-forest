---
name: propose-implementation-plan
description: Create an implementation plan with atomic commits that build toward a complete feature
---

# Propose Implementation Plan Skill

## Purpose

Create a detailed implementation plan that breaks a feature into atomic, reviewable commits.
The plan serves as a roadmap for implementation, ensuring each commit is self-contained, tested, and buildable.

## When to Use

- Feature design document exists and is approved
- Ready to begin implementation
- Need to coordinate work or track progress
- Want to ensure commits are appropriately sized

## Prerequisites

- Feature design exists in `docs/features/NNNN-feature-name/design.md`
- Design has been reviewed and approved
- Implementor understands the technical approach

## Procedure

### 1. Verify Design Exists

```bash
ls docs/features/NNNN-feature-name/design.md
```

If it doesn't exist, use `propose-feature-design` skill first.

### 2. Create Planning Directory and Copy Template

Implementation plans go in `planning/` (gitignored scratch space), not `docs/features/`.

```bash
mkdir -p planning/NNNN-feature-name
cp docs/features/0000-templates/implementation-plan.md planning/NNNN-feature-name/
```

### 3. Study the Design Document

Read the design thoroughly, noting:
- Module structure and affected files
- Dependencies between components
- Migration requirements from current state
- Testing strategy

### 3a. Extract Critical Constraints

Review the design's Critical Constraints table (CC-1, CC-2, etc.).
For each constraint:
1. Identify which commit(s) implement it
2. Copy the constraint and anti-pattern to those commits' Acceptance Criteria
3. These become explicit review checkpoints

**This step prevents the most common implementation mistakes.**
If the design lacks critical constraints, ask for clarification before proceeding.

### 4. Identify Natural Boundaries

Look for logical separation points:
- New types that can be introduced independently
- Trait definitions separate from implementations
- Schema changes separate from code using them
- Tests that can be written before implementation

### 5. Plan Commits Following the Atomic Commit Rules

**Each commit MUST be:**

1. **Buildable** - The project compiles after this commit
2. **Tested** - New code has tests; existing tests pass (or are explicitly disabled with TODO)
3. **Focused** - Does one logical thing
4. **Reviewable** - Small enough to review in one sitting (target: <400 lines changed)

**Each commit SHOULD:**

1. **Be independently valuable** - Provides some benefit even if later commits aren't merged
2. **Have a clear purpose** - The commit message explains why, not just what
3. **Minimize risk** - Smaller commits are easier to revert if problems arise

### 5a. Handle Test Breakage During Refactors

When a commit breaks tests in distant modules (e.g., schema changes that break integration tests), **do not leave broken tests**.
Instead, explicitly disable them with a TODO that references when they should be re-enabled.

**Pattern for disabling tests:**

```rust
// TODO: Re-enable in Commit 9a after updating domain types
#[cfg(all(test, feature = "enable_broken_tests"))]
mod tests {
    // ...
}
```

Or for individual tests:

```rust
#[test]
#[ignore] // TODO: Re-enable in Commit 12a after facade integration
fn test_search_returns_results() {
    // ...
}
```

**When planning commits that break existing tests:**

1. **Identify which tests will break** - Note them in the commit description
2. **Disable tests explicitly** - Use `#[ignore]` or feature flags, not deletion
3. **Add a TODO comment** - Reference the specific commit that will re-enable them
4. **Plan a re-enablement commit** - Add a commit (e.g., "9a", "12a") that fixes and re-enables the tests
5. **Place re-enablement after dependencies are ready** - The re-enable commit comes after all changes needed to fix the tests

**Example from a real plan:**

```markdown
- [ ] **Commit 4**: Update database schema (disables incompatible tests)
- [ ] **Commit 8**: Update IndexedFile to include new fields
- [ ] **Commit 9**: Update Chunk to use content-addressed storage
- [ ] **Commit 9a**: Re-enable and fix storage layer tests  ← Re-enablement commit
```

This approach:
- Keeps the build green at every commit
- Makes test debt explicit and trackable
- Ensures tests aren't forgotten
- Gives coders clear guidance on when to fix tests

### 6. Size Commits Appropriately

**Too Small** (avoid):
- Adding a single import
- Renaming one variable
- Adding an empty module

**Too Large** (avoid):
- Entire feature in one commit
- Multiple unrelated changes
- Changes that take days to review

**Just Right** (target):
- Add a new type with its tests (~50-200 lines)
- Implement a trait for one adapter (~100-300 lines)
- Add a new CLI command with tests (~100-300 lines)
- Refactor a module to prepare for new feature (~100-400 lines)

### 7. Order Commits by Dependency

Structure commits so each builds on previous work:

```
Phase 1: Foundation
  Commit 1: Add new types (no behavior yet)
  Commit 2: Add trait definitions (interfaces only)
  
Phase 2: Core Implementation  
  Commit 3: Implement trait for primary adapter
  Commit 4: Wire into application layer
  
Phase 3: Integration
  Commit 5: Add CLI commands
  Commit 6: Add integration tests
```

### 8. Write the Commit Checklist

Create the high-level checklist at the top of the document.
Keep descriptions to one line—details go in the commit sections.

```markdown
- [ ] **Commit 1**: Add Context and ContextId types
- [ ] **Commit 2**: Add ContextRepository trait
- [ ] **Commit 3**: Implement SqliteContextRepository
```

### 9. Write Detailed Commit Descriptions

For each commit, document:

**Summary**: What and why in one paragraph.

**Files Changed**: List files with brief description of changes.

**Key Changes**: Bullet points of specific modifications.

**Acceptance Criteria** *(for commits implementing critical constraints)*:
- Copy from design doc's Critical Constraints table
- Make them checkable (e.g., "SQL query contains JOIN to indexed_files")
- Include anti-patterns for reviewers to reject

**Testing**: How to verify the commit works.

**Dependencies**: Which prior commits must be complete.

### 10. Identify Parallelization Opportunities

Note which commits have no dependencies on each other.
These can be implemented simultaneously by different people or in any order.

```markdown
## Parallelization Notes

- Commits 3 and 4 can be developed in parallel (both depend only on 1-2)
- Phase 2 requires all of Phase 1 to be complete
```

### 11. Document Open Questions

Track decisions that need resolution during implementation:

```markdown
## Open Questions

- [ ] Should we use async for the repository trait?
- [ ] What's the migration strategy for existing databases?
```

## Commit Sizing Guidelines

### Indicators a Commit is Too Large

- More than 500 lines changed
- Touches more than 5 files
- Takes more than 2 hours to implement
- Commit message needs multiple paragraphs to explain
- Reviewer asks "can this be split up?"

### Indicators a Commit is Too Small

- Less than 20 lines changed
- No tests included
- Doesn't compile on its own
- Commit message is longer than the change
- Creates dead code that's only used in later commits

### Splitting Large Changes

If a change seems too large, look for:

1. **Type extraction**: Can new types be introduced first?
2. **Interface first**: Can traits be defined before implementations?
3. **Refactor then change**: Can existing code be restructured first?
4. **Test scaffolding**: Can test infrastructure be added separately?
5. **Feature flags**: Can new code be added but disabled?

### Example: Splitting a Large Feature

Instead of one commit "Add multi-context support":

```
Commit 1: Add Context type and ContextId newtype
Commit 2: Add ContextRepository trait definition
Commit 3: Update database schema with contexts table
Commit 4: Implement SqliteContextRepository
Commit 5: Add context discovery (workspace walk)
Commit 6: Wire context resolution into facade
Commit 7: Add 'context list' CLI command
Commit 8: Add 'context remove' CLI command
Commit 9: Update indexing to use context-scoped storage
Commit 10: Update search to filter by context
```

Each commit is reviewable, testable, and buildable.

## Validation

Verify the implementation plan:

```bash
# Check file exists
ls planning/NNNN-feature-name/implementation-plan.md

# Verify it has the checklist
grep -E "^\- \[ \]" planning/NNNN-feature-name/implementation-plan.md

# Count commits planned
grep -c "^#### Commit" planning/NNNN-feature-name/implementation-plan.md
```

Review the plan for:
- [ ] Each commit is atomic and buildable
- [ ] Commits are appropriately sized (target <400 lines)
- [ ] Dependencies are clearly stated
- [ ] Testing approach is documented for each commit
- [ ] Phases group related work logically

## Common Issues

**Commits too large**: Split by type/trait/implementation boundaries.

**Commits too small**: Combine related changes that don't make sense alone.

**Unclear dependencies**: Draw a dependency graph if needed.

**Missing tests**: Every commit should include tests for new code.

**Vague descriptions**: Be specific about files and changes.

**Refactor breaks distant tests**: Don't leave tests broken or delete them. Disable with `#[ignore]` or feature flag, add a TODO referencing the re-enablement commit, and plan a specific commit to fix and re-enable them. See section 5a.

## Next Steps

After creating the implementation plan:
1. Review with team for feasibility and sizing
2. Adjust based on feedback
3. Begin implementation, checking off commits as completed
4. Update plan if implementation reveals needed changes
5. Use the checklist to track progress
