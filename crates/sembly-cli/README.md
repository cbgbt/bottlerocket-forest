# Sembly

Sembly is a semantic search tool for exploring documentation using ML embeddings. It provides fast, targeted documentation lookup across large codebases.

## Purpose

Sembly enables semantic search across documentation, making it easy to find relevant information even when you don't know the exact keywords. It uses machine learning embeddings to understand the meaning of your queries and match them with relevant documentation.

## Installation

Build from source:

```bash
cd bottlerocket-forest
cargo build --release -p sembly-cli
```

The binary will be at `target/release/sembly`.

## Usage

Build the index (first time or after major changes):

```bash
sembly build
```

Search the documentation:

```bash
sembly search "how to build a kit"
sembly search "boot process" --limit 5
sembly search "systemd configuration" --show-chunks
```

Check index status:

```bash
sembly status
```

Update incrementally (faster than full rebuild):

```bash
sembly update
```

Rebuild from scratch:

```bash
sembly rebuild
```

Clear the index:

```bash
sembly clear
```

## How It Works

Sembly uses the `sentence-transformers/all-MiniLM-L6-v2` model for embeddings and stores data in `.sembly/knowledge/` at the forest root. It automatically discovers documentation from all repositories and supports markdown files, Rust source files, and other text formats.

The index downloads ML models automatically on first use (~90MB for the embedding model).

## Configuration

Sembly looks for a `.sembly.toml` configuration file in the forest root to determine which repositories and paths to index. If not found, it uses sensible defaults.

## Library Usage

Sembly is built on `sembly-core`, a reusable library for semantic search. You can use it in your own projects:

```toml
[dependencies]
sembly-core = "0.1.0"
```

See `crates/sembly-core/` for library documentation.
