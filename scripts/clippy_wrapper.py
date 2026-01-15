#!/usr/bin/env python3
"""Run clippy and emit steering guidance on failure."""
import subprocess
import sys

def main():
    result = subprocess.run(
        ["cargo", "clippy", "--workspace", "--locked", "--quiet", "--", "-D", "warnings", "--no-deps"],
        capture_output=False
    )
    if result.returncode != 0:
        print()
        print("⚡ REFACTOR_PHOENIX: See docs/lint-steering/clippy.md")
        sys.exit(1)

if __name__ == "__main__":
    main()
