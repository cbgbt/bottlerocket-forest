# Pipeline Overview

Before diving into the details, let's see how crumbly transforms your documentation into a searchable index.

## The Big Picture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         CRUMBLY PIPELINE                                │
└─────────────────────────────────────────────────────────────────────────┘

    ┌──────────┐    ┌──────────┐    ┌───────────┐    ┌─────────┐    ┌────────┐
    │  Content │    │          │    │           │    │         │    │        │
    │  Sources │───▶│ Chunking │───▶│ Embedding │───▶│ Storage │───▶│ Search │
    │          │    │          │    │           │    │         │    │        │
    └──────────┘    └──────────┘    └───────────┘    └─────────┘    └────────┘
         │               │               │               │              │
    Find files      Split into      Convert to       Save to        Query
    to index        pieces          vectors          database       index
```

## What Each Stage Does

**Content Sources** discover which files to index.
By default, crumbly walks your filesystem respecting `.gitignore` rules.
It can also index bare git repositories directly.

**Chunking** splits documents into searchable pieces.
A 500-line README becomes multiple focused chunks, each representing a logical section.
Different chunkers handle different file types—markdown splits by headings, while Rust and Go files extract documentation comments.

**Embedding** converts text into vectors.
Each chunk becomes a 384-dimensional vector that captures its semantic meaning.
Similar content produces similar vectors, which is what makes semantic search work.

**Storage** persists everything to a SQLite database.
Chunks, vectors, and metadata all live in `.crumbly/index.db`.
Content-addressing means identical text shares the same embedding, saving space and time.

**Search** finds relevant chunks for your query.
Your search terms become a vector, then crumbly finds the chunks with the most similar vectors.
Boost rules let you prioritize certain files (like giving README files a relevance bump).

## The Flow

When you run `crumbly build`, here's what happens:

```
Your Files                    The Index
──────────                    ─────────

docs/
├── guide.md      ──┐
├── api.md          │        ┌─────────────────┐
└── examples/       ├──────▶ │  .crumbly/      │
    └── basic.md    │        │  └── index.db   │
src/                │        └─────────────────┘
└── lib.rs        ──┘              │
                                   │
                                   ▼
                          "How do I configure X?"
                                   │
                                   ▼
                          ┌─────────────────────┐
                          │ docs/guide.md:42    │
                          │ src/lib.rs:15       │
                          └─────────────────────┘
```

1. **Files in**: Content sources find `guide.md`, `api.md`, `basic.md`, and `lib.rs`
2. **Chunks**: Each file is split into logical pieces (sections, doc comments)
3. **Vectors**: Every chunk gets converted to a 384-dimensional vector
4. **Stored**: Chunks and vectors go into the SQLite database
5. **Searchable**: Queries find the most semantically similar chunks

## Quick Example

Build an index:

```bash
crumbly build --context ./my-project
```

Search it:

```bash
crumbly search "authentication flow"
```

That's the whole pipeline in action.

## What's Next

The following chapters explore each stage in detail:

- **Content Sources** — How crumbly discovers files to index
- **Chunking** — How documents get split into searchable pieces  
- **Embedding** — How text becomes vectors
- **Storage** — How the index is organized
- **Search** — How queries find relevant results

You don't need to understand every stage to use crumbly effectively, but knowing the pipeline helps when you want to tune behavior or troubleshoot results.
