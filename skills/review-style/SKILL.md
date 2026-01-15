---
name: review-style
description: Verify code follows project style guides without providing improvement suggestions
---

# Review Style Skill

## Purpose

Verify that code follows all project style guides.
This is a pass/fail gate, not a feedback session.

## When to Use

- TDD verification phase
- PR review for style compliance
- Any code review requiring style validation

## Inputs Required

### context_files
- `docs/style/rust-design.md` - Design phase rules
- `docs/style/rust-impl.md` - Implementation phase rules  
- `docs/style/rust-test.md` - Test phase rules
- Changed files to review

### context_data
- `phase`: Which phase (design/impl/test) - determines primary style guide
- `changed_files`: List of files to review

## Style Guides (Authoritative Sources)

The style guides in `docs/style/` are the ONLY source of truth:
- `docs/style/rust-design.md` - Design phase rules
- `docs/style/rust-impl.md` - Implementation phase rules
- `docs/style/rust-test.md` - Test phase rules

Read the applicable guide(s) and apply your judgment. Do not rely on summaries.

## Procedure

### 1. Identify Applicable Rules

Based on the phase and file types:
- Design phase: rust-design.md rules
- Impl phase: rust-impl.md rules
- Test phase: rust-test.md rules
- Test files always get rust-test.md rules regardless of phase

### 2. Review Each File

Check against applicable style guide rules.
Only flag clear violations, not style preferences.

### 3. Report Result

**If all checks pass:**
```
ACCEPT
```

**If any check fails, list violations:**
```
VIOLATIONS:
- <file>:<line> - <description in your own words>
- <file>:<line> - <description>

[Optional] THEMES: <pattern worth calling out>
```

## Violation Format

Describe violations naturally with location and clear explanation:

```
- src/config.rs:23 - snafu selectors imported at module level; should be inside function
- src/parser.rs:45 - missing Given/When/Then comments in test
```

If you notice patterns across violations, call them out:

```
THEMES: Several violations stem from inconsistent error handling approach
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
- Review scope/correctness (use review-scope for that)
- Flag style preferences not in guides

This is a binary gate: ACCEPT or VIOLATIONS. Refactoring notes are optional flags, not blockers.
