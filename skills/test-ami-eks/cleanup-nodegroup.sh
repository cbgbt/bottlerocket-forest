#!/bin/bash
set -euo pipefail

# Clean up EKS nodegroup (and optionally cluster)

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Delete an EKS nodegroup

Options:
    --cluster NAME          Cluster name (default: br-test-cluster)
    --nodegroup NAME        Nodegroup name (default: br-test-ng)
    --region REGION         AWS region (default: us-west-2)
    --delete-cluster        Also delete the cluster
    -h, --help              Show this help
EOF
    exit 1
}

# Defaults
CLUSTER_NAME="br-test-cluster"
NODEGROUP_NAME="br-test-ng"
REGION="us-west-2"
DELETE_CLUSTER=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --cluster) CLUSTER_NAME="$2"; shift 2 ;;
        --nodegroup) NODEGROUP_NAME="$2"; shift 2 ;;
        --region) REGION="$2"; shift 2 ;;
        --delete-cluster) DELETE_CLUSTER=true; shift ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if ! aws sts get-caller-identity &>/dev/null; then
    echo "Error: No valid AWS credentials"
    exit 1
fi

# Delete nodegroup
if eksctl get nodegroup --cluster "$CLUSTER_NAME" --region "$REGION" --name "$NODEGROUP_NAME" &>/dev/null; then
    echo "Deleting nodegroup $NODEGROUP_NAME..."
    eksctl delete nodegroup \
        --cluster "$CLUSTER_NAME" \
        --region "$REGION" \
        --name "$NODEGROUP_NAME" \
        --wait
    echo "✓ Nodegroup deleted"
else
    echo "Nodegroup $NODEGROUP_NAME not found"
fi

# Optionally delete cluster
if [[ "$DELETE_CLUSTER" == "true" ]]; then
    if eksctl get cluster --name "$CLUSTER_NAME" --region "$REGION" &>/dev/null; then
        echo "Deleting cluster $CLUSTER_NAME..."
        eksctl delete cluster --name "$CLUSTER_NAME" --region "$REGION" --wait
        echo "✓ Cluster deleted"
    else
        echo "Cluster $CLUSTER_NAME not found"
    fi
fi
