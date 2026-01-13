# Content Sources

Before crumbly can search your documentation, it needs to know where to find it.
This chapter explains how crumbly discovers files and the options you have for configuring that discovery.

## How Content Discovery Works

When you run `crumbly build`, crumbly walks through your configured directories looking for files to index.
It respects your gitignore rules by default, so generated files and dependencies stay out of your search results.

```
┌─────────────────────────────────────────────────┐
│                 Content Sources                 │
├─────────────────────────────────────────────────┤
│                                                 │
│   Filesystem Source        Bare Git Source      │
│   (default)                (for servers)        │
│                                                 │
│   ./docs/                  repo.git/            │
│   ./src/                     └─ objects/        │
│   ./README.md                └─ refs/           │
│                                                 │
└─────────────────────────────────────────────────┘
                      │
                      ▼
              Files to Index
```

## Filesystem Source (Default)

The filesystem source is what you'll use most of the time.
It scans directories on disk, following the familiar patterns of a normal project checkout.

```bash
# Index the current directory
crumbly build --context .

# Index a specific project
crumbly build --context ./my-project
```

This works exactly how you'd expect—crumbly walks the directory tree, finds documentation files, and indexes them.

### When to Use Filesystem Source

- Local development checkouts
- Monorepos with multiple projects
- Any normal directory structure

This is the default, so you don't need any special configuration.

## Bare Git Repository Source

Sometimes you need to index repositories that aren't checked out—like on a server hosting bare git repos.
A bare repository has no working directory, just the git database itself.

```
# Normal checkout          # Bare repository
my-project/                my-project.git/
├── .git/                  ├── objects/
├── src/                   ├── refs/
├── docs/                  ├── HEAD
└── README.md              └── config
```

Crumbly can read files directly from bare repositories without checking them out:

```bash
# Index a bare repository
crumbly build --context /srv/git/my-project.git
```

### When to Use Bare Git Source

- Git servers (Gitea, GitLab, self-hosted)
- CI/CD pipelines with shallow clones
- Indexing many repos without disk overhead of full checkouts

Crumbly automatically detects bare repositories, so you don't need to configure anything special.

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
# Only index documentation, not generated code
targets = [
    "docs",
    "packages/core/src",
    "packages/cli/src",
]
```

### Example: Focus on Documentation Only

```toml
# Just the docs folder
targets = ["docs"]
```

### Example: Index Everything (Default)

```toml
# Scan from the root - this is the default if omitted
targets = ["."]
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
# crumbly.toml
use-crumblyignore = true
```

### When to Use Crumblyignore

- Excluding verbose changelogs that clutter search results
- Hiding auto-generated documentation
- Keeping certain files in git but out of search

## Putting It Together

Here's a complete example for a typical project:

```toml
# crumbly.toml

# Focus on these directories
targets = ["docs", "src"]

# Respect .gitignore (default)
respect-gitignore = true

# Also use .crumblyignore for search-specific exclusions
use-crumblyignore = true
```

With a `.crumblyignore`:

```
# Keep changelogs out of search
CHANGELOG.md

# Exclude generated API reference
docs/api/generated/
```

Now when you run `crumbly build`, it will:

1. Scan only `docs/` and `src/`
2. Skip anything in `.gitignore`
3. Also skip anything in `.crumblyignore`
4. Index everything else
