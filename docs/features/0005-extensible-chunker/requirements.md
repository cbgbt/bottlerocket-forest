# Extensible Chunker Abstraction - Requirements Specification

## Overview

This specification defines requirements for making crumbly's storage layer chunker-agnostic.
Currently, adding a new language chunker requires schema migrations and hardcoded changes across multiple files.
These requirements enable new chunkers to be added with only chunker implementation and dispatcher registration.

See [concept.md](./concept.md) for problem analysis and solution direction.

## Functional Requirements

### ECA-1: Schema Extensibility

**WHEN** a new chunker type is registered  
**THEN** the system **SHALL** store chunks without requiring a database schema migration

### ECA-2: Unknown Context Type Preservation

**WHILE** deserializing chunk data from storage  
**WHERE** the context_type value is not recognized by the current code  
**THEN** the system **SHALL** preserve the chunk with a `ChunkContext::Unknown` variant containing the original type string

### ECA-3: Unknown Context Type Warning

**WHILE** deserializing chunk data from storage  
**WHERE** the context_type value is not recognized by the current code  
**THEN** the system **SHALL** emit a warning log message indicating the unrecognized type

### ECA-4: Backward Compatibility

**WHILE** operating with an existing database  
**WHERE** the database contains chunks created before the schema migration  
**THEN** the system **SHALL** deserialize all existing chunks without error

### ECA-5: Forward Compatibility

**WHILE** operating with a database  
**WHERE** the database contains chunks with context types added by newer code versions  
**THEN** the system **SHALL** load those chunks as `Unknown` variants without failing

### ECA-6: Schema Migration

**WHEN** the application starts with a v3 schema database  
**THEN** the system **SHALL** migrate to v4 by removing the CHECK constraint on context_type

### ECA-7: New Chunker Registration

**WHEN** adding a new language chunker (e.g., Java)  
**THEN** the system **SHALL** require only:
1. Chunker implementation (implements chunking trait)
2. Dispatcher registration (add to defaults)

**WHERE** no schema migration is required  
**AND** no serialization code changes are required

## Non-Functional Requirements

### ECA-NFR-1: Migration Performance

**WHILE** migrating from v3 to v4 schema  
**THEN** the system **SHALL** complete migration in O(1) time regardless of row count

### ECA-NFR-2: Deserialization Performance

**WHILE** deserializing chunks  
**THEN** the system **SHALL** maintain O(1) lookup for context type mapping

## Error Handling

### ECA-ERR-1: Migration Failure Recovery

**WHILE** performing schema migration  
**WHERE** the migration fails  
**THEN** the system **SHALL** roll back to the previous schema version

### ECA-ERR-2: Malformed Context Data

**WHILE** deserializing chunk context  
**WHERE** the context_type is empty or null  
**THEN** the system **SHALL** return an error (not Unknown)

## Appendix A: ChunkContext Enum Changes

Current enum (closed):
```rust
pub enum ChunkContext {
    Markdown { heading_path: Vec<String> },
    RustDoc { item_path: String, item_kind: String },
    GoDoc { item_path: String, item_kind: String },
}
```

Required enum (open):
```rust
pub enum ChunkContext {
    Markdown { heading_path: Vec<String> },
    RustDoc { item_path: String, item_kind: String },
    GoDoc { item_path: String, item_kind: String },
    Unknown { type_name: String, raw_data: String },
}
```

## Appendix B: Schema Migration v3→v4

Current CHECK constraint to remove:
```sql
CHECK(context_type IN ('markdown', 'rust_doc', 'go_doc'))
```

Migration removes constraint, allowing any string value for context_type.

## Appendix C: Validation - Java Chunker

To validate the abstraction works, a Java chunker will be added requiring only:

1. `JavaChunker` struct implementing the chunking trait
2. Registration in `ChunkingDispatcher::with_defaults_and_filter`
3. New `ChunkContext::JavaDoc` variant

No schema migration or serialization changes should be needed.
