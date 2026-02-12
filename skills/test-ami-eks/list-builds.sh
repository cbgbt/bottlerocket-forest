#!/bin/bash
set -euo pipefail

# List tracked AMI builds

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

List tracked AMI builds from test_builds.toml

Options:
    --tracking FILE     Path to test_builds.toml (default: ./test_builds.toml)
    --last N            Show only last N builds
    --variant VAR       Filter by variant
    -h, --help          Show this help
EOF
    exit 1
}

# Defaults
TRACKING_FILE="./test_builds.toml"
LAST_N=""
VARIANT_FILTER=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --tracking) TRACKING_FILE="$2"; shift 2 ;;
        --last) LAST_N="$2"; shift 2 ;;
        --variant) VARIANT_FILTER="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [[ ! -f "$TRACKING_FILE" ]]; then
    echo "Error: Tracking file not found: $TRACKING_FILE"
    exit 1
fi

# Parse and display builds
echo "AMI Builds from $TRACKING_FILE"
echo "======================================"
printf "%-4s %-25s %-8s %-25s %s\n" "#" "VARIANT" "ARCH" "AMI_ID" "TIMESTAMP"
echo "--------------------------------------"

# Simple awk parser for TOML builds
awk -v filter="$VARIANT_FILTER" '
    /^\[\[builds\]\]/ { in_build=1; num=""; var=""; arch=""; ami=""; ts=""; next }
    in_build && /^number = / { gsub(/number = /, ""); num=$0 }
    in_build && /^variant = / { gsub(/variant = "|"/,""); var=$0 }
    in_build && /^arch = / { gsub(/arch = "|"/,""); arch=$0 }
    in_build && /^ami_id = / { gsub(/ami_id = "|"/,""); ami=$0 }
    in_build && /^timestamp = / { gsub(/timestamp = "|"/,""); ts=$0 }
    in_build && /^$/ {
        if (num != "" && (filter == "" || var ~ filter)) {
            printf "%-4s %-25s %-8s %-25s %s\n", num, var, arch, ami, ts
        }
        in_build=0
    }
    END {
        if (in_build && num != "" && (filter == "" || var ~ filter)) {
            printf "%-4s %-25s %-8s %-25s %s\n", num, var, arch, ami, ts
        }
    }
' "$TRACKING_FILE" | if [[ -n "$LAST_N" ]]; then tail -n "$LAST_N"; else cat; fi
