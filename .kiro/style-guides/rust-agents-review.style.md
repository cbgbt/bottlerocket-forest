---
style-guide:
  description: "Rust Style Guide - Code Review (All Phases)"
  prefer-in-contexts:
    - "Rust code review"
    - "Style checking"
    - "Bug detection"
---

# Rust Style Guide - Code Review

Combined rules for reviewing Rust code. Check adherence across design, test, and implementation phases.

## Project Crate Choices (Non-Negotiable)

| Purpose | Required | Forbidden |
|---------|----------|-----------|
| Error handling | `snafu` | `thiserror` |
| Builders | `bon` | manual builders |
| Validated newtypes | `nutype` | manual validation |
| Measurements | `uom` | raw numbers for quantities |
| Logging | `tracing` | `log` |
| Config format | `toml` | JSON/YAML |

## Error Handling Review

Check for:
- [ ] Error-per-operation pattern (each fallible fn has its own error type)
- [ ] `#[snafu(module)]` on error enums
- [ ] No `{source}` in display messages
- [ ] Context selectors imported inside functions, not at module level
- [ ] `snafu::ensure!` for validation, not `if/return Err`
- [ ] Traits use associated error types, not shared concrete errors

## Type Design Review

Check for:
- [ ] `nutype` for validated newtypes (not manual impl)
- [ ] `bon` builders REQUIRED for composite structs (2+ fields)
- [ ] `#[builder(on(_, into))]` on all builder structs
- [ ] `#[non_exhaustive]` REQUIRED on builder structs - prevents direct construction
- [ ] NO `#[builder(default)]` on `Option<T>` fields - they auto-default to None
- [ ] `#[builder(default = Utc::now())]` for timestamp fields that should default to now
- [ ] Only newtypes (single-field tuple structs) get `new()` constructors
- [ ] `uom` for physical quantities
- [ ] `path-clean` for path normalization
- [ ] Domain types over primitives in APIs
- [ ] Streaming support (`impl Read`) for large data

## Documentation Review

Check for:
- [ ] Doc comments on all public items
- [ ] Summary line < 120 chars
- [ ] No `# Errors` sections
- [ ] No parameter/return docs in docstrings
- [ ] No vague terms: "robust", "efficient", "handles", "manages"

## Serialization Review

Check for:
- [ ] `#[serde(rename_all = "kebab-case", deny_unknown_fields)]`
- [ ] Typed config structs with domain types (not raw strings)

## Test Review

Check for:
- [ ] Given/When/Then comments in every test
- [ ] `test_case` for parameterized tests (not copy-pasted tests)
- [ ] `#[automock]` on traits that need mocking
- [ ] `assert!(matches!())` for pattern matching
- [ ] Whole-struct comparison, not field-by-field
- [ ] No tests for compiler-generated derives
- [ ] Tests in `mod test` at file bottom

## Implementation Review

Check for:
- [ ] No `unwrap()`/`expect()` in production code (ok in tests)
- [ ] `.context()` with `?` operator
- [ ] Combinators over verbose match blocks
- [ ] No `.to_string()` with bon builders (auto-converts)
- [ ] `#[instrument]` on key functions
- [ ] Comments explain *why*, not *what*
- [ ] `SAFETY:` comments on unsafe blocks

## Import Review

Check for:
- [ ] All imports at top (except snafu context selectors)
- [ ] Grouped items from same module
- [ ] Not grouped across modules
- [ ] Crate-local types unqualified
- [ ] Ambiguous third-party types qualified

## Module Organization Review

Check for:
- [ ] Domain-specific modules, not `types.rs` grab-bags
- [ ] Directory modules when > 500 LOC
- [ ] Error types near their operations

## Common Bugs to Flag

- Panics in production code paths
- Missing error context (bare `?` without `.context()`)
- Raw strings where domain types exist
- Manual path manipulation instead of `path-clean`
- Shared error types across trait implementations
- Function-level imports (except snafu selectors)
- Env vars for config instead of CLI args
- `#[builder(default)]` on Option fields (redundant - bon auto-defaults to None)
- Missing `#[non_exhaustive]` on builder structs
- `new()` constructors on multi-field structs (should use builder)
- `if condition { return Err(...) }` pattern - MUST use `snafu::ensure!` instead
