#!/bin/bash
set -euo pipefail

# Register a Bottlerocket image as an AMI and track it

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Register a Bottlerocket image as an AMI and track in test_builds.toml

Options:
    --variant VAR       Variant name (e.g., aws-k8s-1.34)
    --arch ARCH         Architecture (default: x86_64)
    --region REGION     AWS region (default: us-west-2)
    --name NAME         Custom AMI name (auto-generated if not specified)
    --tracking FILE     Path to test_builds.toml (default: ./test_builds.toml)
    -h, --help          Show this help
EOF
    exit 1
}

# Defaults
ARCH="x86_64"
REGION="us-west-2"
VARIANT=""
AMI_NAME=""
TRACKING_FILE="./test_builds.toml"

while [[ $# -gt 0 ]]; do
    case $1 in
        --variant) VARIANT="$2"; shift 2 ;;
        --arch) ARCH="$2"; shift 2 ;;
        --region) REGION="$2"; shift 2 ;;
        --name) AMI_NAME="$2"; shift 2 ;;
        --tracking) TRACKING_FILE="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

# Must be in bottlerocket directory or have it as subdirectory
if [[ -d "bottlerocket" ]]; then
    BR_DIR="bottlerocket"
elif [[ -f "Twoliter.toml" ]]; then
    BR_DIR="."
else
    echo "Error: Must run from directory containing bottlerocket/ or from bottlerocket repo"
    exit 1
fi

# Auto-detect variant from latest build if not specified
if [[ -z "$VARIANT" ]]; then
    BUILD_DIR="$BR_DIR/build/images"
    if [[ -d "$BUILD_DIR" ]]; then
        LATEST=$(ls -1 "$BUILD_DIR" | grep "^${ARCH}-" | head -1)
        if [[ -n "$LATEST" ]]; then
            VARIANT=${LATEST#${ARCH}-}
            echo "Auto-detected variant: $VARIANT"
        fi
    fi
fi

if [[ -z "$VARIANT" ]]; then
    echo "Error: Could not detect variant. Specify with --variant"
    exit 1
fi

# Check AWS credentials
if ! aws sts get-caller-identity &>/dev/null; then
    echo "Error: No valid AWS credentials"
    exit 1
fi

# Calculate next build number
if [[ -f "$TRACKING_FILE" ]]; then
    BUILD_COUNT=$(grep -c '^number = ' "$TRACKING_FILE" 2>/dev/null || echo "0")
else
    BUILD_COUNT=0
    # Create tracking file with defaults
    cat > "$TRACKING_FILE" <<EOF
# AMI Build Tracking
default_region = "$REGION"
default_cluster = "br-test-cluster"
default_instance_type = "m5.large"
EOF
fi
NEXT_BUILD=$((BUILD_COUNT + 1))

# Generate AMI name if not specified
TIMESTAMP=$(date -u +"%Y%m%d-%H%M%S")
if [[ -z "$AMI_NAME" ]]; then
    AMI_NAME="${VARIANT}-test-${NEXT_BUILD}"
fi
AMI_DESC="${AMI_NAME}_${TIMESTAMP}"

echo "Build #$NEXT_BUILD: $VARIANT ($ARCH)"
echo "AMI Name: $AMI_NAME"

# Run cargo make ami
cd "$BR_DIR"
cargo make \
    -e BUILDSYS_VARIANT="$VARIANT" \
    -e BUILDSYS_ARCH="$ARCH" \
    -e PUBLISH_REGIONS="$REGION" \
    -e PUBLISH_AMI_NAME="$AMI_NAME" \
    -e PUBLISH_AMI_DESCRIPTION="$AMI_DESC" \
    ami

# Find and extract AMI ID from amis.json
AMIS_JSON=$(find "build/images/${ARCH}-${VARIANT}/latest" -name '*-amis.json' | head -1)
if [[ -z "$AMIS_JSON" ]] || [[ ! -f "$AMIS_JSON" ]]; then
    echo "Error: Could not find amis.json"
    exit 1
fi

AMI_ID=$(jq -r ".\"${REGION}\".id" "$AMIS_JSON")
if [[ -z "$AMI_ID" ]] || [[ "$AMI_ID" == "null" ]]; then
    echo "Error: Could not extract AMI ID for region $REGION"
    exit 1
fi

cd - >/dev/null

# Record in tracking file
cat >> "$TRACKING_FILE" <<EOF

[[builds]]
number = $NEXT_BUILD
variant = "$VARIANT"
arch = "$ARCH"
region = "$REGION"
timestamp = "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
ami_id = "$AMI_ID"
ami_name = "$AMI_NAME"
EOF

echo ""
echo "✓ AMI registered: $AMI_ID"
echo "✓ Build #$NEXT_BUILD recorded in $TRACKING_FILE"
echo ""
echo "$AMI_ID"
