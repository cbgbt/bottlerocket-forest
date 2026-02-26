---
name: test-ami-eks
description: Register a Bottlerocket AMI and launch it on EKS for validation
---

# Test AMI on EKS

Register a Bottlerocket image as an AMI, track it, and launch it on an EKS cluster for validation.

## When to Use

- After building a Bottlerocket variant image
- To validate custom packages or configurations work on real infrastructure
- For integration testing before release

## Prerequisites

- Built Bottlerocket image in `bottlerocket/build/images/`
- AWS credentials with EC2 and EKS permissions
- `eksctl`, `kubectl`, `aws` CLI installed
- `jq` for JSON parsing

## Procedure

### Stage 1: Register AMI

From the grove root (or bottlerocket directory):

```bash
# Auto-detect variant from latest build
./skills/test-ami-eks/register-ami.sh

# Or specify variant explicitly
./skills/test-ami-eks/register-ami.sh --variant aws-k8s-1.34

# Custom options
./skills/test-ami-eks/register-ami.sh \
    --variant aws-k8s-1.34 \
    --arch x86_64 \
    --region us-west-2 \
    --tracking ./test_builds.toml
```

Outputs the AMI ID on the last line for piping.

### Stage 2: Launch on EKS

```bash
# Launch with AMI ID from register step
# NOTE: This will delete any existing nodegroups in the cluster first
./skills/test-ami-eks/launch-eks-nodegroup.sh --ami ami-0123456789abcdef0

# Full options
./skills/test-ami-eks/launch-eks-nodegroup.sh \
    --ami ami-0123456789abcdef0 \
    --cluster br-test-cluster \
    --nodegroup br-test-ng \
    --region us-west-2 \
    --instance-type m5.large \
    --capacity 2

# Keep existing nodegroups (don't auto-delete)
./skills/test-ami-eks/launch-eks-nodegroup.sh \
    --ami ami-0123456789abcdef0 \
    --keep-old
```

Creates cluster if it doesn't exist, then creates nodegroup.

### Stage 3: Validate

```bash
# Check nodes joined
kubectl get nodes -o wide

# Check Bottlerocket version
kubectl get nodes -o jsonpath='{.items[*].status.nodeInfo.osImage}'

# SSH via admin container (if enabled)
apiclient exec admin
```

### Stage 4: Cleanup

```bash
# Delete nodegroup only (keep cluster for reuse)
./skills/test-ami-eks/cleanup-nodegroup.sh \
    --cluster br-test-cluster \
    --nodegroup br-test-ng

# Delete cluster too
./skills/test-ami-eks/cleanup-nodegroup.sh \
    --cluster br-test-cluster \
    --nodegroup br-test-ng \
    --delete-cluster
```

### List Tracked Builds

```bash
# Show all builds
./skills/test-ami-eks/list-builds.sh --tracking ./test_builds.toml

# Last 5 builds
./skills/test-ami-eks/list-builds.sh --last 5

# Filter by variant
./skills/test-ami-eks/list-builds.sh --variant aws-k8s-1.34
```

## Files

| File | Purpose |
|------|--------|
| `register-ami.sh` | Register image as AMI, track in TOML |
| `launch-eks-nodegroup.sh` | Create cluster/nodegroup with AMI |
| `cleanup-nodegroup.sh` | Delete nodegroup/cluster |
| `list-builds.sh` | Display tracked builds |
| `eksctl-nodegroup.yaml.template` | eksctl config template |

## Tracking File Format

`test_builds.toml`:

```toml
default_region = "us-west-2"
default_cluster = "br-test-cluster"
default_instance_type = "m5.large"

[[builds]]
number = 1
variant = "aws-k8s-1.34"
arch = "x86_64"
region = "us-west-2"
timestamp = "2026-02-05T17:00:00Z"
ami_id = "ami-0123456789abcdef0"
ami_name = "aws-k8s-1.34-test-1"
```

## Validation Scripts

### Smoke Test via SSM

Run basic validation commands on nodes via SSM:

```bash
# Test all nodes in nodegroup
./skills/test-ami-eks/smoke-test.sh \
    --cluster br-test-cluster \
    --nodegroup br-test-ng

# Test specific instance
./skills/test-ami-eks/smoke-test.sh --instance i-0123456789abcdef0
```

Checks:
- SSM connectivity
- System reached multi-user target
- Basic system health

### Get Console Output

If SSM fails, check console output for boot issues:

```bash
# Get console output
./skills/test-ami-eks/get-console.sh --instance i-0123456789abcdef0

# Raw output only (for parsing)
./skills/test-ami-eks/get-console.sh --instance i-0123456789abcdef0 --raw
```

## Common Issues

**"No valid AWS credentials"**
- Run `aws sts get-caller-identity` to verify credentials
- Ensure credentials have EC2 and EKS permissions

**"Could not detect variant"**
- Specify `--variant` explicitly
- Ensure image was built in `bottlerocket/build/images/`

**"Nodegroup already exists"**
- Run `cleanup-nodegroup.sh` first to delete existing nodegroup

**Nodes not joining cluster**
- Check security groups allow node-to-control-plane communication
- Verify IAM roles have required policies
- Check `kubectl describe node` for errors
