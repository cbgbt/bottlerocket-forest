#!/usr/bin/env python3
"""Run clippy and emit steering guidance on failure."""
import subprocess
import sys
from pathlib import Path

GUIDANCE_FILE = Path(__file__).parent / "lint-guidance.md"

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
    result = subprocess.run(
        ["cargo", "clippy", "--workspace", "--locked", "--quiet", "--", "-D", "warnings", "--no-deps"],
        capture_output=False
    )
    if result.returncode != 0:
        content = GUIDANCE_FILE.read_text()
        preamble = extract_section(content, "preamble")
        clippy = extract_section(content, "clippy")
        print(f"\n{'='*60}\n")
        if preamble:
            print(preamble)
            print()
        if clippy:
            print(clippy)
        sys.exit(1)

if __name__ == "__main__":
    main()
