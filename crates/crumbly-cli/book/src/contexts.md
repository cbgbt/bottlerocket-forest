# Contexts

A context is a named scope for indexing and searching.
When you build an index, you specify which directory to index as a context.
When you search, results come from that context.

## Why Contexts?

You might work on multiple projects, or want separate indexes for different parts of a large monorepo.
Contexts let you keep these separate without maintaining multiple databases.

Each context tracks:

- Which files have been indexed
- The chunks extracted from those files
- File modification times (for incremental updates)

## Creating a Context

Contexts are created implicitly when you build:

```bash
crumbly build --context ./my-project
```

The context name is derived from the path.
Running this command again updates the existing context, re-indexing only files that changed.

## Searching a Context

By default, search uses the context from your current directory:

```bash
cd my-project
crumbly search "authentication"
```

Or specify explicitly:

```bash
crumbly search --context ./my-project "authentication"
```

## Multiple Contexts

You can have as many contexts as you need:

```bash
crumbly build --context ./project-a
crumbly build --context ./project-b
crumbly build --context ./monorepo/services
```

Each maintains its own set of indexed files.
The underlying storage is shared—if two contexts contain identical content, the embeddings are stored only once.

## Listing Contexts

See what contexts exist:

```bash
crumbly status
```

This shows each context with its chunk count and last update time.
