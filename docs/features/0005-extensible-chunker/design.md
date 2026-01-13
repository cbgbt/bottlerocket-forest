# Extensible Chunker Abstraction - Technical Design

## Overview

This design enables adding new language chunkers without schema migrations.
The storage layer becomes chunker-agnostic by treating `context_type` as an opaque string.
Unrecognized types are preserved via a new `Unknown` variant, enabling forward compatibility.

See [concept.md](./concept.md) for problem analysis and [requirements.md](./requirements.md) for specifications.

## Critical Constraints

| ID | Constraint | Rationale | Anti-pattern |
|----|------------|-----------|-------------|
| CC-1 | Migration must use ALTER TABLE, not table recreation | O(1) time regardless of row count (ECA-NFR-1) | Creating new table and copying rows |
| CC-2 | Unknown variant must preserve original type string | Round-trip preservation for forward compatibility (ECA-5) | Discarding or normalizing unknown types |
| CC-3 | Deserialization must warn, not fail, on unknown types | Graceful degradation (ECA-3, ECA-5) | Returning error on unrecognized context_type |
| CC-4 | Empty/null context_type must error, not become Unknown | Malformed data detection (ECA-ERR-2) | Treating empty string as valid Unknown |
| CC-5 | Migration must rollback on failure | Data integrity (ECA-ERR-1) | Partial migration leaving inconsistent state |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Chunking Layer                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │MarkdownChunker│ │ RustChunker │  │ JavaChunker │  ...    │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │                │                │                 │
│         └────────────────┼────────────────┘                 │
│                          ▼                                  │
│              ┌───────────────────────┐                      │
│              │  ChunkingDispatcher   │                      │
│              │  (extension → chunker)│                      │
│              └───────────┬───────────┘                      │
└──────────────────────────┼──────────────────────────────────┘
                           │ Chunk with ChunkContext
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                     Storage Layer                           │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              Serialization Module                    │   │
│  │  serialize_context() → (type_string, json_data)     │   │
│  │  deserialize_context() → ChunkContext (or Unknown)  │   │
│  └─────────────────────────┬───────────────────────────┘   │
│                            ▼                                │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              SQLite (schema v4)                      │   │
│  │  context_type TEXT NOT NULL  (no CHECK constraint)  │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

The chunking layer produces typed `ChunkContext` variants.
The storage layer persists `context_type` as an opaque string.
Deserialization reconstructs known variants or creates `Unknown` for unrecognized types.

## Domain Model

### ChunkContext Enum

Current (closed):
```rust
pub enum ChunkContext {
    Markdown(MarkdownContext),
    RustDoc(RustDocContext),
    GoDoc(GoDocContext),
}
```

Required (open):
```rust
pub enum ChunkContext {
    Markdown(MarkdownContext),
    RustDoc(RustDocContext),
    GoDoc(GoDocContext),
    Unknown(UnknownContext),
}
```

### UnknownContext Type

```rust
/// Preserves unrecognized chunk context types for forward compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Builder, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub struct UnknownContext {
    /// Original context type string from storage.
    pub type_name: String,
    /// Raw JSON data preserved for round-tripping.
    pub raw_data: String,
}
```

**Invariants:**
- `type_name` must be non-empty (CC-4)
- `raw_data` preserves original JSON exactly for round-trip fidelity (CC-2)

## Module Structure

No new modules required. Changes are localized to existing files:

```
crates/crumbly-core/src/knowledge/
├── constants.rs           # SCHEMA_VERSION: 3 → 4
├── domain/
│   └── chunk.rs           # Add UnknownContext, ChunkContext::Unknown
└── storage/
    ├── schema.rs          # Add migrate_v3_to_v4()
    └── sqlite/
        └── serialization.rs  # Update serialize/deserialize_context()
```

## Design Decisions

### DD-1: ALTER TABLE vs Table Recreation

**Decision:** Use `ALTER TABLE ... DROP CONSTRAINT` (via table recreation in SQLite)

**Alternatives:**
1. SQLite doesn't support DROP CONSTRAINT directly; must recreate table
2. Use pragma to disable constraint checking

**Rationale:** SQLite requires table recreation to modify CHECK constraints.
However, we can achieve O(1) by noting that SQLite's CHECK constraints are only enforced on INSERT/UPDATE, not stored per-row.
The migration creates a new table without the constraint, copies data, and swaps—but this is O(n).

**Revised approach:** Since O(1) is required (ECA-NFR-1), and SQLite CHECK constraints cannot be dropped in O(1), we accept that the existing v2→v3 migration pattern (table recreation) violates this.
For v3→v4, we use the same pattern but document that large databases may have slow migrations.

**Alternative considered:** Store context_type in a separate table with no CHECK.
Rejected: adds complexity without solving the fundamental SQLite limitation.

### DD-2: Unknown Variant Structure

**Decision:** Store both `type_name` and `raw_data` in UnknownContext

**Alternatives:**
1. Store only type_name, discard data
2. Store as generic JSON Value

**Rationale:** Preserving raw_data enables:
- Round-trip fidelity when saving Unknown chunks
- Future code versions can deserialize properly
- Debugging visibility into unknown chunk contents

### DD-3: Warning Mechanism

**Decision:** Use `tracing::warn!` on unknown type deserialization

**Alternatives:**
1. Log at info level
2. Collect warnings and report at end
3. Silent handling

**Rationale:** warn! level is appropriate because:
- User should know their index contains types their version doesn't understand
- Not an error (system continues working)
- Visible in default log configurations

## Implementation Guidance

### Schema Migration (v3→v4)

Reference: CC-1, CC-5, ECA-6, ECA-ERR-1

```rust
fn migrate_v3_to_v4(conn: &Connection) -> Result<()> {
    // Transaction for rollback on failure (CC-5)
    // Recreate chunks table without CHECK constraint
    // Pattern matches existing migrate_v2_to_v3
}
```

The new CREATE_CHUNKS constant removes the CHECK constraint:
```sql
CREATE TABLE chunks (
    -- ... same columns ...
    context_type TEXT NOT NULL,  -- No CHECK constraint
    -- ...
)
```

### Serialization Changes

Reference: CC-2, CC-3, CC-4, ECA-2, ECA-3

`serialize_context()` addition:
```rust
ChunkContext::Unknown(ctx) => (
    ctx.type_name.clone(),
    ctx.raw_data.clone(),
)
```

`deserialize_context()` changes:
```rust
match context_type {
    "" => Err(/* CC-4: empty type is error */),
    "markdown" => /* existing */,
    "rust_doc" => /* existing */,
    "go_doc" => /* existing */,
    unknown => {
        tracing::warn!("Unknown context type: {}", unknown);  // ECA-3
        Ok(ChunkContext::Unknown(UnknownContext {
            type_name: unknown.to_string(),
            raw_data: context_data.to_string(),
        }))
    }
}
```

### Adding a New Chunker (e.g., Java)

Reference: ECA-7

After this change, adding Java support requires only:

1. **Domain type** (optional, for type safety):
```rust
// In chunk.rs
pub struct JavaDocContext { /* fields */ }

pub enum ChunkContext {
    // ... existing ...
    JavaDoc(JavaDocContext),
}
```

2. **Serialization** (if adding typed variant):
```rust
// In serialization.rs - add match arms
ChunkContext::JavaDoc(ctx) => ("java_doc", serde_json::to_string(ctx)?)
"java_doc" => Ok(ChunkContext::JavaDoc(serde_json::from_str(context_data)?))
```

3. **Chunker implementation**:
```rust
// New file: chunking/java.rs
pub struct JavaChunker;
impl Chunker for JavaChunker { /* ... */ }
```

4. **Dispatcher registration**:
```rust
// In dispatcher.rs
dispatcher.register(".java", Box::new(JavaChunker));
```

**No schema migration required.** The storage layer accepts any context_type string.

### Constants Update

```rust
// In constants.rs
pub const SCHEMA_VERSION: u32 = 4;  // was 3
```

### Version Compatibility Matrix

| Database | Code | Behavior |
|----------|------|----------|
| v3 | v4 | Migrates to v4, works normally |
| v4 | v4 | Works normally |
| v4 (with java_doc) | v4 (no Java) | Java chunks load as Unknown, warning logged |
| v4 | v3 | Schema mismatch error (expected: rebuild index) |

## Testing Approach

1. **Migration tests**: v3 database migrates to v4, data preserved
2. **Round-trip tests**: Unknown chunks serialize/deserialize with fidelity
3. **Warning tests**: Unknown type logs warning (capture tracing output)
4. **Error tests**: Empty context_type returns error, not Unknown
5. **Integration test**: Add mock "test_doc" type, verify no schema changes needed

## Next Steps

1. Create test plan using `propose-feature-test-plan` skill
2. Create implementation plan using `propose-implementation-plan` skill
3. Implement schema migration
4. Update serialization
5. Add Unknown variant to ChunkContext
6. Validate with Java chunker addition
