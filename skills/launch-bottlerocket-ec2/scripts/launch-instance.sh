#!/bin/bash
set -euo pipefail
AMI_ID="${1:?Usage: launch-instance.sh AMI_ID USERDATA_FILE}"
USERDATA_FILE="${2:?Usage: launch-instance.sh AMI_ID USERDATA_FILE}"

: "${REGION:=us-west-2}"
: "${SUBNET_ID:?Set SUBNET_ID environment variable}"
: "${SG_ID:?Set SG_ID environment variable}"
: "${INSTANCE_PROFILE:?Set INSTANCE_PROFILE environment variable}"
: "${INSTANCE_TYPE:=m5.large}"
: "${INSTANCE_NAME:=bottlerocket-test}"

aws ec2 run-instances \
  --region "$REGION" \
  --image-id "$AMI_ID" \
  --instance-type "$INSTANCE_TYPE" \
  --subnet-id "$SUBNET_ID" \
  --security-group-ids "$SG_ID" \
  --iam-instance-profile "Name=$INSTANCE_PROFILE" \
  --user-data "file://$USERDATA_FILE" \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$INSTANCE_NAME}]" \
  --query 'Instances[0].InstanceId' \
  --output text
