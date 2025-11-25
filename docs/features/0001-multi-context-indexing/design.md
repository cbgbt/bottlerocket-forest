# Multi-Context Indexing - Technical Design

## Overview

This design introduces context-aware indexing to sembly, enabling multiple working directories to share a single embedding database.
The key architectural change is separating content-addressed storage (embeddings, chunks) from per-context file mappings.

**Design Philosophy**: Contexts are isolated views into shared content.
Each context tracks which files it contains; the underlying embeddings are shared and deduplicated by content hash.

See [concept.md](./concept.md) for motivation and [requirements.md](./requirements.md) for detailed requirements.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      CLI / Facade                           │
│  (command dispatch, context resolution, progress reporting) │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Context Manager                         │
│  (context discovery, registration, workspace resolution)    │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────┐   ┌──────────┐
        │ Indexing │   │  Search  │   │    GC    │
        └──────────┘   └──────────┘   └──────────┘
              │               │               │
              └───────────────┼───────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Storage Layer                            │
│  ┌─────────────────┐  ┌──────────────────────────────────┐  │
│  │  Per-Context    │  │     Content-Addressed            │  │
│  │  - contexts     │  │     - chunks (by chunk_hash)     │  │
│  │  - indexed_files│  │     - vec_chunks (embeddings)    │  │
│  └─────────────────┘  └──────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

**Facade** - Entry point for all operations; resolves context from cwd or `--context` flag; delegates to appropriate subsystem.

**Context Manager** - Discovers workspace by walking up directory tree; manages context lifecycle (register, list, remove); validates context paths.

**Indexing** - Scans files within a context; computes content hashes; reuses existing embeddings when content matches; updates per-context file mappings.

**Search** - Queries embeddings scoped to a single context's files; applies boost rules; returns ranked results.

**GC** - Removes unreferenced chunks and embeddings; operates across all contexts.

**Storage** - Persists all data; separates per-context tables from content-addressed tables.

## Domain Model

### Core Types

**Context**

- Represents a working directory registered with sembly
- Properties: `context_id` (relative path), `created_at`, `last_indexed`
- The default context has `context_id = "."`
- Context IDs are canonicalized (no `./` prefix, normalized `.` and `..`)

**IndexedFile**

- Tracks a file's presence in a specific context
- Properties: `context_id`, `file_path`, `file_hash`, `mtime_ns`
- Primary key: `(context_id, file_path)`
- `file_hash` links to content-addressed chunks

**Chunk**

- A searchable unit of content derived from a file
- Properties: `chunk_hash`, `file_hash`, `content`, `context_type`, `context_data`, `token_count`
- Keyed by `chunk_hash` (content-addressed)
- Multiple chunks can share the same `file_hash` (one file → many chunks)

**Embedding**

- Vector representation of a chunk
- Properties: `chunk_hash`, `embedding`
- Keyed by `chunk_hash` (content-addressed)
- Stored in sqlite-vec virtual table

**Workspace**

- The root directory containing `.sembly/knowledge.db` and `.sembly.toml`
- All contexts within a workspace share configuration and embeddings
- Discovered by walking up from cwd

### Domain Operations

**discover_workspace(cwd) -> Result\<Workspace, WorkspaceNotFound>**

- Walks up directory tree looking for `.sembly/knowledge.db`
- Returns workspace root path or error with diagnostic (MCI-ERR-1)

**resolve_context(workspace, path) -> Result\<Context, ContextError>**

- Canonicalizes path relative to workspace root
- Validates context exists in database
- Returns error if unregistered (MCI-ERR-2)

**register_context(workspace, path) -> Result\<Context, ContextError>**

- Canonicalizes path, validates target directories exist (MCI-6)
- Inserts into contexts table
- Fails if already registered (MCI-3, MCI-4)

**index_context(context) -> Result<IndexStats>**

- Scans files in context's target directories
- For each file: compute `file_hash`, check for existing chunks
- Reuse embeddings when `chunk_hash` matches (MCI-11)
- Update `indexed_files` mapping (MCI-12)

**search(context, query) -> Result\<Vec<SearchResult>>**

- Generate query embedding
- Search vec_chunks filtered to context's file_hashes
- Apply boost rules, return ranked results (MCI-13, MCI-14)

**garbage_collect(workspace) -> Result<GcStats>**

- Find chunks not referenced by any context's indexed_files
- Delete orphaned chunks and embeddings (MCI-18)

## Boundaries & Adapters

### Trait: ContextRepository

**Purpose**: Abstract context persistence operations.

```rust
trait ContextRepository {
    fn list_contexts(&self) -> Result<Vec<Context>>;
    fn get_context(&self, context_id: &str) -> Result<Option<Context>>;
    fn insert_context(&self, context: &Context) -> Result<()>;
    fn remove_context(&self, context_id: &str) -> Result<()>;
    fn get_file_hashes(&self, context_id: &str) -> Result<HashSet<FileHash>>;
}
```

**Implementations**: `SqliteContextRepository` (production), in-memory mock (tests).

### Trait: ChunkRepository

**Purpose**: Abstract content-addressed chunk storage.

```rust
trait ChunkRepository {
    fn get_chunks_by_file_hash(&self, file_hash: &FileHash) -> Result<Vec<Chunk>>;
    fn insert_chunks(&self, chunks: &[Chunk]) -> Result<()>;
    fn delete_orphaned_chunks(&self) -> Result<u64>;
}
```

### Trait: EmbeddingRepository

**Purpose**: Abstract vector storage and search.

```rust
trait EmbeddingRepository {
    fn search(&self, embedding: &[f32], file_hashes: &HashSet<FileHash>, limit: usize) -> Result<Vec<ChunkMatch>>;
    fn insert_embedding(&self, chunk_hash: &ChunkHash, embedding: &[f32]) -> Result<()>;
    fn has_embedding(&self, chunk_hash: &ChunkHash) -> Result<bool>;
}
```

## Module Structure

```
sembly-core/src/knowledge/
├── mod.rs
├── constants.rs
├── domain/
│   ├── mod.rs
│   ├── context.rs          # NEW: Context, ContextId types
│   ├── chunk.rs
│   ├── file_type.rs
│   └── search.rs
├── context/                 # NEW: Context management
│   ├── mod.rs
│   ├── discovery.rs        # Workspace discovery (walk up tree)
│   ├── manager.rs          # Context lifecycle operations
│   └── resolver.rs         # Context resolution from cwd
├── storage/
│   ├── mod.rs
│   ├── repository.rs
│   ├── schema.rs           # MODIFIED: New schema with contexts
│   └── sqlite/
│       ├── mod.rs
│       ├── context.rs      # NEW: ContextRepository impl
│       ├── chunks.rs       # MODIFIED: Content-addressed queries
│       └── embeddings.rs   # MODIFIED: Context-filtered search
├── indexing/
│   ├── mod.rs
│   ├── scanner.rs          # MODIFIED: Context-aware scanning
│   ├── indexer/            # MODIFIED: Content-addressed storage
│   └── ...
├── search/
│   ├── mod.rs
│   ├── semantic.rs         # MODIFIED: Context-scoped queries
│   └── ...
└── facade/
    ├── mod.rs              # MODIFIED: Context resolution
    └── types.rs
```

## Migration from Current Design

### Current State

- Single implicit context (workspace root)
- `indexed_files` keyed by `(file_path)`
- Chunks keyed by `(file_path, chunk_index)`
- Embeddings stored alongside chunks
- No content-addressing

### Changes Required

**Types**

- `IndexedFile` gains `context_id` and `file_hash` fields
- `Chunk` changes key from `(file_path, chunk_index)` to `chunk_hash`
- New `Context` type for context registry

**Data Structures**

- Current: `indexed_files(file_path, mtime_ns)`

- New: `indexed_files(context_id, file_path, file_hash, mtime_ns)`

- Current: `chunks(file_path, chunk_index, content, ...)`

- New: `chunks(chunk_hash, file_hash, content, ...)`

**Processes**

- `index_file()` now computes `file_hash` first, checks for existing chunks
- `search()` now filters by context's file_hashes via join
- New `discover_workspace()` walks up directory tree

### Affected Modules

| Module                | Change Type | Summary                                      |
| --------------------- | ----------- | -------------------------------------------- |
| `storage/schema.rs`   | Major       | New schema with contexts, content-addressing |
| `storage/sqlite/`     | Major       | New context repository, modified queries     |
| `indexing/scanner.rs` | Moderate    | Context-aware file discovery                 |
| `indexing/indexer/`   | Moderate    | Content-addressed chunk storage              |
| `search/semantic.rs`  | Moderate    | Context-filtered vector search               |
| `facade/mod.rs`       | Moderate    | Context resolution, new commands             |
| `domain/`             | Minor       | New Context type                             |

## Design Patterns

**Content-Addressed Storage**

- Chunks and embeddings keyed by content hash
- Enables deduplication across contexts
- Natural cache behavior: same content = same embedding

**Repository Pattern**

- Abstract storage behind traits
- Enables testing with in-memory implementations
- Isolates sqlite-vec specifics from domain logic

**Workspace Discovery**

- Walk up directory tree (similar to git)
- Single source of truth for database location
- Enables nested worktree structures

## Implementation Guidance

1. **Start with schema changes** - Define new tables, implement migration detection (fail on old schema per MCI-NFR-3)

1. **Implement context discovery** - `discover_workspace()` is foundational; all commands need it

1. **Add context repository** - CRUD operations for contexts table

1. **Modify indexing for content-addressing** - Compute file_hash, check for existing chunks before embedding

1. **Update search for context filtering** - Join through indexed_files to filter by context

1. **Add CLI commands** - `context list`, `context remove`, `--context` flag

1. **Implement garbage collection** - Find and delete orphaned chunks/embeddings

**Key Constraints**

- All contexts share `.sembly.toml` (MCI-20)
- Target paths resolve relative to context root (MCI-21)
- No cross-context search (MCI-15)
- Embedding model is global; changing requires rebuild (MCI-NFR-4)

**Performance Considerations**

- First index of new context should be fast when content overlaps (MCI-NFR-1)
- Use batch inserts for indexed_files updates
- Consider transaction boundaries for large indexes

## Testing Strategy

**Unit Tests**

- Context ID canonicalization (various path formats)
- Workspace discovery (finds db, handles missing)
- Content hash computation consistency
- Context resolution logic

**Integration Tests**

- Full index → search cycle with multiple contexts
- Embedding reuse verification (mock embedding provider, count calls)
- Garbage collection removes only orphaned content
- Schema migration detection

**Edge Cases**

- Context at workspace root (`.`)
- Deeply nested context paths
- Context with no matching target directories
- Concurrent access (if applicable)

## Notes

- Schema version bump required; existing databases need `sembly rebuild`
- Git-aware optimization (tracking HEAD) is out of scope but design accommodates future addition
- Forester integration for worktree creation is a separate feature
