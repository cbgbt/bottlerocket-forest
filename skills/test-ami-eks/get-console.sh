#!/bin/bash
set -euo pipefail

# Get EC2 console output for debugging boot issues

usage() {
    cat <<EOF
Usage: $(basename "$0") --instance INSTANCE_ID [OPTIONS]

Get EC2 console output (forces latest fetch)

Required:
    --instance ID       EC2 instance ID

Options:
    --region REGION     AWS region (default: us-west-2)
    --raw               Output raw console text only
    -h, --help          Show this help
EOF
    exit 1
}

INSTANCE_ID=""
REGION="us-west-2"
RAW=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --instance) INSTANCE_ID="$2"; shift 2 ;;
        --region) REGION="$2"; shift 2 ;;
        --raw) RAW=true; shift ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [[ -z "$INSTANCE_ID" ]]; then
    echo "Error: --instance is required"
    usage
fi

if [[ "$RAW" == "true" ]]; then
    aws ec2 get-console-output \
        --instance-id "$INSTANCE_ID" \
        --region "$REGION" \
        --latest \
        --query 'Output' \
        --output text
else
    echo "Console output for $INSTANCE_ID (region: $REGION)"
    echo "================================================"
    aws ec2 get-console-output \
        --instance-id "$INSTANCE_ID" \
        --region "$REGION" \
        --latest \
        --query 'Output' \
        --output text
fi
