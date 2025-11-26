# Feature Name - Implementation Plan

## Commit Checklist

High-level tracking of implementation progress.
Each commit should be atomic, buildable, and tested.

- [ ] **Commit 1**: Brief description of change
- [ ] **Commit 2**: Brief description of change
- [ ] **Commit 3**: Brief description of change

## Commit Details

### Phase 1: Foundation

Commits that establish the groundwork.
Later phases depend on these being complete.

---

#### Commit 1: Title describing the change

**Summary**: One paragraph explaining what this commit accomplishes and why it's needed.

**Files Changed**:
- `path/to/file.rs` - What changes
- `path/to/other.rs` - What changes

**Key Changes**:
- Specific change 1
- Specific change 2

**Acceptance Criteria** *(for commits implementing critical constraints)*:
- [ ] Verifiable criterion from design doc (e.g., "SQL query contains JOIN")
- [ ] Another measurable requirement

**Anti-patterns** *(reviewer: reject if present)*:
- What NOT to do (from design doc CC table)

**Testing**: How to verify this commit works (unit tests, manual verification, etc.)

**Dependencies**: None (first commit) or list prior commits

---

#### Commit 2: Title describing the change

**Summary**: One paragraph explaining what this commit accomplishes.

**Files Changed**:
- `path/to/file.rs` - What changes

**Key Changes**:
- Specific change 1

**Testing**: How to verify this commit works.

**Dependencies**: Commit 1

---

### Phase 2: Core Implementation

Commits that implement the main functionality.

---

#### Commit 3: Title describing the change

**Summary**: One paragraph explaining what this commit accomplishes.

**Files Changed**:
- `path/to/file.rs` - What changes

**Key Changes**:
- Specific change 1

**Testing**: How to verify this commit works.

**Dependencies**: Commits 1, 2

---

## Parallelization Notes

*Optional section documenting which commits could be implemented in parallel.*

- Commits X and Y have no dependencies on each other and can be developed simultaneously
- Phase 2 commits require Phase 1 to be complete

## Open Questions

*Track decisions that need to be made during implementation.*

- [ ] Question 1?
- [ ] Question 2?
