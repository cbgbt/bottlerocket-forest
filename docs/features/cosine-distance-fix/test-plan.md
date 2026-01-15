# Test Plan: Cosine Distance Fix

## Overview

Verify that crumbly correctly uses cosine distance for vector similarity search, handles schema migration gracefully, and preserves contexts during rebuild.

## Test Types

- **Unit**: Test internal logic in isolation, mocks allowed
- **Integration**: Exercise actual CLI binary, NO mocks

## Requirements Coverage

| Req ID | Test Type | Test Name | Description |
|--------|-----------|-----------|-------------|
| R1 | unit | test_check_schema_version_v4_fails_with_helpful_error | Verify v4 schema triggers SchemaMismatch error |
| R1 | integration | test_v4_database_shows_rebuild_suggestion | CLI shows helpful message when opening v4 database |
| R2 | unit | test_create_tables_uses_cosine_distance | Verify vec_chunks DDL includes distance_metric=cosine |
| R3 | unit | test_rebuild_preserves_contexts | Verify contexts are read before delete and restored after |
| R3 | integration | test_rebuild_retains_all_contexts | CLI rebuild preserves non-default contexts |
| R4 | integration | test_search_scores_in_expected_range | Good matches return scores 0.5-0.8 after rebuild |

## Integration Test Requirements

For CLI tests:
- Exercise actual `crumbly` binary
- Use real filesystem fixtures
- Verify stdout/stderr output
- Do NOT mock internal APIs

## Test Implementation Notes

### R1: Schema v4 Detection

Unit test in `schema.rs`:
- Create v4 schema manually (set version=4 in index_metadata)
- Call `check_schema_version()`
- Assert returns `SchemaMismatch { stored: 4, expected: 5 }`

Integration test:
- Create fixture with v4 database
- Run `crumbly search "test"`
- Assert stderr contains "rebuild" suggestion

### R2: Cosine Distance DDL

Unit test in `schema.rs`:
- Call `create_tables()`
- Query sqlite_master for vec_chunks table SQL
- Assert contains `distance_metric=cosine`

### R3: Context Preservation

Unit test in `builder.rs`:
- Create index with multiple contexts
- Call rebuild
- Assert all contexts exist after rebuild

Integration test:
- Build index with `--context ./subdir`
- Run `crumbly rebuild`
- Verify context still searchable with `--context ./subdir`

### R4: Score Range Validation

Integration test:
- Create fixture with known content
- Build index
- Search for exact phrase from content
- Assert top result score >= 0.5
