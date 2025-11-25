# Feature Name - Technical Design

## Overview

High-level description of the architecture and design approach.

**Design Philosophy**: Focus on types, their relationships, and key architectural decisions.
Leave specific coding decisions to implementors—this document disambiguates the important changes, not every detail.

## Architecture

Describe the overall structure:

```
Component A
  ↓
Component B
  ↓
├─ Module X
├─ Module Y
└─ Module Z
```

### Layer Responsibilities

**Domain** - Core types and business logic, no external dependencies

**Ports** - Trait definitions for external capabilities

**Adapters** - Implementations of port traits (filesystem, network, database, etc.)

**Application** - Wires adapters to domain, orchestrates workflows

## Domain Model

### Core Types

**TypeName**
- Purpose and role
- Key properties
- Validation rules

**AnotherType**
- Purpose and role
- Key properties

### Domain Operations

Specify key operations with their signatures and error types:

**operation_name(param: ParamType) -> Result<OutputType, OperationError>**
- What it does
- Key behaviors

### Error Types

Define error type hierarchy. Each fallible operation typically has its own error type.

**OperationError**
- Variants and when they occur
- What lower-level errors it wraps

## Boundaries & Adapters

Define traits for external dependencies to enable testing and flexibility.

**Trait: TraitName**
- Purpose: What external capability this abstracts
- Key methods and their semantics
- Implementations: production impl, test/mock impl

Adapters should be thin—translate between external systems and domain types.
Domain logic should never depend on concrete adapters, only on trait definitions.

## Module Structure

```
src/
├── domain/       # Core types and logic
├── ports/        # Trait definitions
├── adapters/     # Trait implementations
└── app/          # Application wiring
```

## Migration from Current Design

*Remove this section for greenfield features.*

### Current State

- Key types and their roles
- Current data flow
- Pain points being addressed

### Changes Required

**Types**
- `ExistingType` → changes to...
- New type `NewType` introduced for...

**Data Structures**
- Current: describe current structure
- New: describe new structure

**Processes**
- `existing_function()` currently does X, will change to Y
- New process `new_function()` added for...

### Affected Modules

List modules that require changes and summarize the nature of changes.

## Design Patterns

Describe relevant patterns:
- Pattern name: Why and how it applies
- Another pattern: Usage guidance

## Implementation Guidance

Key considerations for implementors:
- Guideline 1
- Guideline 2
- Guideline 3

## Testing Strategy

- Unit tests: domain logic with mock adapters
- Integration tests: real adapters, component interactions
- Edge cases to cover

## Notes

- Focus on types and their interactions
- Guide architectural decisions, not implementation details
- Reference requirements by ID when relevant
