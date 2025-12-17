#!/bin/bash
set -euo pipefail
CLUSTER_NAME="${1:?Usage: get-eks-details.sh CLUSTER_NAME}"
aws eks describe-cluster --name "$CLUSTER_NAME" \
  --query 'cluster.{endpoint:endpoint,ca:certificateAuthority.data,name:name}' \
  --output json
