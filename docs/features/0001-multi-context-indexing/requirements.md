# Multi-Context Indexing - Requirements Specification

## Overview

This specification defines requirements for sembly's multi-context indexing feature.
Contexts allow multiple working directories to share a single embedding database, enabling efficient parallel development workflows.
See [concept.md](./concept.md) for background and motivation.

## Functional Requirements

### Context Discovery

#### MCI-1: Database Discovery via Directory Walk

**WHEN** sembly executes any command  
**THEN** the system **SHALL** walk up the directory tree from cwd to find `.sembly/knowledge.db`

**WHERE** no database is found  
**THEN** the system **SHALL** fail with a diagnostic suggesting the user create an index

#### MCI-2: Context Detection from Current Directory

**WHEN** sembly executes a command without `--context`  
**THEN** the system **SHALL** use cwd as the context

**WHERE** cwd is not inside a registered context  
**THEN** the system **SHALL** fail with a diagnostic suggesting available contexts or how to register one

### Context Registration

#### MCI-3: Explicit Context Registration

**WHEN** the user runs `sembly build --context <path>`  
**THEN** the system **SHALL** register the path as a context and index its files

**WHERE** the context is already registered  
**THEN** the system **SHALL** fail with a diagnostic suggesting `sembly update` or `sembly clear`

#### MCI-4: Default Context Creation

**WHEN** the user runs `sembly build` without `--context`  
**THEN** the system **SHALL** create a context with ID `.` (the workspace root) and index its files

**WHERE** the default context is already registered  
**THEN** the system **SHALL** fail with a diagnostic suggesting `sembly update` or `sembly clear`

#### MCI-5: Context ID Canonicalization

**WHEN** registering a context  
**THEN** the system **SHALL** store the context ID as a relative path from the workspace root, with `.` and `..` normalized and leading `./` stripped

Example:
```
./worktrees/feature-a → worktrees/feature-a
```

#### MCI-6: Target Directory Validation

**WHEN** registering a context  
**WHERE** none of the configured target directories exist in the context  
**THEN** the system **SHALL** emit a warning diagnostic

### Context Management

#### MCI-7: List Contexts

**WHEN** the user runs `sembly context list`  
**THEN** the system **SHALL** display all registered contexts with the default context (`.`) marked

#### MCI-8: Remove Context

**WHEN** the user runs `sembly context remove <context-id>`  
**THEN** the system **SHALL** remove the context's file mappings from the database

**WHERE** the context does not exist  
**THEN** the system **SHALL** fail with a diagnostic

#### MCI-9: Clear Context

**WHEN** the user runs `sembly clear`  
**THEN** the system **SHALL** remove the current context's file mappings from the database, leaving the context unindexed but registered

**WHEN** the user runs `sembly clear --context <path>`  
**THEN** the system **SHALL** remove the specified context's file mappings

### Content-Addressed Storage

#### MCI-10: File Hash Computation

**WHEN** indexing a file  
**THEN** the system **SHALL** compute a content hash of the file

#### MCI-11: Embedding Reuse

**WHEN** indexing a file  
**WHERE** a chunk with the same content hash already exists  
**THEN** the system **SHALL** reuse the existing embedding without regeneration

#### MCI-12: Per-Context File Mapping

**WHEN** indexing a file in a context  
**THEN** the system **SHALL** record the mapping of (context_id, file_path) → file_hash

### Search

#### MCI-13: Context-Scoped Search

**WHEN** the user runs `sembly search <query>`  
**THEN** the system **SHALL** return results only from files in the current context

#### MCI-14: Explicit Context Search

**WHEN** the user runs `sembly search --context <path> <query>`  
**THEN** the system **SHALL** return results only from files in the specified context

#### MCI-15: No Cross-Context Search

**THEN** the system **SHALL NOT** provide a mechanism to search across multiple contexts simultaneously

### Incremental Updates

#### MCI-16: Context-Scoped Update

**WHEN** the user runs `sembly update`  
**THEN** the system **SHALL** update only the current context's file mappings

#### MCI-17: Change Detection

**WHEN** updating a context  
**THEN** the system **SHALL** detect changed files using mtime comparison

### Garbage Collection

#### MCI-18: Explicit Garbage Collection

**WHEN** the user runs `sembly gc`  
**THEN** the system **SHALL** remove chunks and embeddings not referenced by any context

#### MCI-19: No Automatic Garbage Collection

**THEN** the system **SHALL NOT** automatically remove unreferenced embeddings during normal operations

### Configuration

#### MCI-20: Shared Configuration

**THEN** the system **SHALL** use a single `.sembly.toml` at the workspace root for all contexts

#### MCI-21: Relative Target Paths

**WHEN** interpreting target directories from `.sembly.toml`  
**THEN** the system **SHALL** resolve them relative to the context root, not the workspace root

## Non-Functional Requirements

### MCI-NFR-1: New Context Indexing Performance

**WHILE** indexing a new context where most files are unchanged from an existing context  
**THEN** the system **SHALL** complete indexing significantly faster than a full rebuild (targeting seconds rather than minutes)

### MCI-NFR-2: Storage Efficiency

**WHILE** multiple contexts share identical file content  
**THEN** the system **SHALL** store only one copy of each unique embedding

### MCI-NFR-3: Backward Compatibility

**WHILE** opening a database created before multi-context support  
**THEN** the system **SHALL** fail with a diagnostic instructing the user to run `sembly rebuild`

### MCI-NFR-4: Embedding Model Consistency

**THEN** the system **SHALL** require all contexts in a workspace to use the same embedding model

## Error Handling

### MCI-ERR-1: Missing Database

**WHEN** executing a command  
**WHERE** no `.sembly/knowledge.db` is found in the directory tree  
**THEN** the system **SHALL** fail with a miette diagnostic suggesting how to create an index

### MCI-ERR-2: Unregistered Context

**WHEN** executing a search or update  
**WHERE** cwd is not inside a registered context  
**THEN** the system **SHALL** fail with a miette diagnostic listing available contexts

### MCI-ERR-3: Context Removal of Default

**WHEN** the user runs `sembly context remove .`  
**THEN** the system **SHALL** fail with a diagnostic explaining the default context cannot be removed

## Appendix A: Database Schema

```sql
-- Context registry
contexts (
    context_id TEXT PRIMARY KEY,    -- relative path, e.g., "worktrees/feature-a" or "."
    created_at INTEGER NOT NULL,
    last_indexed INTEGER
)

-- Per-context: which files exist
indexed_files (
    context_id TEXT NOT NULL,
    file_path TEXT NOT NULL,        -- relative path within context
    file_hash BLOB NOT NULL,        -- hash of entire file content
    mtime_ns INTEGER NOT NULL,
    PRIMARY KEY (context_id, file_path)
)

-- Content-addressed: chunks derived from files
chunks (
    chunk_hash BLOB PRIMARY KEY,    -- hash of chunk content
    file_hash BLOB NOT NULL,        -- which file produced this chunk
    content TEXT NOT NULL,          -- chunk text
    context_type TEXT NOT NULL,     -- 'markdown', 'rust_doc'
    context_data TEXT NOT NULL,     -- chunk metadata (heading, position, etc.)
    token_count INTEGER NOT NULL
)

-- Content-addressed: embeddings
vec_chunks (
    chunk_hash BLOB PRIMARY KEY,
    embedding FLOAT[N]
)
```

## Appendix B: Context List Output Format

```
Contexts:
  .                      (default)
  worktrees/feature-a
  worktrees/feature-b
```

## Notes

- Prefix: MCI (Multi-Context Indexing)
- Git-aware optimization (tracking HEAD commit) is out of scope for initial implementation but the design should accommodate it
- Schema migrations module (`storage/migrations/`) is a separate concern; this feature requires `sembly rebuild` for existing databases
