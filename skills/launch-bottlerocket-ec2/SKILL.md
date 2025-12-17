---
name: launch-bottlerocket-ec2
description: Launch a Bottlerocket EC2 instance for testing with proper user data configuration
---

# Launch Bottlerocket EC2

Launch a Bottlerocket EC2 instance for testing, with proper user data configuration for EKS or standalone use.

## When to Use

- Testing a custom-built Bottlerocket AMI
- Validating settings or configuration changes
- Debugging Bottlerocket behavior on EC2

## Prerequisites

- AWS credentials configured
- VPC with subnet and security group
- IAM instance profile with required permissions
- For EKS: Existing EKS cluster

## Procedure

### 1. Gather Required Information

From user or environment:
- AMI ID (custom build or public AMI)
- Target cluster name (for EKS variants)
- Instance type preference
- Region

### 2. Verify AWS Access

```bash
aws sts get-caller-identity
```

### 3. Get Network Configuration

```bash
# List VPCs
aws ec2 describe-vpcs --query 'Vpcs[*].[VpcId,Tags[?Key==`Name`].Value|[0]]' --output table

# List subnets in VPC
aws ec2 describe-subnets --filters "Name=vpc-id,Values=VPC_ID" \
  --query 'Subnets[*].[SubnetId,AvailabilityZone,Tags[?Key==`Name`].Value|[0]]' --output table

# List security groups
aws ec2 describe-security-groups --filters "Name=vpc-id,Values=VPC_ID" \
  --query 'SecurityGroups[*].[GroupId,GroupName]' --output table
```

### 4. Prepare User Data

Create `/tmp/userdata.toml` based on variant:

**For EKS variants (aws-k8s-*):**
```toml
[settings.kubernetes]
api-server = "https://CLUSTER_ENDPOINT"
cluster-name = "CLUSTER_NAME"
cluster-certificate = "BASE64_ENCODED_CA_CERT"
```

To get EKS cluster details:
```bash
./scripts/get-eks-details.sh CLUSTER_NAME
```

### 5. Launch Instance

```bash
./scripts/launch-instance.sh AMI_ID USERDATA_FILE
```

Returns the instance ID.

### 6. Wait for Instance

```bash
./scripts/wait-for-instance.sh INSTANCE_ID
```

Waits for instance to be running and displays state and IP.

## Validation

### Check Instance Status

```bash
aws ec2 describe-instance-status --instance-ids INSTANCE_ID --region REGION
```

### For EKS: Verify Node Joined

```bash
kubectl get nodes -l "kubernetes.io/hostname=PRIVATE_DNS"
```

### Check SSM Connectivity

```bash
aws ssm describe-instance-information \
  --filters Key=InstanceIds,Values=INSTANCE_ID \
  --query 'InstanceInformationList[*].[InstanceId,PingStatus]' --output table
```

## Cleanup

```bash
aws ec2 terminate-instances --instance-ids INSTANCE_ID --region REGION
```

## Common Issues

**Instance fails to join cluster:** Check security group allows cluster communication.

**x509: certificate signed by unknown authority:** The `cluster-certificate` value is corrupted or stale.
Always fetch cluster details fresh at launch time using `get-eks-details.sh` — never cache or hardcode the certificate through intermediate steps, as base64 strings are easily corrupted by a single character.

**Multiple security groups required:** EKS nodes typically need both the cluster SG and the control plane SG.
The `--security-group-ids` flag in `launch-instance.sh` accepts a single `SG_ID` env var.
For multiple SGs, either call `aws ec2 run-instances` directly or set `SG_ID` to a space-separated list (unquoted).

**SSM not connecting:** Verify IAM role has SSM permissions and instance has internet access.

**User data not applied:** Ensure TOML syntax is valid. Check `/var/log/cloud-init-output.log` via SSM.

## Reference

- [Bottlerocket User Data](https://github.com/bottlerocket-os/bottlerocket#using-user-data)
- [EKS Node IAM Role](https://docs.aws.amazon.com/eks/latest/userguide/create-node-role.html)
