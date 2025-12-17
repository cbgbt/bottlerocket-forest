#!/bin/bash
set -euo pipefail
INSTANCE_ID="${1:?Usage: wait-for-instance.sh INSTANCE_ID}"
: "${REGION:=us-west-2}"

echo "Waiting for instance $INSTANCE_ID to be running..."
aws ec2 wait instance-running --instance-ids "$INSTANCE_ID" --region "$REGION"

aws ec2 describe-instances --instance-ids "$INSTANCE_ID" --region "$REGION" \
  --query 'Reservations[0].Instances[0].{State:State.Name,PrivateIP:PrivateIpAddress,PublicIP:PublicIpAddress}' \
  --output table
