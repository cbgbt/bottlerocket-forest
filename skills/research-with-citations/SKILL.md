---
name: research-with-citations
description: Answer questions about Bottlerocket by finding and reading relevant documentation
---

# Research with Citations

A systematic approach to answering questions about Bottlerocket by searching documentation, reading relevant files, and providing complete citations.

## Purpose

Ensures accurate, well-sourced answers to questions about Bottlerocket architecture, features, and implementation by:
- Using semantic search to find relevant documentation
- Reading actual source files rather than relying on general knowledge
- Providing complete citations for all information

## When to Use

- User asks questions about Bottlerocket internals, architecture, or features
- Need to understand how a system component works
- Investigating configuration options or behavior
- Any question that should be answered from documentation rather than general knowledge

## Prerequisites

- Forest has been seeded (`./seed-forest.sh`)
- Forester index is built and up-to-date

## Procedure

### 1. Formulate Search Query

Create a concise search query with key terms related to the question:
```bash
./forester/target/release/forester index search "key terms from question"
```

**Tips for effective queries:**
- Include technical terms (e.g., "partition", "dm-verity", "boot")
- Use multiple related concepts together
- Avoid overly generic terms

### 2. Review Search Results

Examine the results for:
- Match count and relevance scores
- File paths that indicate relevant content
- Top 3-5 results are usually most relevant

### 3. Read Identified Files

For each relevant file, read the content:
```bash
# For targeted reading with pattern matching
fs_read with mode: "Search", pattern: "relevant terms", context_lines: 5

# For complete file reading
fs_read with mode: "Line", start_line: 1, end_line: -1
```

**Reading strategy:**
- Start with highest-scoring results
- Use pattern search first to locate relevant sections
- Read full sections or entire files as needed
- Continue to additional files if information is incomplete

### 4. Synthesize and Cite

When providing the answer:
- Synthesize information from multiple sources
- Structure the response logically
- **Always include a "Sources" section** listing:
  - Full file paths
  - What information came from each file
  - Key details or quotes if helpful

**Citation format:**
```
## Sources

1. **`/path/to/file.md`**
   - Specific information found
   - Key concepts explained

2. **`/path/to/another/file.rs`**
   - Additional details
   - Implementation specifics
```

### 5. Iterate if Needed

If initial search doesn't yield complete information:
- Refine search terms based on what you learned
- Search for related concepts discovered in first pass
- Look for cross-references mentioned in documentation

## Example Workflow

**Question:** "How does Bottlerocket's root volume work?"

**Step 1 - Search:**
```bash
./forester/target/release/forester index search "root volume partition disk layout"
```

**Step 2 - Review results, identify top files:**
- `signpost/README.md` (score: 0.879)
- `SECURITY_FEATURES.md` (score: 0.766)

**Step 3 - Read files:**
- Read signpost README for partition structure
- Read SECURITY_FEATURES for dm-verity and immutability
- Search variants README for layout options

**Step 4 - Provide answer with citations:**
- Explain dual partition sets (from signpost)
- Describe security features (from SECURITY_FEATURES)
- Note configuration options (from variants README)
- List all sources with specific details

## Validation

A good research response includes:
- ✓ Information directly from documentation files
- ✓ Multiple sources cross-referenced
- ✓ Complete file paths cited
- ✓ Specific details attributed to sources
- ✓ No speculation or general knowledge where docs exist

## Common Patterns

**Architecture questions:** Search for component names + "architecture", "design", "structure"

**Configuration questions:** Search for setting names + "config", "toml", "metadata"

**Security questions:** Check `SECURITY_FEATURES.md`, `SECURITY_GUIDANCE.md` first

**Build system questions:** Look in `docs/build-system.md`, `Makefile`, `Twoliter.toml`

**Implementation details:** Search for package names, then read source in `sources/` or `packages/`

## Notes

- The forester index covers documentation, not source code - use grep/find for code search
- Documentation may be in multiple repos (bottlerocket, kits, twoliter, etc.)
- README files are often the best starting point for a component
- Changelog files can explain historical context for features
