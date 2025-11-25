# Feature Documentation

This directory contains feature proposals and designs for bottlerocket-forest crates.

## Structure

Each feature lives in its own numbered directory:

```
docs/features/
├── 0001-semantic-search/
│   ├── concept.md
│   ├── requirements.md
│   └── design.md
└── 0002-registry-management/
    ├── concept.md
    ├── requirements.md
    └── design.md
```

## Three-Document System

Each feature has three documents that evolve through development:

### 1. Concept (`concept.md`)

**Purpose**: Pitch the feature and explain the "why" and "what"

**Contents**:
- Problem statement
- Proposed solution
- How it works (high-level)
- Benefits
- Technical notes (brief)

**Audience**: Anyone evaluating whether this feature should exist

### 2. Requirements (`requirements.md`)

**Purpose**: Formal specification using EARS notation

**Contents**:
- Functional requirements (EARS format)
- Non-functional requirements
- Edge cases and error conditions

**Audience**: Implementors who need to know exactly what the system must do

**Format**: Use EARS keywords (WHILE, WHEN, WHERE, THEN, SHALL)

### 3. Design (`design.md`)

**Purpose**: Architecture and design guidance for implementation

**Contents**:
- Architecture overview
- Layer responsibilities
- Domain model (types, operations)
- Module structure
- Design patterns
- Minimal illustrative code

**Audience**: Developers implementing the feature

**Note**: Avoid large code dumps - guide implementors, don't write the implementation

## Feature Lifecycle

Features track their status in the concept document frontmatter:

```yaml
---
feature: NNNN-feature-name
status: proposed
---
```

### Status Values

- **proposed**: Concept exists, under review
- **in-development**: Approved and being implemented
- **completed**: Implemented and merged
- **discarded**: Decided not to pursue

### Optional Metadata

Add tracking information as needed:

```yaml
---
feature: NNNN-feature-name
status: in-development
tracking-issue: bottlerocket-os/bottlerocket#1234
---
```

Update the status as the feature progresses through development.

## Naming Convention

Features use four-digit prefixes for scalability:

```
0001-feature-name
0002-another-feature
...
9999-last-feature
```

Use lowercase with hyphens, keep names concise and descriptive.

## Requirements ID Conventions

Use feature-specific prefixes to avoid collisions:

- Prefix should be short (2-5 chars) and descriptive
- Use separate numbering for functional vs non-functional if needed
- Examples: `SEM-1`, `REG-1`, `SEM-NFR-1`

## Creating a New Feature

Use the three-phase skill workflow:

**Phase 1: Concept**
```
Use the propose-feature-concept skill
```
Creates the concept document to pitch the feature idea.

**Phase 2: Requirements**
```
Use the propose-feature-requirements skill
```
Creates the formal specification with EARS notation.

**Phase 3: Design**
```
Use the propose-feature-design skill
```
Creates the technical design and implementation guidance.

Or manually:
1. Find the next available number
2. Create directory: `docs/features/NNNN-feature-name/`
3. Copy templates from `docs/features/0000-templates/`
4. Fill in concept document first
5. Add requirements once concept is approved
6. Add design once requirements are complete
