# Configuration

Crumbly uses a `crumbly.toml` file to control what gets indexed and how search results are ranked.

## Where to Put Your Config

Place `crumbly.toml` in the root of the directory you want to index:

```
my-project/
├── crumbly.toml    ← config goes here
├── docs/
├── src/
└── README.md
```

Then build your index:

```bash
crumbly build --context ./my-project
```

If no config file exists, crumbly uses sensible defaults.

## Getting Started Config

Here's a good starting point for most projects:

```toml
# What directories to index
targets = ["."]

# What file types to process
enabled-file-types = ["markdown"]

# Boost important docs in search results
[[boost-rules]]
pattern = "README.md"
multiplier = 1.5

[[boost-rules]]
pattern = "docs/**"
multiplier = 1.3
```

This indexes all markdown files, with READMEs and docs/ content ranked higher in results.

## Common Configuration Tasks

### Indexing Multiple Directories

List each directory you want indexed:

```toml
targets = ["docs", "guides", "api"]
```

### Adding Code Documentation

Include doc comments from source files:

```toml
enabled-file-types = ["markdown", "rust", "go"]
```

Crumbly extracts documentation comments (not code) from Rust and Go files.

### Filtering Rust Documentation

Control which Rust items get indexed:

```toml
[file-types.rust]
visibility = ["public"]           # Only public items
items = ["function", "struct", "trait", "module"]
min-doc-lines = 2                  # Skip sparse docs
```

### Filtering Go Documentation

Similar controls for Go:

```toml
[file-types.go]
visibility = ["exported"]         # Only exported items
items = ["function", "type", "method"]
min-doc-lines = 2
```

### Boosting Important Content

Make certain files rank higher in search results:

```toml
[[boost-rules]]
pattern = "architecture.md"
multiplier = 2.0    # 2x boost

[[boost-rules]]
pattern = "CHANGELOG.md"
multiplier = 0.5    # Rank lower
```

Patterns use glob syntax. A multiplier above 1.0 boosts; below 1.0 demotes.

### Excluding Files

Create a `.crumblyignore` file (same syntax as `.gitignore`):

```
# .crumblyignore
vendor/
node_modules/
*.generated.md
```

Or disable gitignore/crumblyignore handling:

```toml
respect-gitignore = false
use-crumblyignore = false
```

## Tips for Tuning

**Start simple.** Begin with just markdown files and add more file types as needed.

**Check your index.** Run `crumbly status` to see what got indexed:

```bash
crumbly status
# Shows: file counts, chunk counts, index size
```

**Test your boosts.** Search for terms and see if the right docs surface first. Adjust multipliers accordingly.

**Use targeted directories.** Instead of indexing everything from root, list specific directories in `targets` to keep your index focused.

---

## Configuration Reference

Complete list of all configuration options.

### Top-Level Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `targets` | array of strings | `["."]` | Directories to scan for content |
| `enabled-file-types` | array of strings | `["markdown"]` | File types to index: `markdown`, `rust`, `go` |
| `respect-gitignore` | boolean | `true` | Honor `.gitignore` patterns |
| `use-crumblyignore` | boolean | `true` | Honor `.crumblyignore` patterns |

### Chunking Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max-tokens` | integer | `512` | Maximum tokens per chunk |
| `overlap-tokens` | integer | `50` | Token overlap between chunks |

### Rust File Type Options

`[file-types.rust]`

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `visibility` | array of strings | `["public", "private"]` | Filter by visibility: `public`, `private` |
| `items` | array of strings | all items | Item types to index: `function`, `struct`, `enum`, `trait`, `module`, `const`, `static`, `type` |
| `min-doc-lines` | integer | `1` | Minimum doc comment lines to include item |

### Go File Type Options

`[file-types.go]`

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `visibility` | array of strings | `["exported", "unexported"]` | Filter by visibility: `exported`, `unexported` |
| `items` | array of strings | all items | Item types to index: `function`, `type`, `method`, `const`, `var` |
| `min-doc-lines` | integer | `1` | Minimum doc comment lines to include item |

### Boost Rules

`[[boost-rules]]`

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `pattern` | string | yes | Glob pattern to match file paths |
| `multiplier` | float | yes | Score multiplier (>1.0 boosts, <1.0 demotes) |

### Example: Full Configuration

```toml
# Directories to index
targets = ["docs", "src", "examples"]

# File types to process
enabled-file-types = ["markdown", "rust", "go"]

# Ignore handling
respect-gitignore = true
use-crumblyignore = true

# Chunking
max-tokens = 512
overlap-tokens = 50

# Rust documentation filters
[file-types.rust]
visibility = ["public"]
items = ["function", "struct", "trait", "enum", "module"]
min-doc-lines = 2

# Go documentation filters
[file-types.go]
visibility = ["exported"]
items = ["function", "type", "method"]
min-doc-lines = 2

# Boost important content
[[boost-rules]]
pattern = "README.md"
multiplier = 1.5

[[boost-rules]]
pattern = "docs/**"
multiplier = 1.3

[[boost-rules]]
pattern = "**/CHANGELOG.md"
multiplier = 0.8
```
