# Extensible Chunker Abstraction - Implementation Plan

## Commit Checklist

- [ ] **Commit 1**: Add Unknown variant to ChunkContext enum
- [ ] **Commit 2**: Update serialization to handle unknown context types with warning
- [ ] **Commit 3**: Add schema migration v3→v4 (remove CHECK constraint)
- [ ] **Commit 4**: Bump SCHEMA_VERSION to 4

## Commit Details

### Phase 1: Domain Layer Changes

---

#### Commit 1: Add Unknown variant to ChunkContext enum

**Summary**: Extends ChunkContext enum with an Unknown variant that preserves unrecognized context type strings. This enables forward compatibility—databases written by newer crumbly versions remain readable by older versions.

**Files Changed**:
- `crates/crumbly-core/src/knowledge/domain/chunk.rs` - Add Unknown variant to ChunkContext

**Key Changes**:
- Add `Unknown { type_name: String, raw_data: String }` variant to ChunkContext enum
- Variant stores original type string and raw JSON data for round-tripping

**Requirements Addressed**:
- ECA-2: Unknown context type preservation
- ECA-5: Forward compatibility

**Testing**: Unit test that Unknown variant can be constructed and fields accessed.

**Dependencies**: None

---

### Phase 2: Serialization Layer

---

#### Commit 2: Update serialization to handle unknown context types

**Summary**: Modifies deserialize_context to return Unknown variant instead of error for unrecognized types, emitting a warning log. Updates serialize_context to round-trip Unknown variants correctly.

**Files Changed**:
- `crates/crumbly-core/src/knowledge/storage/sqlite/serialization.rs` - Handle unknown types in deserialize_context and serialize_context

**Key Changes**:
- deserialize_context: Return ChunkContext::Unknown for unrecognized types instead of error
- deserialize_context: Emit tracing::warn for unknown types
- serialize_context: Handle Unknown variant by using stored type_name and raw_data
- Preserve error for empty/null context_type (ECA-ERR-2)

**Requirements Addressed**:
- ECA-2: Unknown context type preservation
- ECA-3: Unknown context type warning
- ECA-5: Forward compatibility
- ECA-ERR-2: Error on malformed context data

**Testing**:
- Unit test: deserialize unknown type returns Unknown variant
- Unit test: deserialize unknown type emits warning (use tracing-test)
- Unit test: serialize Unknown round-trips correctly
- Unit test: empty context_type returns error

**Dependencies**: Commit 1

---

### Phase 3: Schema Migration

---

#### Commit 3: Add schema migration v3→v4 (remove CHECK constraint)

**Summary**: Adds migration that removes the CHECK constraint from context_type column, allowing any string value. Migration is O(1) since it only modifies table definition, not row data.

**Files Changed**:
- `crates/crumbly-core/src/knowledge/storage/schema.rs` - Add migrate_v3_to_v4, update check_schema_version, update CREATE_CHUNKS

**Key Changes**:
- Add migrate_v3_to_v4 function using table rebuild pattern (CREATE new, INSERT SELECT, DROP old, RENAME)
- Update check_schema_version to handle (3, 4) migration path
- Update CREATE_CHUNKS constant to remove CHECK constraint
- Migration wrapped in transaction with rollback on failure

**Requirements Addressed**:
- ECA-1: Schema extensibility
- ECA-6: Schema migration v3→v4
- ECA-NFR-1: O(1) migration time
- ECA-ERR-1: Migration rollback on failure

**Testing**:
- Unit test: migrate_v3_to_v4 succeeds and updates version
- Unit test: existing data preserved after migration
- Unit test: new context_type values can be inserted after migration
- Unit test: migration rolls back on failure

**Dependencies**: Commits 1, 2

---

#### Commit 4: Bump SCHEMA_VERSION to 4

**Summary**: Updates SCHEMA_VERSION constant to 4, activating the new schema for fresh databases.

**Files Changed**:
- `crates/crumbly-core/src/knowledge/constants.rs` - Update SCHEMA_VERSION to 4

**Key Changes**:
- Change `pub const SCHEMA_VERSION: u32 = 3;` to `pub const SCHEMA_VERSION: u32 = 4;`
- Update doc comment to describe v4 changes

**Requirements Addressed**:
- ECA-1: Schema extensibility (completes the change)
- ECA-4: Backward compatibility (v4 reads v3 data via migration)

**Testing**: Existing schema tests should pass. Integration test: fresh database uses v4 schema.

**Dependencies**: Commit 3

---

## Parallelization Notes

- Commits 1 and 2 must be sequential (2 depends on Unknown variant from 1)
- Commits 3 and 4 must be sequential (4 activates migration from 3)
- All commits are sequential due to dependencies

## Validation

After all commits:
1. `make integ` passes
2. Fresh database has schema v4 without CHECK constraint
3. v3 database migrates to v4 successfully
4. Unknown context types deserialize with warning, not error
5. Unknown context types round-trip through serialize/deserialize

## Open Questions

None - requirements are clear and implementation path is straightforward.
