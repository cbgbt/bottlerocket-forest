# Lint Failure Guidance

Guidance for addressing lint failures in the Bottlerocket Forest codebase.

<!-- SECTION: preamble -->
## Before You Start

This lint exists to combat minimal change bias.

When code triggers a lint, the tempting fix is the smallest change:
move a few methods elsewhere, extract one helper, add another parameter.

Resist this impulse!

Minimal changes accumulate into the tangled code that triggered this lint.
Instead, step back and ask:

  "What would a loosely coupled design look like here?"

Think in terms of:
- Separate components with clear boundaries
- Types that own their responsibilities
- Interfaces that hide implementation details

The minimal change may be right here, but embracing a modular design pays for itself over the time horizon of this software's life.
<!-- END SECTION: preamble -->

<!-- SECTION: loc -->
## LOC Limit Guidance

The goal is not "make this file shorter."
The goal is "make this system easier to understand in pieces."

### Refactoring Patterns to Consider

1. INTERNAL DECOMPOSITION (preferred for large structs)
   If a struct has too many methods, decompose into internal components:

   BEFORE: facade.rs (900 lines)
     - KnowledgeIndex with build(), search(), gc(), etc. all in one file

   AFTER:
     - facade/mod.rs (500 lines): KnowledgeIndex struct + delegating methods
     - facade/builder.rs: KnowledgeIndex::build() calls builder::build()
     - facade/searcher.rs: KnowledgeIndex has a searcher::SearchOperation

   Components are private implementation details. Public API unchanged.

   ⚠️ DO NOT split a struct's impl blocks across files without decomposition.
   Scattered impl blocks fragment semantic understanding for LLMs.

2. SPLIT BY OPERATION (for files with distinct operations)
   If a module has multiple logical operations, split them:

   BEFORE: facade.rs (700 lines)
     - build(), rebuild(), update(), clear() + all tests

   AFTER:
     - facade/build.rs (400 lines): build(), rebuild() + their tests
     - facade/update.rs (150 lines): update() + its tests
     - facade/clear.rs (150 lines): clear() + its tests

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

4. TEST FIXTURES (module-local)
   Add shared helpers/fixtures to modules to reduce setup boilerplate:

   fn setup_test_repo() -> (TempDir, PathBuf) { ... }
   fn create_test_file(dir: &Path, name: &str, content: &str) { ... }
<!-- END SECTION: loc -->

<!-- SECTION: clippy -->
## Clippy Guidance

Clippy warnings often indicate deeper design issues, not just style violations.

### Refactoring Patterns to Consider

1. COMPLEXITY WARNINGS (cognitive_complexity, too_many_arguments)
   These signal functions doing too much.
   Should this function instead compose smaller abstractions?
   Can we extract behavior into methods?
   Weigh the cost of abstraction with the cost of the separation between components.

2. TYPE WARNINGS (type_complexity)
   Complex types suggest missing abstractions. Introduce newtypes or
   domain-specific types to clarify intent.

3. CLONE/COPY WARNINGS
   Excessive cloning may indicate ownership design issues. Consider
   borrowing patterns or restructuring data flow.

Address the underlying design issue, not just the symptom.
<!-- END SECTION: clippy -->
