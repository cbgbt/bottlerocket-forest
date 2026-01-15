# Cosine Distance Fix - Implementation Plan (Revised)

## Summary

The original plan required changing the vec_chunks DDL to use `distance_metric=cosine`, but the sqlite-vec Rust crate doesn't support this option. 

**Revised approach:** Fix the L2-to-similarity conversion formula instead. This is simpler and works with existing indexes.

## Commit Checklist

- [x] **Commit 1**: Bump schema version and add v4 detection with helpful error (DONE - but will revert, not needed)
- [ ] **Commit 1 (revised)**: Fix L2-to-cosine-similarity conversion formula
- [ ] **Commit 2**: Preserve contexts during rebuild  
- [ ] **Commit 3**: Add integration tests for score validation and context preservation

## Key Insight

For normalized vectors (which fastembed produces):
- L2² = 2 * (1 - cosine_similarity)
- cosine_similarity = 1 - (L2² / 2)

The fix changes `search.rs:97` from:
```rust
let similarity = (1.0 - distance).clamp(0.0, 1.0);
```
To:
```rust
let similarity = (1.0 - (distance * distance / 2.0)).clamp(0.0, 1.0);
```

**No schema change needed.** Existing indexes work correctly after this fix.

## Commit Details

### Commit 1: Fix L2-to-cosine-similarity conversion formula

**Files Changed**:
- `crates/crumbly-core/src/knowledge/storage/sqlite/search.rs` - Fix formula, update comments

**Key Changes**:
- Line 97: Change conversion formula
- Lines 3,7,26,29: Update comments to clarify L2 distance is used but converted to cosine similarity

**Requirements Addressed**: R2, R4

### Commit 2: Preserve contexts during rebuild

**Files Changed**:
- `crates/crumbly-core/src/knowledge/facade/inner/builder.rs` - Read contexts before delete, restore after

**Requirements Addressed**: R3

### Commit 3: Add integration tests

**Files Changed**:
- `crates/crumbly-cli/tests/schema_migration.rs` - Context preservation and score validation tests

**Requirements Addressed**: R3, R4

## Notes

- R1 (schema v4 detection) is no longer needed since existing indexes work with the formula fix
- The schema version bump from Commit 1 (original) should be reverted
