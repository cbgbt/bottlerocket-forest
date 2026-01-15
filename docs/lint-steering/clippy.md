# ⚡ REFACTOR_PHOENIX — Clippy

This lint guards against minimal-change bias.

## The Trap

When clippy warns about complexity, the tempting fix is:
- Add `#[allow(...)]` to silence it
- Extract a helper that's only called once
- Suppress with a minor restructure

This hides complexity without reducing it.

## Instead, Ask

1. What are the distinct phases of this function?
2. Is this function doing multiple jobs?
3. Does this type represent a missing abstraction?

## Patterns

### Complexity Warnings (cognitive_complexity, too_many_arguments)

These signal functions doing too much.

- Should this function compose smaller abstractions?
- Can we extract behavior into methods on a type?
- Weigh the cost of abstraction against the cost of separation.

### Type Warnings (type_complexity)

Complex types suggest missing abstractions.

Introduce newtypes or domain-specific types to clarify intent:

```rust
// Before: HashMap<String, Vec<(PathBuf, usize, String)>>
// After:
struct SearchResults(HashMap<String, Vec<Match>>);
struct Match { path: PathBuf, line: usize, content: String }
```

### Clone/Copy Warnings

Excessive cloning may indicate ownership design issues.

Consider borrowing patterns or restructuring data flow.

## The Test

After refactoring, the function's logic should be readable top-to-bottom without mental stack management.
