#!/bin/bash
set -euo pipefail

# Launch an EKS nodegroup with a custom Bottlerocket AMI

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    cat <<EOF
Usage: $(basename "$0") --ami AMI_ID [OPTIONS]

Launch an EKS nodegroup with a custom Bottlerocket AMI

Required:
    --ami AMI_ID            AMI ID to use

Options:
    --cluster NAME          Cluster name (default: br-test-cluster)
    --nodegroup NAME        Nodegroup name (default: br-test-ng)
    --region REGION         AWS region (default: us-west-2)
    --instance-type TYPE    Instance type (default: m5.large)
    --capacity N            Desired capacity (default: 2)
    --k8s-version VER       Kubernetes version (default: 1.31)
    --keep-old              Don't delete old nodegroups (default: delete them)
    -h, --help              Show this help
EOF
    exit 1
}

# Defaults
AMI_ID=""
CLUSTER_NAME="br-test-cluster"
NODEGROUP_NAME="br-test-ng"
REGION="us-west-2"
INSTANCE_TYPE="m5.large"
DESIRED_CAPACITY="2"
K8S_VERSION="1.31"
KEEP_OLD=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --ami) AMI_ID="$2"; shift 2 ;;
        --cluster) CLUSTER_NAME="$2"; shift 2 ;;
        --nodegroup) NODEGROUP_NAME="$2"; shift 2 ;;
        --region) REGION="$2"; shift 2 ;;
        --instance-type) INSTANCE_TYPE="$2"; shift 2 ;;
        --capacity) DESIRED_CAPACITY="$2"; shift 2 ;;
        --k8s-version) K8S_VERSION="$2"; shift 2 ;;
        --keep-old) KEEP_OLD=true; shift ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [[ -z "$AMI_ID" ]]; then
    echo "Error: --ami is required"
    usage
fi

# Check prerequisites
for cmd in eksctl kubectl aws; do
    if ! command -v $cmd &>/dev/null; then
        echo "Error: $cmd is required but not installed"
        exit 1
    fi
done

if ! aws sts get-caller-identity &>/dev/null; then
    echo "Error: No valid AWS credentials"
    exit 1
fi

echo "Cluster: $CLUSTER_NAME"
echo "Nodegroup: $NODEGROUP_NAME"
echo "AMI: $AMI_ID"
echo "Region: $REGION"
echo ""

# Check if cluster exists
if eksctl get cluster --name "$CLUSTER_NAME" --region "$REGION" &>/dev/null; then
    echo "✓ Cluster $CLUSTER_NAME exists"
    
    # Delete old nodegroups unless --keep-old is specified
    if [[ "$KEEP_OLD" == "false" ]]; then
        echo "Checking for existing nodegroups to clean up..."
        OLD_NODEGROUPS=$(eksctl get nodegroup --cluster "$CLUSTER_NAME" --region "$REGION" -o json 2>/dev/null | jq -r '.[].Name' || true)
        
        for ng in $OLD_NODEGROUPS; do
            if [[ -n "$ng" ]]; then
                echo "Deleting old nodegroup: $ng"
                eksctl delete nodegroup --cluster "$CLUSTER_NAME" --region "$REGION" --name "$ng" --drain=false --disable-eviction || true
            fi
        done
        
        # Wait for deletions to complete
        if [[ -n "$OLD_NODEGROUPS" ]]; then
            echo "Waiting for nodegroup deletions to complete..."
            sleep 30
        fi
    fi
else
    echo "Creating cluster $CLUSTER_NAME..."
    eksctl create cluster         --name "$CLUSTER_NAME"         --region "$REGION"         --version "$K8S_VERSION"         --without-nodegroup
    echo "✓ Cluster created"
    
    echo "Installing default addons..."
    # VPC CNI for pod networking
    kubectl apply -f https://raw.githubusercontent.com/aws/amazon-vpc-cni-k8s/release-1.19/config/master/aws-k8s-cni.yaml
    # kube-proxy for service networking
    kubectl apply -f https://raw.githubusercontent.com/aws/amazon-eks-pod-identity-webhook/master/deploy/auth.yaml 2>/dev/null || true
    echo "✓ Addons installed"
fi

# Render template
TEMPLATE="$SCRIPT_DIR/eksctl-nodegroup.yaml.template"
CONFIG_FILE=$(mktemp /tmp/eksctl-config-XXXXXX.yaml)

export CLUSTER_NAME REGION NODEGROUP_NAME INSTANCE_TYPE DESIRED_CAPACITY AMI_ID
envsubst < "$TEMPLATE" > "$CONFIG_FILE"

echo "Creating nodegroup..."
eksctl create nodegroup -f "$CONFIG_FILE"

rm -f "$CONFIG_FILE"

# Wait for nodes and show status
echo ""
echo "Waiting for nodes to be ready..."
kubectl wait --for=condition=Ready nodes -l eks.amazonaws.com/nodegroup="$NODEGROUP_NAME" --timeout=300s 2>/dev/null || true

echo ""
echo "Node status:"
kubectl get nodes -l eks.amazonaws.com/nodegroup="$NODEGROUP_NAME" -o wide

echo ""
echo "✓ Nodegroup $NODEGROUP_NAME launched with AMI $AMI_ID"
