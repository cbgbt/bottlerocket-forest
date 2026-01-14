# Contexts

A context is a named scope for indexing and searching.
When you build an index, you specify which directory to index as a context.
When you search, results come from that context.

## Index Location

Crumbly stores its index in a `.crumbly` directory.
For contexts to work properly, this index must be in a parent directory of all the contexts you want to index.

A typical setup:

```
my-workspace/
├── .crumbly/          # Index lives here
├── main/              # Main worktree (a context)
├── feature-a/         # Feature worktree (a context)
└── feature-b/         # Another worktree (a context)
```

All three worktrees share the same index because `.crumbly` is in their common parent.

## Content-Addressable Storage

Crumbly uses content-addressable storage internally.
Identical content is stored only once, regardless of how many contexts contain it.

This design shines when you work with git worktrees.
Many developers have found that AI agents work well with worktrees—each task gets its own working directory without branch switching overhead.
When you add a worktree as a new context, crumbly indexes it almost instantly because most of the content already exists in the index from other contexts.

Only the files that differ between worktrees need new embeddings computed.

## Creating a Context

Create a new context with `build`:

```bash
crumbly build --context ./main
```

The context name is derived from the path.

To update an existing context after files change, use `update`:

```bash
crumbly update --context ./main
```

This re-indexes only files that have changed since the last build.

## Worktree Workflow

A typical workflow with worktrees:

```bash
# From the workspace root (where .crumbly lives)
crumbly build --context ./main

# Create a worktree for a feature
cd main
git worktree add ../feature-x -b feature-x
cd ..

# Index it — fast, since most content is shared
crumbly build --context ./feature-x
```

The second build completes quickly because crumbly recognizes that most chunks already exist.

## Searching

Crumbly automatically discovers which context you're in.
When you run a search, it looks for a `.crumbly` index in parent directories, then determines if your current working directory falls within any indexed context.

```bash
cd feature-x
crumbly search "authentication"
```

This finds the index in `../`, recognizes you're in the `feature-x` context, and searches within it.

You can also specify a context explicitly:

```bash
crumbly search --context ./main "authentication"
```

## Listing Contexts

See what contexts exist:

```bash
crumbly status
```

This shows each context with its chunk count and last update time.
