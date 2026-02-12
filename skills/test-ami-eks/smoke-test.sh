#!/bin/bash
set -euo pipefail

# Basic smoke test via SSM - verify instance reached multi-user

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Run smoke test on Bottlerocket nodes via SSM

Options:
    --instance ID           Test specific instance
    --cluster NAME          Get instances from EKS nodegroup
    --nodegroup NAME        Nodegroup name (requires --cluster)
    --region REGION         AWS region (default: us-west-2)
    -h, --help              Show this help

Examples:
    $(basename "$0") --instance i-0123456789abcdef0
    $(basename "$0") --cluster br-test-cluster --nodegroup br-test-ng
EOF
    exit 1
}

INSTANCE_ID=""
CLUSTER_NAME=""
NODEGROUP_NAME=""
REGION="us-west-2"

while [[ $# -gt 0 ]]; do
    case $1 in
        --instance) INSTANCE_ID="$2"; shift 2 ;;
        --cluster) CLUSTER_NAME="$2"; shift 2 ;;
        --nodegroup) NODEGROUP_NAME="$2"; shift 2 ;;
        --region) REGION="$2"; shift 2 ;;
        -h|--help) usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

if [[ -z "$INSTANCE_ID" ]] && [[ -z "$CLUSTER_NAME" ]]; then
    echo "Error: Specify --instance or --cluster/--nodegroup"
    usage
fi

# Get instance IDs from nodegroup if not specified directly
if [[ -n "$CLUSTER_NAME" ]]; then
    if [[ -z "$NODEGROUP_NAME" ]]; then
        NODEGROUP_NAME="br-test-ng"
    fi
    
    echo "Getting instances from nodegroup $NODEGROUP_NAME..."
    INSTANCES=$(aws ec2 describe-instances \
        --region "$REGION" \
        --filters \
            "Name=tag:eks:cluster-name,Values=$CLUSTER_NAME" \
            "Name=tag:eks:nodegroup-name,Values=$NODEGROUP_NAME" \
            "Name=instance-state-name,Values=running" \
        --query 'Reservations[].Instances[].InstanceId' \
        --output text)
    
    if [[ -z "$INSTANCES" ]]; then
        echo "Error: No running instances found in nodegroup"
        exit 1
    fi
else
    INSTANCES="$INSTANCE_ID"
fi

run_smoke_test() {
    local instance=$1
    echo ""
    echo "Testing instance: $instance"
    echo "--------------------------------"
    
    # Check SSM connectivity
    if ! aws ssm describe-instance-information \
        --region "$REGION" \
        --filters "Key=InstanceIds,Values=$instance" \
        --query 'InstanceInformationList[0].PingStatus' \
        --output text | grep -q "Online"; then
        echo "✗ SSM not available - checking console output..."
        "$SCRIPT_DIR/get-console.sh" --instance "$instance" --region "$REGION" | tail -50
        return 1
    fi
    echo "✓ SSM online"
    
    # Run smoke test commands via SSM -> apiclient exec admin
    echo "Running smoke test commands..."
    
    COMMANDS='apiclient exec admin bash -c "systemctl is-system-running; systemctl status multi-user.target --no-pager; uptime"'
    
    RESULT=$(aws ssm send-command \
        --region "$REGION" \
        --instance-ids "$instance" \
        --document-name "AWS-RunShellScript" \
        --parameters "commands=[$COMMANDS]" \
        --query 'Command.CommandId' \
        --output text)
    
    # Wait for command
    sleep 5
    
    OUTPUT=$(aws ssm get-command-invocation \
        --region "$REGION" \
        --command-id "$RESULT" \
        --instance-id "$instance" \
        --query '[Status, StandardOutputContent, StandardErrorContent]' \
        --output text 2>/dev/null || echo "Pending")
    
    STATUS=$(echo "$OUTPUT" | head -1)
    
    if [[ "$STATUS" == "Success" ]]; then
        echo "$OUTPUT" | tail -n +2
        if echo "$OUTPUT" | grep -q "running\|degraded"; then
            echo "✓ System reached multi-user target"
            return 0
        else
            echo "✗ System may not have fully booted"
            return 1
        fi
    else
        echo "Command status: $STATUS"
        echo "$OUTPUT"
        return 1
    fi
}

FAILED=0
for instance in $INSTANCES; do
    if ! run_smoke_test "$instance"; then
        FAILED=$((FAILED + 1))
    fi
done

echo ""
if [[ $FAILED -eq 0 ]]; then
    echo "✓ All smoke tests passed"
    exit 0
else
    echo "✗ $FAILED instance(s) failed smoke test"
    exit 1
fi
