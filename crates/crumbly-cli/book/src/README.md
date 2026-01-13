# Introduction

Crumbly is a semantic search tool for documentation.
It indexes markdown files and doc comments from your codebase, then lets you search by meaning rather than exact keywords.

## The Problem

Code repositories grow.
As they do, humans and AI agents have a harder time figuring out which parts of the code are relevant to a particular feature or task.

Grep helps, but only when you know the right search terms.
If the codebase uses different terminology than you expect, grep comes up empty.

For AI agents, this is especially painful.
Agents exploring unfamiliar repositories often fail to find the right code simply because they don't know what to search for.
They waste context window on irrelevant files or miss critical pieces entirely.

Crumbly addresses this by providing semantic search over your documentation.
It sidesteps the problems of indexing source code directly while still making your codebase navigable.

## Why Documentation, Not Code?

Crumbly deliberately indexes documentation rather than source code.
Code indices become stale quickly—functions get renamed, files move around, and the index drifts from reality.
Documentation changes less frequently and captures intent, not just implementation.

That said, crumbly still helps even if you haven't written much standalone documentation.
It extracts doc comments from Rust and Go source files, so your inline documentation becomes searchable too.

## What Gets Indexed

- Markdown files (`.md`)
- Rust doc comments (`///` and `//!`)
- Go doc comments

Each document is split into chunks based on its structure—headings for markdown, individual documented items for code.
These chunks are converted to vector embeddings that capture semantic meaning.

## Basic Usage

Build an index:

```bash
crumbly build --context ./my-project
```

Search:

```bash
crumbly search "how does authentication work"
```

The search finds relevant chunks even if they don't contain your exact words.
"Authentication" might match documentation about "login flow" or "credential validation."

## What's in This Book

The following chapters walk through crumbly's pipeline:

1. **Pipeline Overview** — How documents flow from source to searchable index
2. **Content Sources** — Where crumbly finds files to index
3. **Chunking** — How documents are split into searchable pieces
4. **Embedding** — How text becomes vectors
5. **Storage** — Where the index lives
6. **Search** — How to query and tune results
7. **Configuration** — All the knobs you can turn
