# Content Sources

Before crumbly can search your documentation, it needs to know where to find it.
This chapter explains how crumbly discovers files.

## Filesystem Source

The filesystem source is what you'll use for day-to-day work.
It scans directories on disk, following the familiar patterns of a normal project checkout.

```bash
# Index the current directory
crumbly build --context .

# Index a specific project
crumbly build --context ./my-project
```

Crumbly walks the directory tree, finds documentation files, and indexes them.
This creates a searchable context.

## Configuring Targets

By default, crumbly indexes everything in your context directory.
Use the `targets` option to focus on specific subdirectories:

```toml
# crumbly.toml
targets = ["docs", "src"]
```

This tells crumbly to only look in `docs/` and `src/`, ignoring everything else.

### Example: Monorepo with Multiple Projects

```toml
targets = [
    "docs",
    "packages/core/src",
    "packages/cli/src",
]
```

### Example: Focus on Documentation Only

```toml
targets = ["docs"]
```

## Ignoring Files

Crumbly respects your existing ignore patterns so you don't index generated files, dependencies, or build artifacts.

### Gitignore Support

By default, crumbly honors `.gitignore` files.
This means `node_modules/`, `target/`, and other ignored directories stay out of your index.

```toml
# crumbly.toml
respect-gitignore = true  # This is the default
```

To index everything regardless of gitignore:

```toml
respect-gitignore = false
```

### Crumblyignore Support

Need to exclude files from search that you don't want in gitignore?
Create a `.crumblyignore` file with the same syntax:

```
# .crumblyignore

# Exclude changelogs from search results
CHANGELOG.md
**/CHANGELOG.md

# Exclude auto-generated API docs
docs/api/generated/
```

Enable it in your config:

```toml
use-crumblyignore = true  # This is the default
```

## Pre-warming with Bare Repositories

This is an advanced feature you can skip on first read.

Crumbly can read files directly from bare git repositories (repos without a working directory).
However, bare repositories can only be used to *cache* embeddings—they don't create searchable contexts.

### Why Cache-Only?

A bare repository has no working directory:

```
my-project.git/
├── objects/
├── refs/
├── HEAD
└── config
```

There's no file tree to search against.
But crumbly can still extract content and compute embeddings, storing them in the content-addressable storage.

### The Use Case: Pre-warming

When you later check out that repository as a normal worktree and build a context, crumbly finds that most embeddings already exist.
The build completes almost instantly.

This is useful for:

- Git servers that want to pre-compute embeddings
- CI pipelines that prepare indexes before developers clone
- Reducing first-build time for large repositories

### How to Pre-warm

```bash
# Cache embeddings from a bare repo (not searchable)
crumbly cache /srv/git/my-project.git

# Later, when someone checks it out:
git clone /srv/git/my-project.git
cd my-project
crumbly build --context .  # Fast—embeddings already cached
```

For most users, you can ignore bare repository support entirely.
Just use filesystem sources with normal checkouts.
