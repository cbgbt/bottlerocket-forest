---
name: ssm-executor
description: Execute commands on Bottlerocket EC2 instances via AWS Systems Manager
---

# SSM Executor

Execute commands on Bottlerocket EC2 instances using AWS Systems Manager (SSM), with access to both the control container and host system.

## When to Use

- Debugging Bottlerocket instances (ECS, K8s, or standalone)
- Checking system state, logs, or configuration
- Running diagnostic commands
- When kubectl exec is not available or insufficient

## Prerequisites

- AWS credentials with SSM permissions
- Instance has SSM agent running (enabled by default in Bottlerocket)
- Instance has IAM role with `AmazonSSMManagedInstanceCore` policy
- Network path to SSM endpoints (internet or VPC endpoints)

## Procedure

### 1. Verify SSM Connectivity

```bash
./scripts/verify-connectivity.sh INSTANCE_ID REGION
```

Expected: `Online` status and `Bottlerocket` platform.

### 2. Execute Commands

**Simple command (control container context):**
```bash
./scripts/control-container-command.sh INSTANCE_ID REGION "uname -a"
```

**Access host rootfs via sheltie (full host access):**
```bash
./scripts/sheltie-command.sh INSTANCE_ID REGION "containerd --version"
```

### 3. Understanding the Execution Context

SSM commands run through a chain of contexts:

```
SSM → Control Container → (optional) Admin Container → Sheltie → Host
```

- **Control container**: Limited environment, has `apiclient`
- **Admin container**: Interactive shell, accessed via `apiclient exec admin bash`
- **Sheltie**: Direct host access via `apiclient exec admin sheltie -- <cmd>`

## Common Commands

### Bottlerocket Settings (control container)

```bash
./scripts/control-container-command.sh INSTANCE_ID REGION "apiclient get settings.kubernetes"
./scripts/control-container-command.sh INSTANCE_ID REGION "apiclient set motd='Debug session'"
./scripts/control-container-command.sh INSTANCE_ID REGION "apiclient get os"
```

### Host Binaries (via sheltie)

```bash
./scripts/sheltie-command.sh INSTANCE_ID REGION "containerd --version"
./scripts/sheltie-command.sh INSTANCE_ID REGION "kubelet --version"
./scripts/sheltie-command.sh INSTANCE_ID REGION "systemctl list-units --type=service"
./scripts/sheltie-command.sh INSTANCE_ID REGION "systemctl status containerd"
```

### Filesystem Inspection

```bash
./scripts/sheltie-command.sh INSTANCE_ID REGION "cat /etc/os-release"
./scripts/sheltie-command.sh INSTANCE_ID REGION "df -h"
./scripts/sheltie-command.sh INSTANCE_ID REGION "free -h"
```

### Networking

```bash
./scripts/sheltie-command.sh INSTANCE_ID REGION "ip addr"
./scripts/sheltie-command.sh INSTANCE_ID REGION "ip route"
./scripts/sheltie-command.sh INSTANCE_ID REGION "ss -tlnp"
```

## Comparison with k8s-node-executor

| Feature | ssm-executor | k8s-node-executor |
|---------|--------------|-------------------|
| Works with | Any EC2 instance | K8s nodes only |
| Requires | SSM connectivity | kubectl access |
| Access level | Full host via sheltie | Host namespaces via pod |
| Best for | ECS, standalone, early boot | K8s-specific debugging |

## Validation

- [ ] Instance shows `Online` in SSM
- [ ] Control container commands execute
- [ ] Sheltie commands access host

## Common Issues

**Instance not showing in SSM:**
- Check IAM role has SSM permissions
- Verify network path to SSM endpoints
- Instance may need reboot after IAM role attachment

**Command timeout:**
- Increase timeout in send-command
- Check instance is not overloaded

**Permission denied:**
- Some commands require sheltie for host access
- Check if admin container is enabled

## Reference

- [Bottlerocket Admin Container](https://github.com/bottlerocket-os/bottlerocket#admin-container)
- [AWS SSM Run Command](https://docs.aws.amazon.com/systems-manager/latest/userguide/execute-remote-commands.html)
