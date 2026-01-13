# Storage

Crumbly stores all indexed content locally in a SQLite database.
No cloud services, no external dependencies—your data stays on your machine.

## The .crumbly Directory

When you run `crumbly build`, crumbly creates a `.crumbly` directory to store the index:

```
your-project/
├── .crumbly/
│   └── index.db    # SQLite database with all indexed content
├── src/
├── docs/
└── crumbly.toml
```

By default, crumbly creates `.crumbly` in the current directory.
You can specify a different location with the `--context` flag:

```bash
# Index ./my-project, store the database in ./my-project/.crumbly
crumbly build --context ./my-project

# Search uses the same context
crumbly search --context ./my-project "authentication"
```

Add `.crumbly/` to your `.gitignore`—the index is derived from your source files and doesn't need to be committed.

## Content Addressing

Crumbly uses content-addressing to avoid redundant work.
When you index a file, crumbly computes a hash of each chunk's content.
If identical content already exists in the database, crumbly reuses the existing embedding.

This means:

- Re-indexing unchanged files is fast (no re-embedding needed)
- Duplicate content across files shares storage
- Only modified chunks get re-processed on rebuild

## Contexts

A "context" is simply a directory containing a `.crumbly` folder.
You can maintain separate indexes for different projects:

```bash
# Index your main project
crumbly build --context ~/projects/backend

# Index a different project
crumbly build --context ~/projects/frontend

# Search each independently
crumbly search --context ~/projects/backend "database migrations"
crumbly search --context ~/projects/frontend "component lifecycle"
```

Each context has its own independent database.
There's no cross-contamination between projects.

## Cleanup with gc

Over time, as files change or get deleted, the database may contain orphaned entries.
The `gc` command cleans these up:

```bash
crumbly gc
```

This removes:

- Chunks from files that no longer exist
- Embeddings that are no longer referenced
- Stale metadata

Run `gc` periodically if you're working on a project with frequent file churn, or after major refactors.

## Checking Index Status

To see what's in your index:

```bash
crumbly status
```

This shows the number of indexed files, chunks, and other statistics about your context.
