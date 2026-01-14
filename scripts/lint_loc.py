#!/usr/bin/env python3
"""Lint Rust source files for line count limits."""
import sys
from pathlib import Path

MAX_LOC = 550
OVERRIDES = {
    "crates/crumbly-core/src/knowledge/storage/sqlite/mod.rs": 630,
    "crates/crumbly-core/src/knowledge/indexing/scanner/internals.rs": 565,
    "crates/crumbly-core/src/knowledge/chunking/rustdoc/extraction.rs": 575,
    "crates/crumbly-core/src/knowledge/indexing/config.rs": 570,
    "crates/crumbly-core/src/knowledge/search/semantic.rs": 575,
}
CRATES = ["crates/brdev", "crates/crumbly-cli", "crates/crumbly-core", "crates/forester"]

def is_test_file(path):
    s = str(path)
    return "/tests/" in s or s.endswith("tests.rs") or s.endswith("_test.rs")

def main():
    violations = []
    for crate in CRATES:
        for rs in Path(crate).rglob("*.rs"):
            if is_test_file(rs):
                continue
            limit = OVERRIDES.get(str(rs), MAX_LOC)
            lines = len(rs.read_text().splitlines())
            if lines > limit:
                violations.append((str(rs), lines, limit))
    if violations:
        print("LOC limit exceeded:\n")
        for path, lines, limit in violations:
            print(f"  {path}: {lines} lines (limit: {limit})")
        print("""
=== Before You Start ===

This lint exists to combat minimal change bias.

When a file grows too large, the tempting fix is the smallest change:
move a few methods elsewhere, extract one helper, add another parameter.

Resist this impulse!

Minimal changes accumulate into the tangled code that triggered this lint.
Instead, step back and ask:

  "What would a loosely coupled design look like here?"

Think in terms of:
- Separate components with clear boundaries
- Types that own their responsibilities
- Interfaces that hide implementation details

The goal is not "make this file shorter."
The goal is "make this system easier to understand in pieces."

=== Refactoring Patterns ===

1. INTERNAL DECOMPOSITION (preferred for large structs)
   If a struct has too many methods, decompose into internal components:

   BEFORE: facade/mod.rs (900 lines)
     - KnowledgeIndex with build(), search(), gc(), etc. all in one file

   AFTER:
     - facade/mod.rs (500 lines): KnowledgeIndex struct + delegating methods
     - facade/inner/builder.rs: BuildOperations component
     - facade/inner/searcher.rs: SearchOperations component

   Components are private implementation details. Public API unchanged.

   ⚠️ DO NOT split a struct's impl blocks across files without decomposition.
   Scattered impl blocks fragment semantic understanding for LLMs.

2. SPLIT BY OPERATION (for files with distinct operations)
   If a module has multiple logical operations, split them:

   BEFORE: facade/build.rs (700 lines)
     - build(), rebuild(), update(), clear() + all tests

   AFTER:
     - facade/build.rs (400 lines): build(), rebuild() + their tests
     - facade/update.rs (300 lines): update(), clear() + their tests

   Tests stay co-located with their implementation.

3. USE test_case FOR PARAMETERIZED TESTS
   Consolidate similar tests that differ only in inputs:

   #[test_case("" ; "empty string")]
   #[test_case("   " ; "whitespace only")]
   fn rejects_blank_input(input: &str) { ... }

* Preserve Given/When/Then comments in tests (required by style guide)
* Keep tests co-located with implementation
* DO NOT delete docstrings or comments to reduce line count
* If LOC limit signals a problem, the abstraction may be too large

4. DRY TEST HELPERS (module-local)
   Add helpers to #[cfg(test)] module to reduce setup boilerplate:

   fn setup_test_repo() -> (TempDir, PathBuf) { ... }
   fn create_test_file(dir: &Path, name: &str, content: &str) { ... }
""")
        sys.exit(1)
    print("All modules are within LOC limits")

if __name__ == "__main__":
    main()
