---
style-guide:
  description: "Rust Style Guide - Implementation Phase (Condensed)"
  prefer-in-contexts:
    - "Rust implementation"
    - "TDD green phase"
---

# Rust Style Guide - Implementation Phase

Rules for implementing code. Use during the "green" phase of TDD.

## Before You Start

1. Check `Cargo.toml` for available dependencies
2. Look up docs for key crates: `snafu`, `bon`, `nutype`

## Quality Gates

Before marking work complete:

```bash
cargo fmt
cargo clippy
cargo test
```

## Snafu Usage

### Context Selectors

Import snafu context selectors locally within functions, not at module level.
This is the ONLY case where function-level imports are appropriate:

```rust
fn load_config(path: &Path) -> Result<Config, LoadConfigError> {
    use load_config_error::*;  // ONLY snafu selectors go here
    
    let content = std::fs::read_to_string(path)
        .context(ReadFileSnafu { path })?;
    
    toml::from_str(&content).context(ParseTomlSnafu)
}
```

All other imports (crates, types, traits) belong at module level, never inside functions.

### Use ensure! for Validation

```rust
pub fn new(start: u32, end: u32) -> Result<Self, SpanError> {
    use span_error::*;
    
    snafu::ensure!(start <= end, InvalidRangeSnafu { start, end });
    Ok(Self { start, end })
}
```

Never use `if condition { return Err(...) }` pattern.

### Boxed Errors for Trait Objects

When wrapping errors from trait implementations:

```rust
#[derive(Debug, Snafu)]
pub enum ProcessError {
    #[snafu(display("Parse failed"))]
    Parse { source: Box<dyn std::error::Error + Send + Sync + 'static> },
}

// Usage
parse(content)
    .map_err(|e| Box::new(e) as _)
    .context(ParseSnafu)?;
```

## Import Organization

- All imports at top, no blank lines between them
- `pub use` separated by one blank line
- Group items from same module: `use serde::{Deserialize, Serialize};`
- Don't group across modules: not `use std::{path::Path, io::Error};`
- Crate-local types: always import, use unqualified
- Ambiguous third-party types: use qualified (`serde_json::Value`, `sqlx::Error`)

## Result/Option Handling

Prefer combinators:

```rust
// Good
data.map(|s| s.len())
result.and_then(|v| process(v))

// Use if-let for early returns with side effects
if let Some(value) = optional {
    return Ok(value);
}
```

Use `.context()` with `?` operator:

```rust
let content = std::fs::read_to_string(path)
    .context(ReadFileSnafu { path })?;
```

## No Panics in Production

Never use `unwrap()` or `expect()` in production code. Return errors instead.

## Builder Usage

Don't call `.to_string()` when using `bon` with `on(_, into)`:

```rust
// Good - bon converts automatically
MyType::builder().name("value").build()

// Bad - unnecessary conversion
MyType::builder().name("value".to_string()).build()
```

## Logging with Tracing

```rust
use tracing::{info, instrument};

#[instrument]
fn process(input: &str) -> Result<Output, Error> {
    info!(length = input.len(), "Processing input");
    // ...
}

#[instrument(err)]
fn fallible() -> Result<(), Error> {
    // Errors automatically logged
}
```

## Configuration

- TOML format preferred
- Pass config via CLI args, not env vars
- Use typed config structs with domain types:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub endpoint: Url,  // Not String
    pub timeout: Duration,
}
```

## Cargo.toml Dependencies

Version specification:
- Pre-1.0: `crate = "0.8"` (major.minor)
- Post-1.0: `crate = "1"` (major only)

Alphabetize dependencies. Use workspace dependencies when available:

```toml
[dependencies]
anyhow.workspace = true
serde = { workspace = true, features = ["derive"] }
tokio.workspace = true
```

## Code Comments

Only add comments that explain *why*, not *what*:

```rust
// Good - explains why
// Retry needed because S3 eventual consistency can cause brief 404s
let result = retry(|| fetch_object()).await?;

// Bad - restates code
// Fetch the object from S3
let result = fetch_object().await?;
```

## Unsafe Code

Avoid. If unavoidable, include `SAFETY:` comment:

```rust
// SAFETY: pointer is aligned and points to initialized memory
// because we just allocated it above
unsafe { *ptr = value; }
```
