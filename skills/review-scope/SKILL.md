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
- <file>:<line> - <description in your own words>
- <file> - <description>

[Optional] THEMES: <pattern worth calling out>
```

## Violation Format

Describe violations naturally with location and clear explanation:

```
- src/parser.rs:45 - Added logging infrastructure not in commit scope
- src/config.rs - Modified file outside allowed set
```

If you notice patterns across violations, call them out:

```
THEMES: Multiple changes extend beyond the stated requirements into error handling refactoring
```

## Minimal Change Bias Check

The implementing agent is biased to make the smallest change possible to achieve its goal.
After identifying violations, consider:

> Do the violations point to any potential refactoring that would make the code more maintainable?
> Did the implementor resist a better structural solution in favor of a quick fix?

If yes, note this in a REFACTORING section:
```
REFACTORING: <description of structural improvement the implementor avoided>
```

This is not a violation—it's a flag for the orchestrator to consider.

## What This Skill Does NOT Do

- Suggest improvements unrelated to violations
- Provide general constructive feedback
- Review code style (use review-style for that)

This is a binary gate: ACCEPT or VIOLATIONS. Refactoring notes are optional flags, not blockers.
