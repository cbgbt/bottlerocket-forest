---
style-guide:
  description: "Rust Style Guide - Design Phase (Condensed)"
  prefer-in-contexts:
    - "Rust API design"
    - "TDD design phase"
---

# Rust Style Guide - Design Phase

Rules for designing APIs, types, and interfaces. Use during the design phase before tests or implementation.

## Project Crate Choices

These are non-negotiable project conventions:

| Purpose | Crate | Notes |
|---------|-------|-------|
| Error handling | `snafu` | Never `thiserror` |
| Builders | `bon` | With `#[builder(on(_, into))]` |
| Validated newtypes | `nutype` | For simple validation |
| Measurements | `uom` | For physical quantities |
| Logging | `tracing` | With `#[instrument]` |
| Config format | `toml` | Prefer over JSON/YAML |

## Error Design with Snafu

### Error-Per-Operation Pattern

Each fallible operation gets its own error type, placed below the function:

```rust
pub fn parse_config(path: impl AsRef<Path>) -> Result<Config, ParseConfigError> {
    todo!()
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum ParseConfigError {
    #[snafu(display("Failed to read config file"))]
    ReadFile { source: std::io::Error },
    
    #[snafu(display("Invalid TOML syntax"))]
    InvalidToml { source: toml::de::Error },
}
```

### Key Snafu Rules

- Use `#[snafu(module)]` to generate context selectors in a submodule
- Never include `{source}` in display messages
- Display messages describe what failed, not how to fix it (remediation belongs in miette diagnostics)
- Use `snafu::ensure!` for validation, not `if/return`
- Hierarchical errors: low-level errors wrap into higher-level ones
- Import context selectors inside functions, not at module level:

```rust
pub fn do_thing() -> Result<(), DoThingError> {
    use do_thing_error::*;  // Import selectors here, not at module top
    // ...
}
```

This pattern is ONLY for snafu context selectors. All other imports belong at module level.

### Traits with Associated Errors

Traits that can fail use associated error types:

```rust
pub trait Repository {
    type Error: std::error::Error + Send + Sync + 'static;
    
    async fn get(&self, id: &str) -> Result<Option<Item>, Self::Error>;
}
```

Each implementation defines its own error type. Never use a shared concrete error across implementations.

## Type Design

### Newtypes with Validation

Use `nutype` for simple validation (single predicate):

```rust
#[nutype(validate(not_empty), derive(Debug, Clone, Serialize, Deserialize))]
pub struct Username(String);
```

Use `nutype` with custom validation for transformations:

```rust
#[nutype(
    derive(Debug, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize),
    validate(with = validate_context_id, error = ContextIdError),
    new_unchecked
)]
pub struct ContextId(String);

fn validate_context_id(raw: &str) -> Result<(), ContextIdError> {
    // validation logic
}
```

Manual implementation with snafu only when nutype cannot express the pattern (rare).

### Path Handling

For path normalization, use `path-clean` crate:

```rust
use path_clean::PathClean;

// Normalize without touching filesystem (resolves . and ..)
let normalized = path.clean();
```

Never reimplement path canonicalization manually.

### Builders with Bon

Composite structs (2+ fields) MUST use builders, not constructors:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct SearchQuery {
    text: String,
    limit: Option<usize>,
}
```

Rules:
- `#[non_exhaustive]` is REQUIRED - prevents direct struct construction
- `on(_, into)` enables `.field("value")` without `.to_string()`
- Only newtypes (single-field tuple structs) get `new()` constructors
- Use `#[builder(default = Utc::now())]` for timestamp fields that should default to now
- `Option<T>` fields automatically default to `None` in bon builders - DO NOT add `#[builder(default)]` to Option fields

Example with timestamps:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct Context {
    context_id: ContextId,
    #[builder(default = Utc::now())]
    created_at: DateTime<Utc>,
    last_indexed: Option<DateTime<Utc>>,  // No annotation needed - Option defaults to None
}

// Usage in tests - clean and minimal
let ctx = Context::builder().context_id(id).build();
```

### Measurements with uom

Physical quantities use `uom`, never raw numbers:

```rust
use uom::si::f64::{Time, Information};
use uom::si::time::second;
use uom::si::information::byte;

fn estimate_duration(size: Information, rate: Information) -> Time {
    todo!()
}
```

### Type-Safe API Design

Prefer methods on domain types over free functions with primitive parameters:

```rust
// Good - type-safe, discoverable, can support streaming
impl FileHash {
    pub fn from_reader(reader: impl Read) -> io::Result<Self> { todo!() }
}

// Bad - primitives lose type safety, require loading all into memory
pub fn compute_hash(content: &[u8]) -> Hash { todo!() }
```

When designing APIs:
- Accept domain types, not primitives (`&Chunk` not `&str`)
- Support streaming (`impl Read`) to avoid loading large data into memory
- Use the type system to prevent mixing up similar-looking values

## Documentation Rules

- Doc comments on all public items (summary line < 120 chars)
- No `# Errors` sections documenting when functions fail
- No parameter/return documentation in docstrings
- Avoid vague terms: "robust", "efficient", "handles", "manages"

## Serialization

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    api_endpoint: Url,
    max_retries: u32,
}
```

## Module Organization

- Domain-specific modules, not `types.rs` grab-bags
- Directory modules (`foo/mod.rs`) when > 500 LOC
- Error types live near their operations
- Submodules should be explainable in the parent's docstring

## Derives

Standard set for most types:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainType { /* ... */ }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleEnum { A, B, C }
```

Derive `PartialEq`/`Eq` for types that will be tested (enables struct comparison in assertions).
