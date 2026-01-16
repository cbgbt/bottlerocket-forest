#!/usr/bin/env python3
"""Run clippy and emit steering guidance on failure."""
import subprocess
import sys

DESIGN_LINTS = {
    "cognitive_complexity", "too_many_arguments", "type_complexity",
    "excessive_nesting", "too_many_lines", "large_enum_variant",
    "result_large_err", "option_option", "struct_excessive_bools",
    "fn_params_excessive_bools",
}

def main():
    result = subprocess.run(
        ["cargo", "clippy", "--workspace", "--locked", "--quiet", "--", "-D", "warnings", "--no-deps"],
        capture_output=True, text=True
    )
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    if result.returncode != 0:
        if any(lint in result.stderr for lint in DESIGN_LINTS):
            print()
            print("⚡ PETAL_DRIFT: See docs/lint-steering/clippy.md")
        sys.exit(1)

if __name__ == "__main__":
    main()
