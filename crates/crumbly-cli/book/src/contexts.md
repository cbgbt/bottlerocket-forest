# Contexts

A context is a named scope for indexing and searching.
When you build an index, you specify which directory to index as a context.
When you search, results come from that context.

## Content-Addressable Storage

Crumbly uses content-addressable storage internally.
This means identical content is stored only once, regardless of how many contexts contain it.

This design shines when you work with git worktrees.
Many developers have found that AI agents work well with worktrees—each task gets its own working directory without branch switching overhead.
When you add a worktree as a new context, crumbly indexes it almost instantly because most of the content already exists in the index from other contexts.

Only the files that differ between worktrees need new embeddings computed.

## Creating a Context

Contexts are created implicitly when you build:

```bash
crumbly build --context ./my-project
```

The context name is derived from the path.
Running this command again updates the existing context, re-indexing only files that changed.

## Worktree Workflow

A typical workflow with worktrees:

```bash
# Main branch already indexed
crumbly build --context ./main

# Create a worktree for a feature
git worktree add ../feature-x -b feature-x

# Index it — fast, since most content is shared
crumbly build --context ../feature-x

# Search within your feature branch
crumbly search --context ../feature-x "authentication"
```

The second build completes quickly because crumbly recognizes that most chunks already exist.

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

## Listing Contexts

See what contexts exist:

```bash
crumbly status
```

This shows each context with its chunk count and last update time.
