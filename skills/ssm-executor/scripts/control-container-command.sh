#!/bin/bash
set -euo pipefail
INSTANCE_ID="${1:?Usage: control-container-command.sh INSTANCE_ID REGION COMMAND}"
REGION="${2:?Usage: control-container-command.sh INSTANCE_ID REGION COMMAND}"
COMMAND="${3:?Usage: control-container-command.sh INSTANCE_ID REGION COMMAND}"

CMD_ID=$(aws ssm send-command \
  --instance-ids "$INSTANCE_ID" \
  --document-name "AWS-RunShellScript" \
  --parameters "{\"commands\":[\"$COMMAND\"]}" \
  --region "$REGION" \
  --query 'Command.CommandId' \
  --output text)
sleep 3
aws ssm get-command-invocation \
  --command-id "$CMD_ID" \
  --instance-id "$INSTANCE_ID" \
  --region "$REGION" \
  --query 'StandardOutputContent' \
  --output text
