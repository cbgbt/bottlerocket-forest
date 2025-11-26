---
name: brownfield-research
description: Research existing code to understand current state before modifying it
---

# Brownfield Research Skill

## Purpose

Understand the current codebase before proposing changes.
This skill answers "what does the code do today?" through automated research, freeing idea-honing to focus on "what should it do?"

## When to Use

- Feature modifies existing functionality (not greenfield)
- Before idea-honing, to inform the discussion
- After idea-honing, if new code questions emerged

## Prerequisites

- User has described a rough feature idea (1-2 paragraphs)
- Forest is seeded and `sembly` index is current

## Procedure

### 1. Get the Rough Idea

Ask the user to describe the feature in 1-2 paragraphs:
- What problem does it solve?
- What part of the system does it touch?

### 2. Create Planning Directory

```bash
mkdir -p planning/NNNN-feature-name
```

### 3. Initialize Current State Document

```bash
cat > planning/NNNN-feature-name/current-state.md << 'EOF'
# Current State: Feature Name

## Summary

*To be filled after research*

## Affected Areas

*To be filled after research*

## Invariants

*To be filled after research*

## Open Questions

*Questions that need human input (feed into idea-honing)*

---

## Research Log

EOF
```

### 4. Phase 1: Exploratory Research

Generate broad questions to find the right areas:
- "Where does [X] happen in the codebase?"
- "What module is responsible for [Y]?"
- "How does data flow from [A] to [B]?"

For each question:

1. Use `sembly search` to find relevant files
2. Use `./scripts/show FILE:START:END` to examine code with accurate line numbers
3. Summarize what you found (don't dump code)
4. Log the Q&A in the Research Log section

**Example research log entry:**
```markdown
### Q: Where does search filtering happen?

**Search**: `sembly search "search filter results"`

**Findings**: 
- `knowledge/search/semantic.rs` - SemanticSearchEngine.search() is the entry point
- `storage/sqlite/search.rs:35-80` - SQL query construction, currently no filtering
- No context parameter exists today

**Relevance**: This is where filtering logic would need to be added.
```

### 5. Phase 2: Pointed Research

Based on Phase 1 findings, drill into specifics:
- "What are the parameters to `function_name`?"
- "What does `TraitName` require?"
- "What tests cover `module_name`?"
- "What happens when [error condition]?"

Continue logging findings.

### 6. Compile Affected Areas

For each module discovered, document:

```markdown
### Module: `path/to/module`

**Role**: One sentence describing what this module does.

**Key types**:
- `TypeName` - brief description

**Key operations**:
- `function()` - what it does

**Current behavior relevant to this feature**:
- Specific behaviors that matter for this change

**Tests**: List test files/functions that cover this module
```

**Keep it relevant** — only include what matters for this feature.

### 7. Identify Invariants

Document behaviors that must be preserved:
- Explicit invariants (documented or asserted)
- Implicit invariants (tests rely on them)
- Performance characteristics

### 8. List Open Questions

Questions research couldn't answer:
- Design decisions requiring human judgment
- Tradeoffs that need discussion
- Ambiguities in requirements

These become input to idea-honing.

### 9. Write Summary

Condense findings into 2-3 sentences at the top of the document.

## Reading Code

**Always use `./scripts/show` for examining code:**

```bash
# View specific line range (line numbers will be accurate)
./scripts/show path/to/file.rs:50:100

# Search for pattern and show context
./scripts/show path/to/file.rs -p "function_name"

# View whole file
./scripts/show path/to/file.rs
```

**When citing code:**
- Use `file.rs:45-60` format
- Verify line numbers with `./scripts/show`
- Summarize what the code does, don't paste large blocks

## Output

The skill produces `planning/NNNN-feature-name/current-state.md` with:

1. **Summary** — 2-3 sentence overview
2. **Affected Areas** — modules, types, operations, tests
3. **Invariants** — behaviors that must be preserved
4. **Open Questions** — input for idea-honing
5. **Research Log** — detailed Q&A trail

## Validation

```bash
# Check document exists
ls planning/NNNN-feature-name/current-state.md

# Verify it has substance
wc -l planning/NNNN-feature-name/current-state.md  # Should be 50+ lines

# Check for open questions
grep -A5 "Open Questions" planning/NNNN-feature-name/current-state.md
```

## Common Issues

**Too much detail**: Summarize relevance, don't dump code. Ask "does this matter for the feature?"

**Missing areas**: If the feature touches multiple modules, research each. Don't stop at the first hit.

**No open questions**: If everything is clear, great. But usually brownfield work surfaces ambiguities—dig deeper.

**Wrong line numbers**: Always use `./scripts/show` to verify line numbers before citing.

## Next Steps

After brownfield research:
1. Review findings with user
2. Proceed to idea-honing (informed by research)
3. If idea-honing surfaces new code questions, run another research pass
4. Then proceed to concept
