#!/bin/bash
set -euo pipefail
INSTANCE_ID="${1:?Usage: verify-connectivity.sh INSTANCE_ID REGION}"
REGION="${2:?Usage: verify-connectivity.sh INSTANCE_ID REGION}"

aws ssm describe-instance-information \
  --filters "Key=InstanceIds,Values=$INSTANCE_ID" \
  --query 'InstanceInformationList[*].[InstanceId,PingStatus,PlatformName]' \
  --output table --region "$REGION"
