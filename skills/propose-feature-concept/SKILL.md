---
name: propose-feature-concept
description: Create a new feature concept document to pitch the idea and explain the problem/solution
---

# Propose Feature Concept Skill

## Purpose

Create a feature concept document that pitches the feature idea and explains the problem it solves. This is the first step in the feature proposal process.

## When to Use

- User wants to propose a new feature for sembly, forester, or other forest crates
- Starting to document a feature idea
- Need to explain "why" and "what" before diving into requirements

## Prerequisites

- Forest is seeded (`./seed-forest.sh` has been run)
- User has described the feature idea

## Procedure

### 0. Brownfield Check and Preparation

Ask the user:

> Does this feature modify existing functionality, or is it entirely new (greenfield)?

**If brownfield** (modifies existing code):
1. Use `brownfield-research` skill first to understand current state
2. Then offer idea-honing (informed by research)
3. If idea-honing surfaces new code questions, do another research pass
4. Return here after both are complete

**If greenfield** (entirely new):
1. Offer idea-honing directly (see below)
2. Return here after complete

### 0a. Offer Idea Honing (Optional)

Ask the user:

> Would you like to use an idea honing process to bring more clarity to the concept? This involves working through questions one at a time to explore ambiguities and design considerations.

If yes, use the `idea-honing` skill following the protocol in `skills/README.md`. Return to this skill after idea honing is complete.

### 1. Determine Feature Number

Find the next available feature number:

```bash
ls -1d docs/features/[0-9][0-9][0-9][0-9]-* 2>/dev/null | tail -1
```

If no features exist, start with `0001`. Otherwise, increment the last number.

### 2. Create Feature Name

Work with the user to create a concise, descriptive name:
- Use lowercase with hyphens
- Keep it short (2-4 words)
- Make it descriptive

Example: `semantic-search`, `registry-management`, `skill-validation`

### 3. Check for Idea Honing Document

```bash
ls planning/NNNN-feature-name/idea-honing.md 2>/dev/null
```

If it exists, reference it when writing the concept. The Q&A provides valuable material for the narrative.

### 4. Create Feature Directory

```bash
mkdir -p docs/features/NNNN-feature-name
```

Replace `NNNN` with the four-digit number and `feature-name` with the agreed name.

### 5. Copy Concept Template

```bash
cp docs/features/0000-templates/concept.md docs/features/NNNN-feature-name/
```

### 6. Fill in Concept Document

Work with the user to complete `concept.md` as a narrative:

**Frontmatter**
- Set `feature:` to `NNNN-feature-name`
- Set `status:` to `proposed`
- Add optional `tracking-issue:` if applicable

**Problem Section**
- Paint a picture of the current situation
- Describe the pain this causes
- Explain why this matters

**Solution Section**
- Describe what users will experience
- Focus on "what" and "why" rather than "how"
- Keep it narrative, not a list

**How It Works Section**
- Tell the story of using the feature
- Walk through the workflow naturally
- Show real usage, not abstract steps

**Benefits Section**
- Explain the value provided
- Connect back to the problem
- Show what becomes possible

**Technical Notes Section**
- Brief constraints or considerations
- Keep it short - details go in design.md

### 7. Review for Narrative Flow

Ensure the document:
- Reads like a story, not a specification
- Avoids bullet points and numbered lists where possible
- Focuses on user experience and value
- Explains "why" before "what"

## Validation

Verify the concept was created correctly:

```bash
# Check directory exists
ls -la docs/features/NNNN-feature-name/

# Verify concept file exists
ls docs/features/NNNN-feature-name/concept.md

# Check it has content
cat docs/features/NNNN-feature-name/concept.md
```

## Common Issues

**Too technical**: If the concept reads like a design doc, refocus on the problem and user experience.

**Too abstract**: If the concept is vague, work with the user to add concrete examples of the pain point.

**List-heavy**: If there are many bullet points, rewrite as narrative prose.

## Next Steps

After creating the concept:
1. Review and refine the narrative
2. Get feedback on whether the feature is valuable
3. Once concept is solid, move to requirements using `propose-feature-requirements` skill
