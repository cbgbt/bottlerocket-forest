# Search

This is where crumbly pays off. After building your index, you can find relevant documentation instantly—even when you don't know the exact words to search for.

## Basic Search

Search your indexed content with a simple query:

```bash
crumbly search "how do I configure logging"
```

Crumbly returns the most relevant chunks from your documentation, ranked by similarity to your query.

## Semantic Search, Not Keywords

Unlike grep or traditional search, crumbly understands *meaning*. It finds content that's conceptually similar to your query, even when the exact words don't match.

For example, searching for:

```bash
crumbly search "how to set up authentication"
```

...might find documentation about:
- "Configuring user credentials"
- "Security and access control"
- "Login flow implementation"

None of these contain the word "authentication," but they're all semantically related.

This works because crumbly converts both your query and the indexed content into numerical vectors that capture meaning. Similar concepts end up close together in this vector space, regardless of the specific words used.

## Limiting Results

By default, crumbly returns the top 10 results. Adjust this with `--limit`:

```bash
# Get more results
crumbly search "error handling" --limit 20

# Get just the top match
crumbly search "main entry point" --limit 1
```

## Boost Rules

Not all documentation is equally important. Boost rules let you prioritize certain files or patterns in search results.

A boost rule has two parts:
- **pattern**: A glob pattern matching file paths
- **multiplier**: A factor applied to the similarity score

Multipliers work like this:
- `> 1.0` — Boost matching files higher in results
- `< 1.0` — Push matching files lower in results  
- `= 1.0` — No effect (default)

### Default Boost Rules

Crumbly ships with sensible defaults:

```
+----------+------------+----------------------------------+
| Pattern  | Multiplier | Effect                           |
+----------+------------+----------------------------------+
| README*  | 1.2        | READMEs rank slightly higher     |
| docs/**  | 1.2        | Documentation folders prioritized|
| CHANGELOG| 0.8        | Changelogs rank slightly lower   |
+----------+------------+----------------------------------+
```

These defaults assume you usually want conceptual documentation over release notes.

### Custom Boost Rules

Override the defaults in your `crumbly.toml`:

```toml
# Prioritize API documentation
[[boost-rules]]
pattern = "docs/api/**"
multiplier = 2.0

# Prioritize architecture docs
[[boost-rules]]
pattern = "**/ARCHITECTURE.md"
multiplier = 1.5

# De-prioritize generated files
[[boost-rules]]
pattern = "generated/**"
multiplier = 0.5

# De-prioritize test fixtures
[[boost-rules]]
pattern = "**/testdata/**"
multiplier = 0.3
```

Boost rules are evaluated in order. If multiple patterns match a file, all multipliers are applied.

### Boost Rule Patterns

Patterns use glob syntax:

| Pattern | Matches |
|---------|--------|
| `README*` | README, README.md, README.txt |
| `docs/**` | Everything under docs/ recursively |
| `*.md` | All markdown files |
| `**/api/**` | Any path containing /api/ |
| `src/lib.rs` | Exact file match |

## Practical Examples

### Finding How-To Information

```bash
# Find setup instructions
crumbly search "getting started with the project"

# Find configuration options
crumbly search "available configuration settings"

# Find error explanations
crumbly search "what does connection refused mean"
```

### Exploring Unfamiliar Codebases

```bash
# Understand the architecture
crumbly search "high level system design"

# Find the entry point
crumbly search "where does execution begin"

# Understand a concept
crumbly search "how does caching work"
```

### Boost Configuration for Different Projects

**Documentation-heavy project:**

```toml
[[boost-rules]]
pattern = "docs/**"
multiplier = 2.0

[[boost-rules]]
pattern = "**/README.md"
multiplier = 1.5
```

**API-focused project:**

```toml
[[boost-rules]]
pattern = "**/api/**"
multiplier = 2.0

[[boost-rules]]
pattern = "**/*_api.md"
multiplier = 1.8
```

**Monorepo with many packages:**

```toml
# Boost the packages you care about
[[boost-rules]]
pattern = "packages/core/**"
multiplier = 1.5

# De-prioritize vendored dependencies
[[boost-rules]]
pattern = "vendor/**"
multiplier = 0.2
```

## Tips for Effective Searches

1. **Ask questions naturally** — "how do I configure X" often works better than just "X configuration"

2. **Be specific about intent** — "error handling best practices" vs just "errors"

3. **Try different phrasings** — if one query doesn't find what you need, rephrase it

4. **Use boost rules strategically** — if you keep finding irrelevant results from certain directories, de-prioritize them

5. **Start broad, then narrow** — use `--limit 20` to see more context, then refine your query
