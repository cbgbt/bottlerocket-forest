# Chunking

Documents are often too large to search effectively as a single unit.
A 500-line README might discuss installation, configuration, troubleshooting, and API reference—all very different topics.
If you search for "installation", you don't want the entire file; you want the relevant section.

Chunking solves this by splitting documents into smaller, focused pieces.
Each chunk becomes independently searchable, so queries match the most relevant portions of your content.

## How Chunking Works

Crumbly uses different chunking strategies depending on file type:

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  Source Files   │ ──▶ │     Chunker      │ ──▶ │  Searchable     │
│                 │     │  (by file type)  │     │    Chunks       │
└─────────────────┘     └──────────────────┘     └─────────────────┘
     .md files     ──▶   MarkdownChunker    ──▶   Heading sections
     .rs files     ──▶   RustDocChunker     ──▶   Doc comments
     .go files     ──▶   GoDocChunker       ──▶   Doc comments
```

## Markdown Chunking

Markdown files are split by heading structure.
Each heading starts a new chunk that includes all content until the next heading of equal or higher level.

Consider this document:

```markdown
# Getting Started

Welcome to the project.

## Installation

Run `cargo install myapp`.

## Configuration

Create a config file...

### Advanced Options

For power users...
```

This produces three chunks:

1. **"Getting Started"** — The intro paragraph
2. **"Installation"** — The install instructions
3. **"Configuration > Advanced Options"** — Config content plus the nested advanced section

Each chunk preserves its heading hierarchy, so search results show context like "Configuration > Advanced Options" rather than just "Advanced Options".

## Rust Doc Comments

For Rust files, crumbly extracts documentation comments rather than code.
This focuses search on the explanatory content that developers write to describe their APIs.

```rust
/// Validates user input against the schema.
///
/// Returns `Ok(())` if valid, or an error describing
/// what failed validation.
///
/// # Examples
///
/// ```
/// let result = validate(&input, &schema);
/// assert!(result.is_ok());
/// ```
pub fn validate(input: &str, schema: &Schema) -> Result<()> {
    // ...
}
```

The doc comment becomes a searchable chunk, tagged with the item name (`validate`) and type (`function`).
The function body is not indexed—only the documentation.

## Go Doc Comments

Go files work similarly.
Doc comments preceding functions, types, and packages are extracted:

```go
// ParseConfig reads a configuration file and returns
// the parsed settings. It supports TOML and YAML formats.
//
// If the file doesn't exist, it returns default settings
// rather than an error.
func ParseConfig(path string) (*Config, error) {
    // ...
}
```

This doc comment becomes a chunk associated with the `ParseConfig` function.

## Enabling File Types

By default, crumbly only indexes markdown files.
Enable additional file types in your `crumbly.toml`:

```toml
enabled-file-types = ["markdown", "rust", "go"]
```

Available types:
- `markdown` — `.md` files (enabled by default)
- `rust` — `.rs` files
- `go` — `.go` files

## Chunk Size Controls

Two settings control how large chunks can be:

```toml
max-tokens = 512
overlap-tokens = 50
```

**max-tokens** sets the maximum size of a chunk.
If a markdown section exceeds this limit, it's split into multiple chunks.

**overlap-tokens** controls how much content is repeated between consecutive chunks.
This overlap helps preserve context when a topic spans chunk boundaries.

```
┌─────────────────────────────────────────────────────────┐
│                    Long Section                         │
└─────────────────────────────────────────────────────────┘
                          ▼
┌───────────────────────┐
│       Chunk 1         │
│                  ─────┼───┐  ◀── overlap
└───────────────────────┘   │
                   ┌────────┼──────────────┐
                   │        │   Chunk 2    │
                   └────────┴──────────────┘
```

The defaults work well for most projects.
Increase `max-tokens` if your documentation has long, indivisible sections.

## Filtering Rust and Go Chunks

For code documentation, you often want to index only public APIs or skip trivial items.
Crumbly provides filters for this:

```toml
[file-types.rust]
visibility = ["public"]
items = ["function", "struct", "trait", "enum", "module"]
min-doc-lines = 2

[file-types.go]
visibility = ["exported"]
items = ["function", "type", "package"]
min-doc-lines = 2
```

### visibility

Controls which items are indexed based on their visibility:

- **Rust**: `public`, `private`, `crate`, `restricted`
- **Go**: `exported` (capitalized names), `unexported`

Default: all visibilities are included.

### items

Limits indexing to specific item types:

- **Rust**: `function`, `struct`, `trait`, `enum`, `module`, `const`, `static`, `type`
- **Go**: `function`, `type`, `package`, `const`, `var`

Default: all item types are included.

### min-doc-lines

Skips items with fewer than N lines of documentation.
This filters out trivial one-liner comments:

```rust
/// Returns the ID.        // 1 line - skipped if min-doc-lines = 2
pub fn id(&self) -> u32

/// Processes the request and returns a response.
/// Handles authentication, validation, and routing.
pub fn process(&self)      // 2 lines - included
```

Default: `1` (include everything with at least one doc line).

## Example Configuration

Here's a complete chunking configuration for a mixed Rust/Go project:

```toml
# crumbly.toml

enabled-file-types = ["markdown", "rust", "go"]
max-tokens = 512
overlap-tokens = 50

[file-types.rust]
visibility = ["public", "crate"]
items = ["function", "struct", "trait", "enum"]
min-doc-lines = 2

[file-types.go]
visibility = ["exported"]
min-doc-lines = 2
```

This indexes:
- All markdown files, split by headings
- Public and crate-visible Rust items with 2+ lines of docs
- Exported Go items with 2+ lines of docs

## Next Steps

Once content is chunked, it moves to the [embedding](./embedding.md) stage where chunks are converted to vectors for semantic search.
