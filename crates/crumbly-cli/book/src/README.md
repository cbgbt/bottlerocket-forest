# Introduction

You have documentation everywhere.
READMEs in each repository, design docs in a wiki, API references generated from code comments.
When you need to find something specific, you grep through directories, open dozens of tabs, and hope you remember where you saw it.

There has to be a better way.

## What is Crumbly?

Crumbly is a semantic search tool for documentation.
Instead of matching exact keywords, it understands *meaning*.
Search for "how do I configure logging" and find relevant docs even if they use words like "log levels" or "debug output" instead.

It's designed for developers and AI agents who need to quickly find information across scattered documentation.

## Quick Start

Build an index of your documentation:

```bash
crumbly build --context ./my-project
```

Search for what you need:

```bash
crumbly search "authentication flow"
```

Crumbly returns the most relevant chunks of documentation, ranked by semantic similarity to your query.

## How It Works

```
┌─────────────┐    ┌──────────┐    ┌───────────┐    ┌────────┐
│ Your Docs   │───▶│ Chunking │───▶│ Embedding │───▶│ Search │
│ .md .rs .go │    │          │    │           │    │        │
└─────────────┘    └──────────┘    └───────────┘    └────────┘
```

Crumbly scans your files, splits them into meaningful chunks, converts those chunks into vectors that capture their meaning, and stores everything for fast retrieval.

## What This Book Covers

- **Getting Started** — Installation and your first search
- **Configuration** — Customizing what gets indexed and how
- **File Types** — Support for Markdown, Rust, and Go documentation
- **Boost Rules** — Prioritizing important documentation in results
- **Architecture** — How the pieces fit together

Let's get started.
