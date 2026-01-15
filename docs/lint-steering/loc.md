# ⚡ REFACTOR_PHOENIX — LOC Limit

This lint guards against minimal-change bias.

## The Trap

When a file exceeds the size limit, the tempting fix is:
- Move one function somewhere else
- Split off "just enough" to pass

This creates arbitrary boundaries that don't reflect real abstractions.

## Instead, Ask

1. What are the distinct responsibilities in this file?
2. Which types/functions form a cohesive unit?
3. What would a new contributor expect to find here vs. elsewhere?

## Patterns

### Internal Decomposition (preferred for large structs)

If a struct has too many methods, decompose into internal components:

**Before:** `facade.rs` (900 lines)
- KnowledgeIndex with build(), search(), gc(), etc. all in one file

**After:**
- `facade/mod.rs` (500 lines): KnowledgeIndex struct + delegating methods
- `facade/builder.rs`: KnowledgeIndex::build() calls builder::build()
- `facade/searcher.rs`: KnowledgeIndex has a searcher::SearchOperation

Components are private implementation details. Public API unchanged.

⚠️ DO NOT split a struct's impl blocks across files without decomposition.
Scattered impl blocks fragment semantic understanding.

### Split by Operation

If a module has multiple logical operations, split them:

**Before:** `facade.rs` (700 lines)
- build(), rebuild(), update(), clear() + all tests

**After:**
- `facade/build.rs` (400 lines): build(), rebuild() + their tests
- `facade/update.rs` (150 lines): update() + its tests
- `facade/clear.rs` (150 lines): clear() + its tests

Tests stay co-located with their implementation.

### Use test_case for Parameterized Tests

Consolidate similar tests that differ only in inputs:

```rust
#[test_case("" ; "empty string")]
#[test_case("   " ; "whitespace only")]
fn rejects_blank_input(input: &str) { ... }
```

## Constraints

- Preserve Given/When/Then comments in tests (required by style guide)
- Keep tests co-located with implementation
- DO NOT delete docstrings or comments to reduce line count

## The Test

After refactoring, each new file should have a clear one-sentence purpose.
If you can't state it, the boundary is arbitrary.
