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

## Style Guide Summary

### rust-design.md (Design Phase)
- Error types: snafu with `#[snafu(module)]`, error-per-operation
- Builders: bon with `#[non_exhaustive]`, per-field `#[builder(into)]`
- Newtypes: nutype for validation
- Module organization: domain-specific, not grab-bags
- Documentation: doc comments on public items, no `# Errors` sections

### rust-impl.md (Implementation Phase)
- Snafu selectors: import inside functions, not module level
- Validation: use `snafu::ensure!`, not if/return
- Imports: all at top, group from same module
- No panics: never `unwrap()`/`expect()` in production
- Logging: tracing with `#[instrument]`

### rust-test.md (Test Phase)
- Structure: Given/When/Then comments required
- Location: unit tests in same file, `#[cfg(test)] mod test`
- Parameterized: use test_case for multiple inputs
- Assertions: prefer `assert!(matches!())` for patterns
- No useless tests: don't test third-party libraries

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
- [STYLE-001] <file>:<line> - <rule violated>: <description>
- [STYLE-002] <file>:<line> - <rule violated>: <description>
```

## Violation Categories

| Code | Category | Description |
|------|----------|-------------|
| STYLE-001 | Error handling | Snafu usage violations |
| STYLE-002 | Builder pattern | Bon usage violations |
| STYLE-003 | Import organization | Import placement/grouping |
| STYLE-004 | Documentation | Missing/incorrect doc comments |
| STYLE-005 | Test structure | Missing Given/When/Then, wrong location |
| STYLE-006 | Panic in production | unwrap()/expect() in non-test code |
| STYLE-007 | Naming | Incorrect naming conventions |
| STYLE-008 | Module organization | Wrong file structure |

## Violation Format

Each violation must include:
- Code (STYLE-NNN)
- Location (file:line)
- Rule being violated
- Concrete description

**Good violation:**
```
- [STYLE-001] src/config.rs:23 - snafu selectors at module level: `use config_error::*` should be inside function
```

**Bad violation:**
```
- The imports look wrong
```

## What This Skill Does NOT Do

- Suggest improvements
- Provide constructive feedback
- Review scope/correctness (use review-scope for that)
- Flag style preferences not in guides

This is a binary gate: ACCEPT or VIOLATIONS. Nothing else.
