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
GUIDANCE_FILE = Path(__file__).parent / "lint-guidance.md"

def is_test_file(path):
    s = str(path)
    return "/tests/" in s or s.endswith("tests.rs") or s.endswith("_test.rs")

def extract_section(content: str, section: str) -> str:
    start = f"<!-- SECTION: {section} -->"
    end = f"<!-- END SECTION: {section} -->"
    try:
        s = content.index(start) + len(start)
        e = content.index(end)
        return content[s:e].strip()
    except ValueError:
        return ""

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
        content = GUIDANCE_FILE.read_text()
        preamble = extract_section(content, "preamble")
        loc = extract_section(content, "loc")
        print(f"\n{'='*60}\n")
        if preamble:
            print(preamble)
            print()
        if loc:
            print(loc)
        sys.exit(1)
    print("All modules are within LOC limits")

if __name__ == "__main__":
    main()
