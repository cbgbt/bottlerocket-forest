---
name: review-scope
description: Verify code changes match intended scope and requirements without exceeding boundaries
---

# Review Scope Skill

## Purpose

Verify that code changes do exactly what they're supposed to do—no more, no less.
This is a pass/fail gate, not a feedback session.

## When to Use

- TDD verification phase
- PR review for scope compliance
- Any code review requiring scope validation

## Inputs Required

### context_files
- Implementation plan or requirements document
- Design document (if available)
- Changed files to review

### context_data
- `commit_details`: What this change is supposed to do
- `requirements`: List of REQ-* being addressed
- `constraints`: List of CC-* constraints that must be satisfied
- `allowed_files`: Files that may be modified (scope boundary)

## Procedure

### 1. Establish Scope Boundaries

From the inputs, identify:
- What MUST be done (requirements)
- What MUST NOT be done (constraints)
- What files MAY be changed
- What behavior is expected

### 2. Review Changes

For each changed file:
1. Is this file in the allowed set?
2. Do changes address stated requirements?
3. Are there changes unrelated to requirements?
4. Do changes violate any constraints?

### 3. Report Result

**If all checks pass:**
```
ACCEPT
```

**If any check fails, list violations:**
```
VIOLATIONS:
- [SCOPE-001] <file>:<line> - <violation description>
- [SCOPE-002] <file> - <violation description>
```

## Violation Categories

| Code | Category | Description |
|------|----------|-------------|
| SCOPE-001 | Out of scope file | Modified file not in allowed set |
| SCOPE-002 | Missing requirement | Required behavior not implemented |
| SCOPE-003 | Scope creep | Changes beyond stated requirements |
| SCOPE-004 | Constraint violation | Violates explicit constraint |
| SCOPE-005 | API change | Public API modified when not allowed |

## Violation Format

Each violation must include:
- Code (SCOPE-NNN)
- Location (file, line if applicable)
- Concrete description of what's wrong

**Good violation:**
```
- [SCOPE-003] src/parser.rs:45 - Added logging infrastructure; not in commit scope
```

**Bad violation:**
```
- The code does extra stuff
```

## What This Skill Does NOT Do

- Suggest improvements
- Provide constructive feedback
- Review code style (use review-style for that)
- Evaluate code quality beyond scope

This is a binary gate: ACCEPT or VIOLATIONS. Nothing else.
