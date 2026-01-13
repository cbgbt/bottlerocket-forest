# Test Plan: Extensible Chunker Abstraction

## Overview

This test plan verifies that crumbly's storage layer becomes chunker-agnostic, allowing new language chunkers to be added without schema migrations.

## Test Types

- **Unit**: Test internal logic in isolation, mocks allowed
- **Integration**: Touch real external resources (filesystem, database), NO mocks
- **Not testable**: Cannot be verified by automated test (explain why)
- **Out of scope**: Requires external authentication - document but do not implement

## Requirements Coverage

| Req ID | Test Type | Test Name | Description |
|--------|-----------|-----------|-------------|
| ECA-1 | integration | test_stores_arbitrary_context_type | Store chunk with custom context_type string, verify retrieval |
| ECA-2 | unit | test_unknown_variant_preserves_type_string | Deserialize unknown type, verify Unknown variant contains original string |
| ECA-3 | unit | test_unknown_type_emits_warning | Deserialize unknown type, verify warning logged |
| ECA-4 | integration | test_reads_v3_database_chunks | Open v3 database with existing chunks, verify all deserialize |
| ECA-5 | unit | test_unknown_type_round_trips | Load Unknown variant, save, verify type string preserved |
| ECA-6 | integration | test_migration_v3_to_v4_removes_constraint | Migrate v3 database, verify CHECK constraint removed |
| ECA-7 | not-testable | - | Requires code review: verify new chunker needs only impl + registration |
| ECA-NFR-1 | integration | test_migration_time_independent_of_row_count | Migrate databases with 100 vs 10000 rows, verify similar duration |
| ECA-NFR-2 | unit | test_context_type_lookup_is_constant_time | Verify lookup uses HashMap or match, not iteration |
| ECA-ERR-1 | integration | test_migration_rollback_on_failure | Simulate migration failure, verify schema unchanged |
| ECA-ERR-2 | unit | test_empty_context_type_returns_error | Deserialize empty context_type, verify error not Unknown |
| ECA-ERR-2 | unit | test_null_context_type_returns_error | Deserialize null context_type, verify error not Unknown |

## Integration Test Requirements

For database operations, integration tests MUST:
- Use real SQLite databases (in-memory or temp files)
- Exercise actual migration code paths
- NOT mock the storage layer

## Test Implementation Notes

### ECA-1: Schema Extensibility
Create a chunk with `context_type = "java_doc"` (or any arbitrary string), store it, retrieve it, verify the type is preserved.

### ECA-2/ECA-5: Unknown Variant
Test the `ChunkContext` deserialization directly:
- Input: `{"type": "unknown_future_type", "data": {...}}`
- Output: `ChunkContext::Unknown { type_name: "unknown_future_type", raw_data: "..." }`
- Round-trip: serialize Unknown back, verify type_name preserved

### ECA-3: Warning on Unknown Type
Use `tracing-test` or similar to capture log output during deserialization of unknown type.

### ECA-4: Backward Compatibility
Create a v3 database fixture with markdown, rust_doc, go_doc chunks. Open with v4 code, verify all chunks load.

### ECA-6: Migration
Start with v3 schema, run migration, query sqlite_master to verify CHECK constraint no longer exists on context_type column.

### ECA-NFR-1: Migration Performance
Time the migration on databases of different sizes. The migration should be DDL-only (ALTER TABLE or table rebuild), not row-by-row.

### ECA-NFR-2: Lookup Performance
Code review verification: ensure context type mapping uses O(1) data structure (HashMap, match expression).

### ECA-ERR-1: Rollback
Inject failure during migration (e.g., disk full simulation or invalid SQL), verify database remains at v3 schema.

### ECA-ERR-2: Malformed Data
Test deserialization with:
- `context_type = ""`
- `context_type = NULL`
Both should return errors, not Unknown variants.

### ECA-7: New Chunker Registration
This is verified by code review and documentation. The test plan documents that adding a new chunker should require:
1. Chunker implementation (struct implementing trait)
2. One-line dispatcher registration

No schema migration or serialization changes should be needed. This will be validated when Java chunker is added per Appendix C.
