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

**YOU MUST COMPLETE THESE FIRST:**

1. Run `./seed-forest.sh` from forest root
2. Verify forester exists: `./forester/target/release/forester --version`
3. Verify index exists: `./forester/target/release/forester index status`

**If any of these fail, STOP and run the seed script.**

## Procedure

**💡 TIP: If you have todolist functionality, create a task list for these steps.** This helps track progress through the research workflow.

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
- **Use numeric references [1], [2], etc. inline** when stating facts
- **Always include a "Sources" section** at the end with numbered citations

**Citation format:**
```
The root filesystem is immutable [1] and verified with dm-verity [2].
Bottlerocket uses dual partition sets [1] for atomic updates.

## Sources

[1] **`bottlerocket/SECURITY_FEATURES.md`**
    - Immutable rootfs backed by dm-verity
    - Dual partition sets for updates

[2] **`kits/bottlerocket-core-kit/sources/updater/signpost/README.md`**
    - Partition structure details
    - GPT priority bits system
```

**Guidelines:**
- Number sources in order of first reference
- Use the same number for multiple facts from the same source
- Include full file paths in the Sources section
- Briefly describe what information came from each source

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
- Explain dual partition sets with inline reference [1]
- Describe security features with inline references [2]
- Use numeric citations throughout the answer
- List all sources at the end with numbers matching inline references

## Validation

A good research response includes:
- ✓ Information directly from documentation files
- ✓ Numeric references [1], [2], etc. inline with facts
- ✓ Multiple sources cross-referenced
- ✓ Numbered Sources section at the end
- ✓ Complete file paths cited
- ✓ Specific details attributed to sources
- ✓ No speculation or general knowledge where docs exist

## Common Patterns

**Architecture questions:** Search for component names + "architecture", "design", "structure"

**Configuration questions:** Search for setting names + "config", "toml", "metadata"

**Security questions:** Check `SECURITY_FEATURES.md`, `SECURITY_GUIDANCE.md` first

**Build system questions:** Look in `docs/build-system.md`, `Makefile`, `Twoliter.toml`

**Implementation details:** Search for package names, then read source in `sources/` or `packages/`

## Research Quality Indicator

End your response with one of:

- ✅ *Answered from documentation* - If forester results were sufficient
- 📝 *Answered primarily from source code* - If you needed to read >3 source files

## Notes

- The forester index covers documentation, not source code - use grep/find for code search
- Documentation may be in multiple repos (bottlerocket, kits, twoliter, etc.)
- README files are often the best starting point for a component
- Changelog files can explain historical context for features
