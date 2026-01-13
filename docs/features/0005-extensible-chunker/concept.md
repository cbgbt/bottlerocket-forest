# Extensible Chunker Abstraction

**Status:** proposed

## Problem

Adding a new language chunker to crumbly currently requires touching the database schema.
The chunks table includes a CHECK constraint that enumerates every valid context_type value, so introducing Go support meant creating a schema migration from v2 to v3.
Beyond the migration itself, each new language also requires a new ChunkContext enum variant, hardcoded serialization match arms, and dispatcher registration.

This coupling between storage and chunking creates unnecessary friction.
A developer who wants to add Java support shouldn't need to understand SQLite migrations or coordinate schema changes across environments.
The storage layer has no business knowing which languages exist—it just needs to persist and retrieve chunk metadata.

## Solution

The storage layer becomes chunker-agnostic by treating context_type as an opaque string rather than an enumerated set.

A schema migration removes the CHECK constraint, allowing any context_type value to be stored.
The ChunkContext enum gains an Unknown variant that preserves unrecognized type strings, so databases written by newer versions of crumbly remain readable by older versions.
When deserialization encounters an unknown type, it emits a warning and continues rather than failing.

After this change, adding a new language chunker requires only two things: the chunker implementation itself and a one-line dispatcher registration.
No schema migrations, no serialization changes, no enum modifications.

## How It Works

A developer adding Java support writes a JavaChunker that implements the existing Chunker trait.
They register it with the dispatcher, mapping the ".java" extension to their chunker.
The chunker returns chunks with context_type set to "java_doc" (or whatever identifier makes sense).

When crumbly indexes a Java file, the chunk flows through the existing pipeline unchanged.
The storage layer persists "java_doc" as the context_type string without validation.
Search and retrieval work immediately because they never depended on knowing the specific language types.

If someone opens a database containing Java chunks with an older crumbly version that predates Java support, deserialization creates ChunkContext::Unknown("java_doc") and logs a warning.
The chunk remains searchable and displayable—the unknown type just means language-specific features won't apply.

## Benefits

New language support becomes a self-contained change.
A contributor can add Java chunking in a single PR without touching storage code or creating migrations.
This lowers the barrier for community contributions and makes the codebase easier to understand.

Forward compatibility improves because databases remain readable across versions.
A team can experiment with custom chunkers for internal languages without forking crumbly or maintaining private schema patches.

The separation also clarifies responsibilities: chunkers define how to split content, storage defines how to persist it, and neither needs to know the other's details.

## Technical Notes

The schema migration (v3→v4) drops the CHECK constraint from the context_type column.
Existing data remains valid since current values are a subset of what the new schema allows.

The Unknown variant stores the original type string to enable round-tripping.
A chunk loaded as Unknown and then saved preserves its original context_type.
